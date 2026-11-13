use anyhow::{Context, Result};
use berth_core::platform::{self, NoWindow};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};

const STDERR_LINES_KEPT: usize = 200;
const LOG_BYTES_KEPT: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum Inbound {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Exited(Option<i32>),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RpcError {
    #[error("{message} ({code})")]
    Remote { code: i64, message: String },
    #[error("the agent process exited")]
    Closed,
}

type Waiters = Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>;

pub struct Conn {
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    waiters: Arc<Waiters>,
    next_id: AtomicU64,
    pid: Option<u32>,
    stderr: Arc<Mutex<VecDeque<String>>>,
    notifications_seen: Arc<AtomicU64>,
}

impl Conn {
    pub fn spawn(
        program: &Path,
        args: &[String],
        cwd: &Path,
        log: Option<&Path>,
    ) -> Result<(Arc<Conn>, mpsc::UnboundedReceiver<Inbound>)> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .no_window();
        platform::lead_process_tree(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", program.display()))?;
        let stdin = child.stdin.take().context("agent stdin")?;
        let stdout = child.stdout.take().context("agent stdout")?;
        let stderr = child.stderr.take().context("agent stderr")?;
        let conn = Arc::new(Conn {
            stdin: tokio::sync::Mutex::new(Some(stdin)),
            waiters: Default::default(),
            next_id: AtomicU64::new(1),
            pid: child.id(),
            stderr: Default::default(),
            notifications_seen: Default::default(),
        });
        let (tx, rx) = mpsc::unbounded_channel();
        let log = log.and_then(|path| {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::File::create(path).ok()
        });
        tokio::spawn(read_stderr(stderr, conn.stderr.clone(), log));
        tokio::spawn(read_stdout(
            stdout,
            child,
            conn.waiters.clone(),
            conn.notifications_seen.clone(),
            tx,
        ));
        Ok((conn, rx))
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.waiters.lock().unwrap().insert(id, tx);
        let sent = self
            .write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        if sent.is_err() {
            self.waiters.lock().unwrap().remove(&id);
            return Err(RpcError::Closed);
        }
        rx.await.unwrap_or(Err(RpcError::Closed))
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .await
    }

    pub async fn respond(&self, id: Value, result: Value) -> Result<()> {
        self.write(&json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .await
    }

    pub async fn respond_error(&self, id: Value, code: i64, message: &str) -> Result<()> {
        self.write(
            &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        )
        .await
    }

    pub fn kill(&self) {
        if let Some(pid) = self.pid {
            if let Err(error) = platform::kill_process_tree(pid) {
                tracing::debug!(pid, error, "ACP agent kill failed");
            }
        }
    }

    pub fn notifications_seen(&self) -> u64 {
        self.notifications_seen.load(Ordering::SeqCst)
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr.lock().unwrap().iter().cloned().collect()
    }

    async fn write(&self, message: &Value) -> Result<()> {
        let mut line = serde_json::to_vec(message)?;
        line.push(b'\n');
        let mut stdin = self.stdin.lock().await;
        let pipe = stdin.as_mut().context("agent stdin closed")?;
        let written = async {
            pipe.write_all(&line).await?;
            pipe.flush().await
        }
        .await;
        if written.is_err() {
            *stdin = None;
        }
        Ok(written?)
    }
}

async fn read_stderr(
    stderr: tokio::process::ChildStderr,
    tail: Arc<Mutex<VecDeque<String>>>,
    mut log: Option<std::fs::File>,
) {
    let mut lines = BufReader::new(stderr).lines();
    let mut logged = 0usize;
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(file) = log.as_mut() {
            use std::io::Write;
            logged += line.len() + 1;
            if logged > LOG_BYTES_KEPT || writeln!(file, "{line}").is_err() {
                log = None;
            }
        }
        let mut tail = tail.lock().unwrap();
        if tail.len() == STDERR_LINES_KEPT {
            tail.pop_front();
        }
        tail.push_back(line);
    }
}

async fn read_stdout(
    stdout: tokio::process::ChildStdout,
    mut child: Child,
    waiters: Arc<Waiters>,
    notifications_seen: Arc<AtomicU64>,
    tx: mpsc::UnboundedSender<Inbound>,
) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            tracing::debug!("ACP agent wrote a non-JSON line to stdout");
            continue;
        };
        if let Some(inbound) = route(message, &waiters) {
            if matches!(&inbound, Inbound::Notification { method, .. } if method == "session/update")
            {
                notifications_seen.fetch_add(1, Ordering::SeqCst);
            }
            let _ = tx.send(inbound);
        }
    }
    for (_, waiter) in waiters.lock().unwrap().drain() {
        let _ = waiter.send(Err(RpcError::Closed));
    }
    let code = child.wait().await.ok().and_then(|s| s.code());
    let _ = tx.send(Inbound::Exited(code));
}

fn route(mut message: Value, waiters: &Waiters) -> Option<Inbound> {
    let params = message
        .get_mut("params")
        .map(Value::take)
        .unwrap_or(Value::Null);
    if let Some(method) = message.get("method").and_then(Value::as_str) {
        let method = method.to_string();
        return Some(match message.get_mut("id").map(Value::take) {
            Some(id) if !id.is_null() => Inbound::Request { id, method, params },
            _ => Inbound::Notification { method, params },
        });
    }
    let id = message.get("id").and_then(Value::as_u64)?;
    let waiter = waiters.lock().unwrap().remove(&id)?;
    let outcome = match message.get_mut("error").map(Value::take) {
        Some(error) => Err(RpcError::Remote {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .to_string(),
        }),
        None => Ok(message
            .get_mut("result")
            .map(Value::take)
            .unwrap_or(Value::Null)),
    };
    let _ = waiter.send(outcome);
    None
}
