use crate::acp::wire::{Host, LaunchSpec, Process, Wire, WireIn};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

pub const REWIND: &str = "_berth/rewind";
pub const MCP_STATUS: &str = "_berth/mcp_status";

const FAILURE_CODE: i64 = -32000;
const STDERR_LINES_IN_FAILURE: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Response {
        id: Value,
        outcome: Result<Value, String>,
    },
}

impl Frame {
    fn parse(line: &str) -> Option<Frame> {
        let mut message: Value = serde_json::from_str(line).ok()?;
        let params = message
            .get_mut("params")
            .map(Value::take)
            .unwrap_or(Value::Null);
        let id = message
            .get_mut("id")
            .map(Value::take)
            .filter(|id| !id.is_null());
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            let method = method.to_string();
            return Some(match id {
                Some(id) => Frame::Request { id, method, params },
                None => Frame::Notification { method, params },
            });
        }
        let outcome = match message.get("error") {
            Some(error) => Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .to_string()),
            None => Ok(message
                .get_mut("result")
                .map(Value::take)
                .unwrap_or(Value::Null)),
        };
        Some(Frame::Response { id: id?, outcome })
    }
}

pub trait Translator: Send + 'static {
    fn on_berth(&mut self, frame: Frame, out: &mut Outputs) -> Result<()>;
    fn on_native(&mut self, line: &str, out: &mut Outputs) -> Result<()>;
    fn native_exited(&mut self, code: Option<i32>, out: &mut Outputs) -> Result<()>;
}

type Spawned = Arc<Mutex<Option<Arc<dyn Process>>>>;

pub struct Outputs {
    to_berth: mpsc::UnboundedSender<WireIn>,
    host: Arc<dyn Host>,
    merged: mpsc::UnboundedSender<(u64, WireIn)>,
    native: Option<mpsc::UnboundedSender<String>>,
    spawned: Spawned,
    generation: u64,
    read_at: Instant,
    next_ask: u64,
}

impl Outputs {
    fn send(&self, message: Value) {
        let _ = self.to_berth.send(WireIn::Line {
            text: message.to_string(),
            read_at: self.read_at,
        });
    }

    pub fn reply(&mut self, id: Value, result: Value) {
        self.send(json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }

    pub fn fail(&mut self, id: Value, message: &str) {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": FAILURE_CODE, "message": message},
        }));
    }

    pub fn update(&mut self, session_id: &str, update: Value) {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {"sessionId": session_id, "update": update},
        }));
    }

    pub fn ask(&mut self, method: &str, params: Value) -> u64 {
        self.next_ask += 1;
        self.send(json!({
            "jsonrpc": "2.0",
            "id": self.next_ask,
            "method": method,
            "params": params,
        }));
        self.next_ask
    }

    pub fn send_native(&mut self, message: &Value) -> Result<()> {
        self.native
            .as_ref()
            .ok_or_else(|| anyhow!("the agent process is not running"))?
            .send(message.to_string())
            .map_err(|_| anyhow!("the agent process closed its input"))
    }

    pub fn start_native(&mut self, spec: LaunchSpec) -> Result<()> {
        self.stop_native();
        let wire = self.host.spawn(spec)?;
        let generation = self.generation;
        let merged = self.merged.clone();
        let mut inbound = wire.inbound;
        tokio::spawn(async move {
            while let Some(read) = inbound.recv().await {
                if merged.send((generation, read)).is_err() {
                    break;
                }
            }
        });
        *self.spawned.lock().unwrap() = Some(wire.process);
        self.native = Some(wire.outbound);
        Ok(())
    }

    pub fn stop_native(&mut self) {
        if let Some(process) = self.spawned.lock().unwrap().take() {
            process.kill();
        }
        self.native = None;
        self.generation += 1;
    }

    pub fn native_running(&self) -> bool {
        self.native.is_some()
    }

    pub fn failure_tail(&self) -> String {
        let tail = self
            .spawned
            .lock()
            .unwrap()
            .as_ref()
            .map(|process| process.stderr_tail())
            .unwrap_or_default();
        let shown = &tail[tail.len().saturating_sub(STDERR_LINES_IN_FAILURE)..];
        shown.join("\n")
    }
}

struct Control {
    spawned: Spawned,
    stop: mpsc::UnboundedSender<()>,
}

impl Process for Control {
    fn kill(&self) {
        if let Some(process) = self.spawned.lock().unwrap().as_ref() {
            process.kill();
        }
        let _ = self.stop.send(());
    }

    fn stderr_tail(&self) -> Vec<String> {
        self.spawned
            .lock()
            .unwrap()
            .as_ref()
            .map(|process| process.stderr_tail())
            .unwrap_or_default()
    }
}

pub fn run<T: Translator>(mut translator: T, host: Arc<dyn Host>) -> Wire {
    let (to_berth, inbound) = mpsc::unbounded_channel();
    let (outbound, mut from_berth) = mpsc::unbounded_channel::<String>();
    let (merged, mut from_native) = mpsc::unbounded_channel::<(u64, WireIn)>();
    let (stop, mut stopped) = mpsc::unbounded_channel::<()>();
    let spawned: Spawned = Default::default();
    let mut out = Outputs {
        to_berth,
        host,
        merged,
        native: None,
        spawned: spawned.clone(),
        generation: 0,
        read_at: Instant::now(),
        next_ask: 0,
    };
    tokio::spawn(async move {
        loop {
            tokio::select! {
                line = from_berth.recv() => {
                    let Some(line) = line else { break };
                    out.read_at = Instant::now();
                    from_berth_line(&mut translator, &mut out, &line);
                }
                Some((generation, read)) = from_native.recv() => {
                    if generation != out.generation {
                        continue;
                    }
                    let handled = match read {
                        WireIn::Line { text, read_at } => {
                            out.read_at = read_at;
                            translator.on_native(&text, &mut out)
                        }
                        WireIn::Exited(code) => {
                            out.read_at = Instant::now();
                            out.native = None;
                            translator.native_exited(code, &mut out)
                        }
                    };
                    if let Err(error) = handled {
                        tracing::warn!(error = %error, "translating an agent frame failed");
                    }
                }
                _ = stopped.recv() => break,
            }
        }
        out.stop_native();
        let _ = out.to_berth.send(WireIn::Exited(None));
    });
    Wire {
        inbound,
        outbound,
        process: Arc::new(Control { spawned, stop }),
    }
}

fn from_berth_line<T: Translator>(translator: &mut T, out: &mut Outputs, line: &str) {
    let Some(frame) = Frame::parse(line) else {
        tracing::debug!("berth wrote a line the translator cannot read");
        return;
    };
    let request_id = match &frame {
        Frame::Request { id, .. } => Some(id.clone()),
        _ => None,
    };
    if let Err(error) = translator.on_berth(frame, out) {
        match request_id {
            Some(id) => out.fail(id, &format!("{error:#}")),
            None => tracing::warn!(error = %error, "handling a berth frame failed"),
        }
    }
}
