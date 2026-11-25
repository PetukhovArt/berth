mod items;
mod models;

use super::blocks::{pieces, Piece};
use super::options::{self, Model};
use super::shape::{self, ALLOW_ALWAYS, ALLOW_ONCE};
use super::translator::{run, str_at, text_at, Frame, Outputs, Translator, MCP_STATUS, REWIND};
use super::Driver;
use crate::acp::providers::AcpProvider;
use crate::acp::wire::{Host, LaunchSpec, Wire};
use anyhow::{anyhow, bail, ensure, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const NATIVE_ARGS: [&str; 3] = ["app-server", "--listen", "stdio://"];
const DEFAULT_EFFORT: &str = "medium";
const METHOD_NOT_FOUND: i64 = -32601;
const OUTPUT_KEPT: usize = 64 * 1024;
const SILENT_NOTIFICATIONS: [&str; 3] = [
    "turn/diff/updated",
    "thread/tokenUsage/updated",
    "account/rateLimits/updated",
];

pub struct Codex;

impl Driver for Codex {
    fn open(&self, provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire> {
        let translator = CodexTranslator::new(
            provider.command.clone(),
            provider.args.clone(),
            cwd.to_path_buf(),
        );
        Ok(run(translator, host))
    }
}

enum Open {
    New,
    Resume(String),
    Fork(String),
}

enum Then {
    Initialize(Value),
    Open(Value, Open),
    Opened(Value),
    Prompt(Value, Value),
}

impl Then {
    fn id(&self) -> &Value {
        match self {
            Then::Initialize(id) | Then::Open(id, _) | Then::Opened(id) | Then::Prompt(id, _) => id,
        }
    }
}

enum Pending {
    Initialize(Then),
    Models(Then),
    Thread(Then),
    Turn,
    Mcp(Value),
    Ignore,
}

enum Asked {
    Approval {
        request_id: Value,
        permissions: Option<Value>,
    },
    Question {
        request_id: Value,
        ids: Vec<String>,
    },
}

struct OpenTurn {
    reply: Value,
    id: Option<String>,
    interrupted: bool,
}

struct CodexTranslator {
    command: String,
    extra_args: Vec<String>,
    cwd: PathBuf,
    thread: Option<String>,
    loaded: bool,
    instructions: Option<String>,
    models: Vec<Model>,
    model: Option<String>,
    effort: Option<String>,
    turn: Option<OpenTurn>,
    pending: HashMap<u64, Pending>,
    asked: HashMap<u64, Asked>,
    calls: HashMap<String, Value>,
    streamed: HashSet<String>,
    output: HashMap<String, String>,
    next_id: u64,
}

fn keep_tail(text: &mut String) {
    if text.len() > OUTPUT_KEPT {
        let mut cut = text.len() - OUTPUT_KEPT;
        while !text.is_char_boundary(cut) {
            cut += 1;
        }
        text.drain(..cut);
    }
}

impl CodexTranslator {
    fn new(command: String, extra_args: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            command,
            extra_args,
            cwd,
            thread: None,
            loaded: false,
            instructions: None,
            models: Vec::new(),
            model: None,
            effort: None,
            turn: None,
            pending: HashMap::new(),
            asked: HashMap::new(),
            calls: HashMap::new(),
            streamed: HashSet::new(),
            output: HashMap::new(),
            next_id: 0,
        }
    }

    fn session(&self) -> String {
        self.thread.clone().unwrap_or_default()
    }

    fn current_model(&self) -> Option<&Model> {
        match &self.model {
            Some(value) => self.models.iter().find(|m| &m.value == value),
            None => self.models.first(),
        }
    }

    fn config_options(&self) -> Value {
        let model = self.current_model();
        let effort = self
            .effort
            .as_deref()
            .or(model.and_then(|m| m.default_effort.as_deref()))
            .unwrap_or(DEFAULT_EFFORT);
        let value = model.map(|m| m.value.as_str()).unwrap_or_default();
        options::config_options(&self.models, value, effort)
    }

    fn capabilities(&self) -> Value {
        let images = self.models.first().is_none_or(|model| model.images);
        json!({
            "protocolVersion": 1,
            "agentCapabilities": {
                "sessionCapabilities": {"resume": {}},
                "promptCapabilities": {"image": images, "embeddedContext": true},
                "_meta": {"berth": {"rewind": true, "mcp": "list"}},
            },
            "authMethods": [],
        })
    }

    fn call(
        &mut self,
        out: &mut Outputs,
        method: &str,
        params: Value,
        then: Pending,
    ) -> Result<()> {
        self.next_id += 1;
        out.send_native(&json!({"id": self.next_id, "method": method, "params": params}))?;
        self.pending.insert(self.next_id, then);
        Ok(())
    }

    fn answer(&mut self, out: &mut Outputs, id: Value, result: Value) -> Result<()> {
        out.send_native(&json!({"id": id, "result": result}))
    }

    fn forget_native(&mut self) {
        self.loaded = false;
        self.pending.clear();
        self.asked.clear();
        self.calls.clear();
        self.streamed.clear();
        self.output.clear();
    }

    fn start(&mut self, then: Then, out: &mut Outputs) -> Result<()> {
        let mut args: Vec<String> = NATIVE_ARGS.iter().map(|arg| arg.to_string()).collect();
        args.extend(self.extra_args.iter().cloned());
        out.start_native(LaunchSpec {
            program: self.command.clone(),
            args,
            cwd: self.cwd.clone(),
        })?;
        self.forget_native();
        let params = json!({
            "clientInfo": {"name": "berth", "title": "Berth", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true, "optOutNotificationMethods": SILENT_NOTIFICATIONS},
        });
        self.call(out, "initialize", params, Pending::Initialize(then))
    }

    fn run(&mut self, then: Then, out: &mut Outputs) {
        let id = then.id().clone();
        let result = if out.native_running() {
            self.proceed(then, out)
        } else {
            self.start(then, out)
        };
        if let Err(error) = result {
            self.reject(id, &format!("{error:#}"), out);
        }
    }

    fn reject(&mut self, id: Value, message: &str, out: &mut Outputs) {
        if self.turn.as_ref().is_some_and(|turn| turn.reply == id) {
            self.turn = None;
        }
        out.fail(id, message);
    }

    fn thread_params(&self) -> Value {
        let mut params = json!({
            "cwd": self.cwd,
            "approvalPolicy": "on-request",
            "sandbox": "workspace-write",
        });
        if let Some(instructions) = &self.instructions {
            params["developerInstructions"] = json!(instructions);
        }
        params
    }

    fn proceed(&mut self, then: Then, out: &mut Outputs) -> Result<()> {
        match then {
            Then::Initialize(id) => out.reply(id, self.capabilities()),
            Then::Opened(id) => out.reply(
                id,
                json!({"sessionId": self.session(), "configOptions": self.config_options()}),
            ),
            Then::Open(id, open) => {
                let mut params = self.thread_params();
                let method = match open {
                    Open::New => "thread/start",
                    Open::Resume(thread) => {
                        params["threadId"] = json!(thread);
                        params["excludeTurns"] = json!(true);
                        "thread/resume"
                    }
                    Open::Fork(turn) => {
                        params["threadId"] = json!(self.session());
                        params["lastTurnId"] = json!(turn);
                        params["excludeTurns"] = json!(true);
                        "thread/fork"
                    }
                };
                self.call(out, method, params, Pending::Thread(Then::Opened(id)))?;
            }
            Then::Prompt(id, params) if !self.loaded => {
                let mut open = self.thread_params();
                open["threadId"] = json!(self
                    .thread
                    .clone()
                    .ok_or_else(|| anyhow!("no session is open"))?);
                open["excludeTurns"] = json!(true);
                self.call(
                    out,
                    "thread/resume",
                    open,
                    Pending::Thread(Then::Prompt(id, params)),
                )?;
            }
            Then::Prompt(_, params) => self.begin_turn(&params, out)?,
        }
        Ok(())
    }

    fn begin_turn(&mut self, params: &Value, out: &mut Outputs) -> Result<()> {
        let blocks = params
            .get("prompt")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("the prompt has no blocks"))?;
        let mut input = Vec::new();
        let mut images = false;
        for piece in pieces(blocks) {
            input.push(match piece {
                Piece::Text(text) => json!({"type": "text", "text": text}),
                Piece::Image { mime, data } => {
                    images = true;
                    json!({"type": "image", "url": format!("data:{mime};base64,{data}")})
                }
            });
        }
        ensure!(!input.is_empty(), "the prompt is empty");
        if let Some(model) = self.current_model().filter(|m| images && !m.images) {
            bail!("{} takes no images", model.name);
        }
        let mut turn = json!({"threadId": self.session(), "input": input});
        if let Some(model) = &self.model {
            turn["model"] = json!(model);
        }
        if let Some(effort) = &self.effort {
            turn["effort"] = json!(effort);
        }
        self.call(out, "turn/start", turn, Pending::Turn)
    }

    fn interrupt(&mut self, out: &mut Outputs) -> Result<()> {
        let Some(turn) = self.turn.as_mut() else {
            return Ok(());
        };
        turn.interrupted = true;
        let Some(id) = turn.id.clone() else {
            return Ok(());
        };
        let params = json!({"threadId": self.session(), "turnId": id});
        self.call(out, "turn/interrupt", params, Pending::Ignore)
    }

    fn request(
        &mut self,
        id: Value,
        method: &str,
        params: &Value,
        out: &mut Outputs,
    ) -> Result<()> {
        match method {
            "initialize" => self.run(Then::Initialize(id), out),
            "session/new" | "session/resume" => {
                self.instructions = params
                    .pointer("/_meta/berth/instructions")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let open = match method {
                    "session/new" => Open::New,
                    _ => {
                        let thread = text_at(params, "sessionId");
                        ensure!(!thread.is_empty(), "the session id is missing");
                        Open::Resume(thread.to_string())
                    }
                };
                self.run(Then::Open(id, open), out);
            }
            "session/set_config_option" => {
                self.set_option(text_at(params, "configId"), text_at(params, "value"))?;
                out.reply(id, json!({"configOptions": self.config_options()}));
            }
            "session/prompt" => {
                ensure!(self.turn.is_none(), "a turn is already open");
                self.turn = Some(OpenTurn {
                    reply: id.clone(),
                    id: None,
                    interrupted: false,
                });
                self.run(Then::Prompt(id, params.clone()), out);
            }
            REWIND => {
                ensure!(self.turn.is_none(), "a turn is open");
                let open = match str_at(params, "to") {
                    Some(turn) => Open::Fork(turn.to_string()),
                    None => Open::New,
                };
                self.run(Then::Open(id, open), out);
            }
            MCP_STATUS => {
                let params = json!({"threadId": self.thread});
                self.call(out, "mcpServerStatus/list", params, Pending::Mcp(id))?;
            }
            other => bail!("{other} is not supported"),
        }
        Ok(())
    }

    fn set_option(&mut self, id: &str, value: &str) -> Result<()> {
        ensure!(!value.is_empty(), "the value is missing");
        match id {
            "model" => {
                let model = self.models.iter().find(|m| m.value == value);
                let model = model.ok_or_else(|| anyhow!("no model `{value}`"))?;
                if self
                    .effort
                    .as_ref()
                    .is_some_and(|e| !model.efforts.contains(e))
                {
                    self.effort = None;
                }
                self.model = Some(value.to_string());
            }
            "effort" => {
                let supported = self
                    .current_model()
                    .is_some_and(|m| m.efforts.iter().any(|e| e == value));
                ensure!(supported, "the model has no effort `{value}`");
                self.effort = Some(value.to_string());
            }
            other => bail!("no option `{other}`"),
        }
        Ok(())
    }

    fn answered(
        &mut self,
        id: Value,
        outcome: Result<Value, String>,
        out: &mut Outputs,
    ) -> Result<()> {
        let Some(asked) = id.as_u64().and_then(|id| self.asked.remove(&id)) else {
            return Ok(());
        };
        let answer = outcome.unwrap_or(Value::Null);
        let (request_id, reply) = match asked {
            Asked::Approval {
                request_id,
                permissions,
            } => {
                let option = answer.pointer("/outcome/optionId").and_then(Value::as_str);
                let reply = match permissions {
                    None => {
                        let decision = match option {
                            Some(ALLOW_ONCE) => "accept",
                            Some(ALLOW_ALWAYS) => "acceptForSession",
                            _ => "decline",
                        };
                        json!({"decision": decision})
                    }
                    Some(requested) => match option {
                        Some(ALLOW_ONCE) => json!({"permissions": requested, "scope": "turn"}),
                        Some(ALLOW_ALWAYS) => json!({"permissions": requested, "scope": "session"}),
                        _ => json!({"permissions": {}}),
                    },
                };
                (request_id, reply)
            }
            Asked::Question { request_id, ids } => {
                let mut answers = serde_json::Map::new();
                if text_at(&answer, "action") == "accept" {
                    for (index, id) in ids.iter().enumerate() {
                        if let Some(values) = shape::answer_values(&answer["content"], index) {
                            answers.insert(id.clone(), json!({"answers": values}));
                        }
                    }
                }
                (request_id, json!({"answers": answers}))
            }
        };
        self.answer(out, request_id, reply)
    }

    fn response(
        &mut self,
        id: Value,
        outcome: Result<Value, String>,
        out: &mut Outputs,
    ) -> Result<()> {
        let Some(pending) = id.as_u64().and_then(|id| self.pending.remove(&id)) else {
            return Ok(());
        };
        let result = match outcome {
            Ok(result) => result,
            Err(message) => {
                self.failed(pending, &message, out);
                return Ok(());
            }
        };
        match pending {
            Pending::Initialize(then) => {
                out.send_native(&json!({"method": "initialized"}))?;
                if self.models.is_empty() {
                    return self.call(out, "model/list", json!({}), Pending::Models(then));
                }
                self.resume(then, out);
            }
            Pending::Models(then) => {
                self.models = models::listed(&result);
                self.resume(then, out);
            }
            Pending::Thread(then) => {
                let thread = &result["thread"];
                self.thread = str_at(thread, "id").map(str::to_string);
                self.loaded = true;
                if self.model.is_none() {
                    self.model = str_at(&result, "model").map(str::to_string);
                }
                if self.effort.is_none() {
                    self.effort = str_at(&result, "reasoningEffort").map(str::to_string);
                }
                self.resume(then, out);
            }
            Pending::Turn => {
                let id = str_at(&result["turn"], "id").map(str::to_string);
                let interrupted = self.turn.as_mut().is_some_and(|turn| {
                    turn.id = id;
                    turn.interrupted
                });
                if interrupted {
                    self.interrupt(out)?;
                }
            }
            Pending::Mcp(id) => {
                let servers: Vec<Value> = result["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| json!({"name": s["name"], "status": s["runtimeStatus"]}))
                    .collect();
                out.reply(id, json!({"servers": servers}));
            }
            Pending::Ignore => {}
        }
        Ok(())
    }

    fn resume(&mut self, then: Then, out: &mut Outputs) {
        let id = then.id().clone();
        if let Err(error) = self.proceed(then, out) {
            self.reject(id, &format!("{error:#}"), out);
        }
    }

    fn failed(&mut self, pending: Pending, message: &str, out: &mut Outputs) {
        match pending {
            Pending::Initialize(then) | Pending::Models(then) | Pending::Thread(then) => {
                self.reject(then.id().clone(), message, out)
            }
            Pending::Turn => {
                if let Some(turn) = self.turn.take() {
                    out.fail(turn.reply, message);
                }
            }
            Pending::Mcp(id) => out.fail(id, message),
            Pending::Ignore => {}
        }
    }

    fn server_request(
        &mut self,
        id: Value,
        method: &str,
        params: &Value,
        out: &mut Outputs,
    ) -> Result<()> {
        let session = self.session();
        let (ask, asked) = match method {
            items::COMMAND_APPROVAL | items::FILE_APPROVAL | items::PERMISSIONS_APPROVAL => {
                let call = match self.calls.get(text_at(params, "itemId")) {
                    Some(call) => items::pending(call),
                    None => items::requested(method, params),
                };
                let ask = out.ask(
                    "session/request_permission",
                    json!({
                        "sessionId": session,
                        "toolCall": call,
                        "options": shape::permission_options(true),
                    }),
                );
                let permissions =
                    (method == items::PERMISSIONS_APPROVAL).then(|| params["permissions"].clone());
                (
                    ask,
                    Asked::Approval {
                        request_id: id,
                        permissions,
                    },
                )
            }
            "item/tool/requestUserInput" => {
                let questions = params
                    .get("questions")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let ids = questions
                    .iter()
                    .map(|q| text_at(q, "id").to_string())
                    .collect();
                let ask = out.ask(
                    "elicitation/create",
                    shape::elicitation_params(&questions, "Codex has a question"),
                );
                (
                    ask,
                    Asked::Question {
                        request_id: id,
                        ids,
                    },
                )
            }
            _ => {
                return out.send_native(&json!({
                    "id": id,
                    "error": {"code": METHOD_NOT_FOUND, "message": "not supported"},
                }));
            }
        };
        self.asked.insert(ask, asked);
        Ok(())
    }

    fn notification(&mut self, method: &str, params: &Value, out: &mut Outputs) {
        let thread = str_at(params, "threadId");
        if thread.is_some() && thread != self.thread.as_deref() {
            return;
        }
        let session = self.session();
        let item_id = text_at(params, "itemId");
        match method {
            "item/agentMessage/delta" => {
                self.streamed.insert(item_id.to_string());
                out.chunk(&session, "agent_message_chunk", text_at(params, "delta"));
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                self.streamed.insert(item_id.to_string());
                out.chunk(&session, "agent_thought_chunk", text_at(params, "delta"));
            }
            "item/reasoning/summaryPartAdded" if params["summaryIndex"].as_u64() > Some(0) => {
                out.chunk(&session, "agent_thought_chunk", "\n\n");
            }
            "item/commandExecution/outputDelta" => {
                let text = self.output.entry(item_id.to_string()).or_default();
                text.push_str(text_at(params, "delta"));
                keep_tail(text);
                let update = shape::tool_update(item_id, "in_progress", text);
                out.update(&session, update);
            }
            "item/started" => {
                if let Some(call) = items::started(&params["item"]) {
                    self.calls
                        .insert(text_at(&params["item"], "id").to_string(), call.clone());
                    out.update(&session, call);
                }
            }
            "item/completed" => self.item_completed(&params["item"], &session, out),
            "turn/plan/updated" => out.update(&session, items::plan(params)),
            "thread/name/updated" => {
                if let Some(title) = str_at(params, "threadName") {
                    out.update(
                        &session,
                        json!({"sessionUpdate": "session_info_update", "title": title}),
                    );
                }
            }
            "turn/started" => {
                if let Some(turn) = self.turn.as_mut().filter(|turn| turn.id.is_none()) {
                    turn.id = str_at(&params["turn"], "id").map(str::to_string);
                }
            }
            "turn/completed" => self.turn_completed(&params["turn"], out),
            _ => {}
        }
    }

    fn item_completed(&mut self, item: &Value, session: &str, out: &mut Outputs) {
        let id = text_at(item, "id");
        let silent = self.streamed.contains(id);
        match text_at(item, "type") {
            "agentMessage" if !silent => {
                out.chunk(session, "agent_message_chunk", text_at(item, "text"))
            }
            "reasoning" if !silent => {
                let summary = item.get("summary").and_then(Value::as_array);
                let text: Vec<&str> = summary
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                if !text.is_empty() {
                    out.chunk(session, "agent_thought_chunk", &text.join("\n\n"));
                }
            }
            _ => {
                let streamed = self.output.get(id).map(String::as_str);
                if let Some(update) = items::finished(item, streamed) {
                    out.update(session, update);
                }
            }
        }
    }

    fn turn_completed(&mut self, turn: &Value, out: &mut Outputs) {
        self.calls.clear();
        self.streamed.clear();
        self.output.clear();
        let Some(open) = self.turn.take() else {
            return;
        };
        let status = text_at(turn, "status");
        if open.interrupted || status == "interrupted" {
            out.reply(open.reply, json!({"stopReason": "cancelled"}));
        } else if status == "failed" {
            let message = str_at(&turn["error"], "message").unwrap_or("the turn failed");
            out.fail(open.reply, message);
        } else {
            out.reply(
                open.reply,
                json!({
                    "stopReason": "end_turn",
                    "_meta": {"berth": {"rewindPoint": str_at(turn, "id")}},
                }),
            );
        }
    }
}

impl Translator for CodexTranslator {
    fn on_berth(&mut self, frame: Frame, out: &mut Outputs) -> Result<()> {
        match frame {
            Frame::Request { id, method, params } => self.request(id, &method, &params, out),
            Frame::Notification { method, .. } => {
                if method == "session/cancel" {
                    self.interrupt(out)?;
                }
                Ok(())
            }
            Frame::Response { id, outcome } => self.answered(id, outcome, out),
        }
    }

    fn on_native(&mut self, line: &str, out: &mut Outputs) -> Result<()> {
        match Frame::parse(line) {
            Some(Frame::Response { id, outcome }) => self.response(id, outcome, out),
            Some(Frame::Request { id, method, params }) => {
                self.server_request(id, &method, &params, out)
            }
            Some(Frame::Notification { method, params }) => {
                self.notification(&method, &params, out);
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn native_exited(&mut self, code: Option<i32>, out: &mut Outputs) -> Result<()> {
        let code = code.map(|c| format!(" with code {c}")).unwrap_or_default();
        let reason = format!("codex exited{code}\n{}", out.failure_tail());
        let pending: Vec<Pending> = self.pending.drain().map(|(_, pending)| pending).collect();
        for pending in pending {
            self.failed(pending, reason.trim(), out);
        }
        if let Some(turn) = self.turn.take() {
            out.fail(turn.reply, reason.trim());
        }
        self.forget_native();
        Ok(())
    }
}
