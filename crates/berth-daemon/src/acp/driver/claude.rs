mod blocks;
#[cfg(test)]
mod live;
mod options;
mod tools;

use super::options::Model;
use super::translator::{
    run, text_at, Frame, Outputs, Translator, INCOMPATIBLE, MCP_STATUS, REWIND, STEER,
};
use super::Driver;
use crate::acp::providers::AcpProvider;
use crate::acp::wire::{Host, LaunchSpec, Wire};
use anyhow::{anyhow, bail, ensure, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DEFAULT_MODEL: &str = "default";

pub struct Claude;

impl Driver for Claude {
    fn open(&self, provider: &AcpProvider, cwd: &Path, host: Arc<dyn Host>) -> Result<Wire> {
        let translator = ClaudeTranslator::new(
            provider.command.clone(),
            provider.args.clone(),
            cwd.to_path_buf(),
        );
        Ok(run(translator, host))
    }
}

enum Start {
    Fresh,
    Resume,
    Fork { from: String, at: String },
}

enum Asked {
    Permission {
        request_id: Value,
        input: Value,
        suggestions: Value,
    },
    Question {
        request_id: Value,
        input: Value,
        questions: Vec<Value>,
    },
}

enum Control {
    Initialize,
    McpStatus(Value),
    Fire,
}

struct ClaudeTranslator {
    command: String,
    extra_args: Vec<String>,
    cwd: PathBuf,
    session: String,
    start: Start,
    model: Option<String>,
    effort: Option<String>,
    models: Vec<Model>,
    turn: Option<Value>,
    interrupted: bool,
    asked: HashMap<u64, Asked>,
    controls: HashMap<String, Control>,
    streamed: HashSet<String>,
    plan_tools: HashSet<String>,
    tasks: tools::Tasks,
    last_assistant: Option<String>,
    steered: Vec<String>,
    next_control: u64,
}

fn prompt_blocks(params: &Value) -> Result<&[Value]> {
    params
        .get("prompt")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| anyhow!("the prompt has no blocks"))
}

fn new_session_id() -> String {
    let bits = ulid::Ulid::generate().0;
    let bits = (bits & !(0xF << 76) & !(0x3 << 62)) | (4 << 76) | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

impl ClaudeTranslator {
    fn new(command: String, extra_args: Vec<String>, cwd: PathBuf) -> Self {
        Self {
            command,
            extra_args,
            cwd,
            session: new_session_id(),
            start: Start::Fresh,
            model: None,
            effort: None,
            models: options::static_models(),
            turn: None,
            interrupted: false,
            asked: HashMap::new(),
            controls: HashMap::new(),
            streamed: HashSet::new(),
            plan_tools: HashSet::new(),
            tasks: tools::Tasks::default(),
            last_assistant: None,
            steered: Vec::new(),
            next_control: 0,
        }
    }

    fn config_options(&self) -> Value {
        options::config_options(&self.models, self.model.as_deref(), self.effort.as_deref())
    }

    fn spec(&mut self) -> LaunchSpec {
        let mut args: Vec<String> = [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-prompt-tool",
            "stdio",
            "--include-partial-messages",
            "--replay-user-messages",
        ]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
        args.extend(self.extra_args.iter().cloned());
        args.extend([
            "--append-system-prompt".into(),
            crate::agent::claude::system_prompt(None),
        ]);
        if let Some(model) = self.model.as_deref().filter(|m| *m != DEFAULT_MODEL) {
            args.extend(["--model".into(), model.into()]);
        }
        if let Some(effort) = &self.effort {
            args.extend(["--effort".into(), effort.clone()]);
        }
        let start = std::mem::replace(&mut self.start, Start::Resume);
        let start = match start {
            Start::Resume if crate::orphans::find_transcript(&self.session).is_none() => {
                Start::Fresh
            }
            other => other,
        };
        match start {
            Start::Fresh => args.extend(["--session-id".into(), self.session.clone()]),
            Start::Resume => args.extend(["--resume".into(), self.session.clone()]),
            Start::Fork { from, at } => args.extend([
                "--resume".into(),
                from,
                "--resume-session-at".into(),
                at,
                "--fork-session".into(),
                "--session-id".into(),
                self.session.clone(),
            ]),
        }
        LaunchSpec {
            program: self.command.clone(),
            args,
            cwd: self.cwd.clone(),
            env: vec![("CLAUDE_CODE_ENABLE_TODO_TOOLS", "true")],
        }
    }

    fn control(&mut self, out: &mut Outputs, request: Value, then: Control) -> Result<()> {
        self.next_control += 1;
        let request_id = format!("b{}", self.next_control);
        self.controls.insert(request_id.clone(), then);
        out.send_native(&json!({
            "type": "control_request",
            "request_id": request_id,
            "request": request,
        }))
    }

    fn ensure_native(&mut self, out: &mut Outputs) -> Result<()> {
        if out.native_running() {
            return Ok(());
        }
        let spec = self.spec();
        out.start_native(spec)?;
        self.control(out, json!({"subtype": "initialize"}), Control::Initialize)
    }

    fn forget_native(&mut self, out: &mut Outputs) {
        self.asked.clear();
        for (_, control) in self.controls.drain() {
            if let Control::McpStatus(id) = control {
                out.fail(id, "claude is no longer running");
            }
        }
        self.streamed.clear();
        self.plan_tools.clear();
        self.tasks.clear();
        self.steered.clear();
    }

    fn fail_handshake(&mut self, out: &mut Outputs, reason: &str) {
        out.stop_native();
        self.forget_native(out);
        if let Some(id) = self.turn.take() {
            out.fail_with(id, INCOMPATIBLE, reason.trim());
        }
    }

    fn request(
        &mut self,
        id: Value,
        method: &str,
        params: &Value,
        out: &mut Outputs,
    ) -> Result<()> {
        match method {
            "initialize" => out.reply(
                id,
                json!({
                    "protocolVersion": 1,
                    "agentCapabilities": {
                        "sessionCapabilities": {"resume": {}},
                        "promptCapabilities": {"image": true, "embeddedContext": true},
                        "_meta": {"berth": {"rewind": true, "steer": true, "mcp": "list"}},
                    },
                    "authMethods": [],
                }),
            ),
            "session/new" => {
                self.session = new_session_id();
                self.start = Start::Fresh;
                out.reply(
                    id,
                    json!({"sessionId": self.session, "configOptions": self.config_options()}),
                );
            }
            "session/resume" => {
                self.session = text_at(params, "sessionId").to_string();
                ensure!(!self.session.is_empty(), "the session id is missing");
                self.start = Start::Resume;
                out.reply(id, json!({"configOptions": self.config_options()}));
            }
            "session/set_config_option" => {
                self.set_option(text_at(params, "configId"), text_at(params, "value"), out)?;
                out.reply(id, json!({"configOptions": self.config_options()}));
            }
            "session/prompt" => {
                ensure!(self.turn.is_none(), "a turn is already open");
                let blocks = prompt_blocks(params)?;
                self.ensure_native(out)?;
                out.send_native(&blocks::user_message(blocks, None))?;
                self.turn = Some(id);
                self.interrupted = false;
            }
            STEER => {
                ensure!(self.turn.is_some(), "no turn is open");
                let uuid = new_session_id();
                out.send_native(&blocks::user_message(prompt_blocks(params)?, Some(&uuid)))?;
                self.steered.push(uuid);
                out.reply(id, json!({}));
            }
            REWIND => {
                ensure!(self.turn.is_none(), "a turn is open");
                let from = std::mem::replace(&mut self.session, new_session_id());
                self.start = match params.get("to").and_then(Value::as_str) {
                    Some(at) => Start::Fork {
                        from,
                        at: at.to_string(),
                    },
                    None => Start::Fresh,
                };
                out.stop_native();
                self.forget_native(out);
                out.reply(id, json!({"sessionId": self.session}));
            }
            MCP_STATUS => {
                self.ensure_native(out)?;
                self.control(
                    out,
                    json!({"subtype": "mcp_status"}),
                    Control::McpStatus(id),
                )?;
            }
            other => bail!("{other} is not supported"),
        }
        Ok(())
    }

    fn set_option(&mut self, id: &str, value: &str, out: &mut Outputs) -> Result<()> {
        ensure!(!value.is_empty(), "the value is missing");
        let live = out.native_running();
        match id {
            "model" => {
                self.model = Some(value.to_string());
                if live {
                    self.control(
                        out,
                        json!({"subtype": "set_model", "model": value}),
                        Control::Fire,
                    )?;
                }
            }
            "effort" => {
                self.effort = Some(value.to_string());
                if live {
                    self.control(
                        out,
                        json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": value}}),
                        Control::Fire,
                    )?;
                }
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
            Asked::Permission {
                request_id,
                input,
                suggestions,
            } => {
                let option = answer.pointer("/outcome/optionId").and_then(Value::as_str);
                (
                    request_id,
                    tools::permission_reply(option, &input, &suggestions),
                )
            }
            Asked::Question {
                request_id,
                mut input,
                questions,
            } => {
                let answers = (text_at(&answer, "action") == "accept")
                    .then(|| tools::question_reply(&questions, &answer["content"]))
                    .flatten();
                let reply = match answers {
                    Some(answers) => {
                        input["answers"] = answers;
                        json!({"behavior": "allow", "updatedInput": input})
                    }
                    None => json!({"behavior": "deny", "message": "The user did not answer."}),
                };
                (request_id, reply)
            }
        };
        out.send_native(&json!({
            "type": "control_response",
            "response": {"subtype": "success", "request_id": request_id, "response": reply},
        }))
    }

    fn can_use_tool(
        &mut self,
        request_id: Value,
        request: &Value,
        out: &mut Outputs,
    ) -> Result<()> {
        let input = request.get("input").cloned().unwrap_or(Value::Null);
        let asked = if text_at(request, "tool_name") == tools::ASK_USER {
            let questions = input
                .get("questions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if questions.is_empty() {
                return self.deny(out, request_id, "The question has no content.");
            }
            let id = out.ask("elicitation/create", tools::elicitation_params(&questions));
            (
                id,
                Asked::Question {
                    request_id,
                    input,
                    questions,
                },
            )
        } else {
            let params = tools::permission_params(&self.session, request);
            let id = out.ask("session/request_permission", params);
            let suggestions = request
                .get("permission_suggestions")
                .cloned()
                .unwrap_or(Value::Null);
            (
                id,
                Asked::Permission {
                    request_id,
                    input,
                    suggestions,
                },
            )
        };
        self.asked.insert(asked.0, asked.1);
        Ok(())
    }

    fn deny(&mut self, out: &mut Outputs, request_id: Value, message: &str) -> Result<()> {
        out.send_native(&json!({
            "type": "control_response",
            "response": {
                "subtype": "success",
                "request_id": request_id,
                "response": {"behavior": "deny", "message": message},
            },
        }))
    }

    fn stream_event(&mut self, frame: &Value, out: &mut Outputs) {
        let delta = &frame["event"]["delta"];
        let (kind, text) = match (text_at(&frame["event"], "type"), text_at(delta, "type")) {
            ("content_block_delta", "text_delta") => {
                ("agent_message_chunk", text_at(delta, "text"))
            }
            ("content_block_delta", "thinking_delta") => {
                ("agent_thought_chunk", text_at(delta, "thinking"))
            }
            _ => return,
        };
        if !frame["parent_tool_use_id"].is_null() {
            return;
        }
        self.streamed
            .insert(text_at(frame, "api_message_id").to_string());
        out.chunk(&self.session, kind, text);
    }

    fn assistant(&mut self, frame: &Value, out: &mut Outputs) {
        let nested = !frame["parent_tool_use_id"].is_null();
        if !nested {
            self.last_assistant = frame["uuid"].as_str().map(str::to_string);
        }
        let message = &frame["message"];
        let streamed = self.streamed.contains(text_at(message, "id")) || nested;
        for block in message["content"].as_array().into_iter().flatten() {
            match text_at(block, "type") {
                "text" if !streamed => {
                    out.chunk(&self.session, "agent_message_chunk", text_at(block, "text"))
                }
                "thinking" if !streamed && !text_at(block, "thinking").is_empty() => out.chunk(
                    &self.session,
                    "agent_thought_chunk",
                    text_at(block, "thinking"),
                ),
                "tool_use" => {
                    let (id, name, input) = (
                        text_at(block, "id"),
                        text_at(block, "name"),
                        &block["input"],
                    );
                    if name == tools::TODO_WRITE {
                        self.plan_tools.insert(id.to_string());
                        out.update(&self.session, tools::plan(input));
                    } else if name == tools::TASK_CREATE {
                        self.plan_tools.insert(id.to_string());
                        self.tasks.creating(id, input);
                    } else if name == tools::TASK_UPDATE {
                        self.plan_tools.insert(id.to_string());
                        if self.tasks.update(input) {
                            out.update(&self.session, self.tasks.plan());
                        }
                    } else {
                        out.update(&self.session, tools::tool_call(id, name, input));
                    }
                }
                _ => {}
            }
        }
    }

    fn user(&mut self, frame: &Value, out: &mut Outputs) {
        if frame["isReplay"] == json!(true) {
            self.steered.retain(|uuid| uuid != text_at(frame, "uuid"));
        }
        for block in frame["message"]["content"].as_array().into_iter().flatten() {
            let id = text_at(block, "tool_use_id");
            if text_at(block, "type") != "tool_result" {
                continue;
            }
            let failed = block["is_error"].as_bool() == Some(true);
            if self.tasks.created(id, failed, &block["content"]) {
                out.update(&self.session, self.tasks.plan());
            } else if !self.plan_tools.contains(id) {
                out.update(
                    &self.session,
                    tools::result_update(id, failed, &block["content"]),
                );
            }
        }
    }

    fn control_response(&mut self, frame: &Value, out: &mut Outputs) {
        let response = &frame["response"];
        let Some(control) = self.controls.remove(text_at(response, "request_id")) else {
            return;
        };
        let failed = text_at(response, "subtype") == "error";
        let body = &response["response"];
        match control {
            Control::Initialize if !failed => {
                let models = options::models_from(body);
                if !models.is_empty() {
                    self.models = models;
                    out.update(
                        &self.session,
                        json!({"sessionUpdate": "config_option_update", "configOptions": self.config_options()}),
                    );
                }
                let commands: Vec<Value> = body["commands"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| json!({"name": c["name"], "description": c["description"]}))
                    .collect();
                out.update(
                    &self.session,
                    json!({"sessionUpdate": "available_commands_update", "availableCommands": commands}),
                );
            }
            Control::Initialize => {
                let error = text_at(response, "error");
                self.fail_handshake(out, &format!("claude refused the handshake: {error}"));
            }
            Control::McpStatus(id) if failed => out.fail(id, text_at(response, "error")),
            Control::McpStatus(id) => {
                let servers: Vec<Value> = body["mcpServers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| json!({"name": s["name"], "status": s["status"]}))
                    .collect();
                out.reply(id, json!({"servers": servers}));
            }
            _ => {}
        }
    }

    fn result(&mut self, frame: &Value, out: &mut Outputs) {
        if !self.interrupted && !self.steered.is_empty() {
            return;
        }
        let Some(id) = self.turn.take() else {
            return;
        };
        let succeeded = text_at(frame, "subtype") == "success" && frame["is_error"] != json!(true);
        if self.interrupted || text_at(frame, "terminal_reason") == "aborted_streaming" {
            out.reply(id, json!({"stopReason": "cancelled"}));
        } else if succeeded {
            let stop = match text_at(frame, "stop_reason") {
                "max_tokens" => "max_tokens",
                "refusal" => "refusal",
                _ => "end_turn",
            };
            out.reply(
                id,
                json!({
                    "stopReason": stop,
                    "_meta": {"berth": {"rewindPoint": self.last_assistant}},
                }),
            );
        } else {
            let message = match text_at(frame, "result") {
                "" => text_at(frame, "subtype"),
                text => text,
            };
            out.fail(id, message);
        }
        self.interrupted = false;
        self.steered.clear();
        self.streamed.clear();
    }
}

impl Translator for ClaudeTranslator {
    fn on_berth(&mut self, frame: Frame, out: &mut Outputs) -> Result<()> {
        match frame {
            Frame::Request { id, method, params } => self.request(id, &method, &params, out),
            Frame::Notification { method, .. } => {
                if method == "session/cancel" && self.turn.is_some() {
                    self.interrupted = true;
                    self.control(out, json!({"subtype": "interrupt"}), Control::Fire)?;
                }
                Ok(())
            }
            Frame::Response { id, outcome } => self.answered(id, outcome, out),
        }
    }

    fn on_native(&mut self, line: &str, out: &mut Outputs) -> Result<()> {
        let Ok(frame) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        match text_at(&frame, "type") {
            "stream_event" => self.stream_event(&frame, out),
            "assistant" => self.assistant(&frame, out),
            "user" => self.user(&frame, out),
            "control_response" => self.control_response(&frame, out),
            "result" => self.result(&frame, out),
            "control_request" => {
                let request_id = frame["request_id"].clone();
                let request = &frame["request"];
                if text_at(request, "subtype") == "can_use_tool" {
                    self.can_use_tool(request_id, request, out)?;
                } else {
                    out.send_native(&json!({
                        "type": "control_response",
                        "response": {"subtype": "error", "request_id": request_id, "error": "not supported"},
                    }))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn native_exited(&mut self, code: Option<i32>, out: &mut Outputs) -> Result<()> {
        let handshaking = self
            .controls
            .values()
            .any(|control| matches!(control, Control::Initialize));
        let code = code.map(|c| format!(" with code {c}")).unwrap_or_default();
        let tail = out.failure_tail();
        if handshaking {
            let reason = format!("claude exited{code} before the handshake\n{tail}");
            self.fail_handshake(out, &reason);
            return Ok(());
        }
        self.forget_native(out);
        if let Some(id) = self.turn.take() {
            let reason = format!("claude exited{code} during the turn\n{tail}");
            out.fail(id, reason.trim());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_appends_the_berth_guidance_the_terminal_gets() {
        let mut translator = ClaudeTranslator::new("claude".into(), Vec::new(), PathBuf::from("."));
        let args = translator.spec().args;
        let at = args
            .iter()
            .position(|arg| arg == "--append-system-prompt")
            .expect("the guidance flag");
        assert_eq!(args[at + 1], crate::agent::claude::system_prompt(None));
    }
}
