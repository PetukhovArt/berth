use super::wire::{Process, Wire, WireIn};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub enum Inbound {
    Request {
        id: Value,
        method: String,
        params: Value,
        read_at: Instant,
    },
    Notification {
        method: String,
        params: Value,
        read_at: Instant,
    },
    Response(Response),
    Exited(Option<i32>),
}

pub struct Response {
    waiter: oneshot::Sender<Result<Value, RpcError>>,
    outcome: Result<Value, RpcError>,
}

impl Response {
    pub fn deliver(self) {
        let _ = self.waiter.send(self.outcome);
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Response").field(&self.outcome).finish()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RpcError {
    #[error("{message}")]
    Remote { code: i64, message: String },
    #[error("the agent process exited")]
    Closed,
}

type Waiters = Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>;

pub struct Conn {
    outbound: mpsc::UnboundedSender<String>,
    process: Arc<dyn Process>,
    waiters: Arc<Waiters>,
    next_id: AtomicU64,
    notifications_seen: Arc<AtomicU64>,
}

impl Conn {
    pub fn over(wire: Wire) -> (Arc<Conn>, mpsc::UnboundedReceiver<Inbound>) {
        let conn = Arc::new(Conn {
            outbound: wire.outbound,
            process: wire.process,
            waiters: Default::default(),
            next_id: AtomicU64::new(1),
            notifications_seen: Default::default(),
        });
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(read_wire(
            wire.inbound,
            conn.waiters.clone(),
            conn.notifications_seen.clone(),
            tx,
        ));
        (conn, rx)
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
        self.process.kill();
    }

    pub fn notifications_seen(&self) -> u64 {
        self.notifications_seen.load(Ordering::SeqCst)
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.process.stderr_tail()
    }

    async fn write(&self, message: &Value) -> Result<()> {
        self.outbound
            .send(serde_json::to_string(message)?)
            .map_err(|_| anyhow!("agent stdin closed"))
    }
}

async fn read_wire(
    mut wire: mpsc::UnboundedReceiver<WireIn>,
    waiters: Arc<Waiters>,
    notifications_seen: Arc<AtomicU64>,
    tx: mpsc::UnboundedSender<Inbound>,
) {
    let mut exit_code = None;
    while let Some(read) = wire.recv().await {
        let (line, read_at) = match read {
            WireIn::Line { text, read_at } => (text, read_at),
            WireIn::Exited(code) => {
                exit_code = code;
                break;
            }
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            tracing::debug!("ACP agent wrote a non-JSON line to stdout");
            continue;
        };
        if let Some(inbound) = route(message, &waiters, read_at) {
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
    let _ = tx.send(Inbound::Exited(exit_code));
}

fn route(mut message: Value, waiters: &Waiters, read_at: Instant) -> Option<Inbound> {
    let params = message
        .get_mut("params")
        .map(Value::take)
        .unwrap_or(Value::Null);
    if let Some(method) = message.get("method").and_then(Value::as_str) {
        let method = method.to_string();
        return Some(match message.get_mut("id").map(Value::take) {
            Some(id) if !id.is_null() => Inbound::Request {
                id,
                method,
                params,
                read_at,
            },
            _ => Inbound::Notification {
                method,
                params,
                read_at,
            },
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
    Some(Inbound::Response(Response { waiter, outcome }))
}
