use anyhow::{Context, Result};
use berth_core::platform::{self, NoWindow};
use berth_core::AgentId;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;

const STDERR_LINES_KEPT: usize = 200;
const LOG_BYTES_KEPT: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum WireIn {
    Line { text: String, read_at: Instant },
    Exited(Option<i32>),
}

impl WireIn {
    pub fn line(text: String) -> Self {
        Self::Line {
            text,
            read_at: Instant::now(),
        }
    }
}

pub struct Wire {
    pub inbound: mpsc::UnboundedReceiver<WireIn>,
    pub outbound: mpsc::UnboundedSender<String>,
    pub process: Arc<dyn Process>,
}

pub trait Process: Send + Sync {
    fn kill(&self);
    fn stderr_tail(&self) -> Vec<String>;
}

pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(&'static str, &'static str)>,
}

pub trait Host: Send + Sync {
    fn spawn(&self, spec: LaunchSpec) -> Result<Wire>;
}

pub struct DaemonHost {
    pub agent: AgentId,
}

type StderrTail = Arc<Mutex<VecDeque<String>>>;

struct StdioProcess {
    pid: Option<u32>,
    stderr: StderrTail,
}

impl Process for StdioProcess {
    fn kill(&self) {
        if let Some(pid) = self.pid {
            if let Err(error) = platform::kill_process_tree(pid) {
                tracing::debug!(pid, error, "ACP agent kill failed");
            }
        }
    }

    fn stderr_tail(&self) -> Vec<String> {
        self.stderr.lock().unwrap().iter().cloned().collect()
    }
}

impl Host for DaemonHost {
    fn spawn(&self, spec: LaunchSpec) -> Result<Wire> {
        let program = platform::resolve_program(&spec.program)
            .with_context(|| format!("`{}` is not installed", spec.program))?;
        let mut command = Command::new(&program);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .no_window();
        for name in berth_core::env::AGENT_SESSION_VARS
            .iter()
            .chain(berth_core::env::HOST_AGENT_SESSION_VARS)
        {
            command.env_remove(name);
        }
        command.envs(spec.env.iter().copied());
        command.env(berth_core::env::AGENT_ID, &self.agent.0);
        platform::lead_process_tree(&mut command);
        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", program.display()))?;
        let stdin = child.stdin.take().context("agent stdin")?;
        let stdout = child.stdout.take().context("agent stdout")?;
        let stderr = child.stderr.take().context("agent stderr")?;
        let process = Arc::new(StdioProcess {
            pid: child.id(),
            stderr: Default::default(),
        });
        let (inbound_tx, inbound) = mpsc::unbounded_channel();
        let (outbound, outbound_rx) = mpsc::unbounded_channel();
        let log = berth_core::paths::acp_log_path(&self.agent.0);
        tokio::spawn(write_stdin(stdin, outbound_rx));
        tokio::spawn(read_stderr(stderr, process.stderr.clone(), open_log(&log)));
        tokio::spawn(read_stdout(stdout, child, inbound_tx));
        Ok(Wire {
            inbound,
            outbound,
            process,
        })
    }
}

fn open_log(path: &Path) -> Option<std::fs::File> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::File::create(path).ok()
}

async fn write_stdin(mut stdin: ChildStdin, mut lines: mpsc::UnboundedReceiver<String>) {
    while let Some(mut line) = lines.recv().await {
        line.push('\n');
        let written = async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await
        }
        .await;
        if written.is_err() {
            return;
        }
    }
}

async fn read_stderr(stderr: ChildStderr, tail: StderrTail, mut log: Option<std::fs::File>) {
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
    stdout: ChildStdout,
    mut child: Child,
    inbound: mpsc::UnboundedSender<WireIn>,
) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = inbound.send(WireIn::line(line));
    }
    let code = child.wait().await.ok().and_then(|s| s.code());
    let _ = inbound.send(WireIn::Exited(code));
}
