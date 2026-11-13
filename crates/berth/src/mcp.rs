use crate::client::runtime;
use crate::sessions;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const DEFAULT_PROTOCOL: &str = "2025-06-18";

pub trait Backend {
    fn sessions(&self, all: bool, only_attention: bool) -> Result<Value>;
    fn send(&self, session: &str, text: &str, submit: bool) -> Result<Value>;
    fn notes(&self, session: Option<&str>) -> Result<Value>;
}

struct Daemon {
    rt: tokio::runtime::Runtime,
}

impl Backend for Daemon {
    fn sessions(&self, all: bool, only_attention: bool) -> Result<Value> {
        self.rt.block_on(sessions::list(all, only_attention))
    }

    fn send(&self, session: &str, text: &str, submit: bool) -> Result<Value> {
        self.rt.block_on(sessions::send(session, text, submit))
    }

    fn notes(&self, session: Option<&str>) -> Result<Value> {
        self.rt.block_on(sessions::task_note(session))
    }
}

fn tool_list() -> Value {
    json!([
        {
            "name": "berth_sessions",
            "description": "List the Berth sessions (agents) as JSON: id, name, status, attention, alive, kind, workspace, project, branch and path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "all": { "type": "boolean", "description": "Include archived sessions" }
                }
            }
        },
        {
            "name": "berth_attention",
            "description": "List the sessions that need a person: waiting on a permission or question, or finished a turn nobody has read.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "berth_send",
            "description": "Type a message into a running session, for example another agent of the same task, and press Enter. Does not wait for the answer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session id, unique id prefix or exact name" },
                    "text": { "type": "string", "description": "The message" },
                    "submit": { "type": "boolean", "description": "Press Enter after the message (default true)" }
                },
                "required": ["session", "text"]
            }
        },
        {
            "name": "berth_notes",
            "description": "Read the Task note of a session's worktree: a decision or caveat the user attached to the task. Defaults to the calling session.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session id, unique id prefix or exact name (default: this session)" }
                }
            }
        }
    ])
}

fn call_tool(backend: &dyn Backend, name: &str, args: &Value) -> Result<Value, String> {
    let doc = match name {
        "berth_sessions" => backend.sessions(args["all"].as_bool().unwrap_or(false), false),
        "berth_attention" => backend.sessions(false, true),
        "berth_send" => {
            let session = args["session"].as_str().ok_or("`session` is required")?;
            let text = args["text"].as_str().ok_or("`text` is required")?;
            backend.send(session, text, args["submit"].as_bool().unwrap_or(true))
        }
        "berth_notes" => backend.notes(args["session"].as_str()),
        _ => return Err(format!("unknown tool `{name}`")),
    };
    Ok(match doc {
        Ok(doc) => json!({
            "content": [{ "type": "text", "text": doc.to_string() }],
            "isError": false,
        }),
        Err(err) => json!({
            "content": [{ "type": "text", "text": json!({ "ok": false, "error": format!("{err:#}") }).to_string() }],
            "isError": true,
        }),
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub fn handle(backend: &dyn Backend, line: &str) -> Option<Value> {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return Some(error(Value::Null, -32700, "parse error"));
    };
    let id = msg.get("id").cloned()?;
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match msg["method"].as_str().unwrap_or("") {
        "initialize" => json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(DEFAULT_PROTOCOL),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "berth", "version": env!("CARGO_PKG_VERSION") },
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tool_list() }),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            match call_tool(backend, name, &params["arguments"]) {
                Ok(result) => result,
                Err(message) => return Some(error(id, -32602, &message)),
            }
        }
        other => return Some(error(id, -32601, &format!("method not found: {other}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

pub fn serve(backend: &dyn Backend, input: impl BufRead, mut output: impl Write) -> Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(backend, &line) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

pub fn run_mcp() -> Result<()> {
    let backend = Daemon { rt: runtime()? };
    serve(&backend, std::io::stdin().lock(), std::io::stdout().lock())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Stub {
        sent: RefCell<Vec<(String, String, bool)>>,
    }

    impl Backend for Stub {
        fn sessions(&self, all: bool, only_attention: bool) -> Result<Value> {
            Ok(json!({ "ok": true, "sessions": [], "all": all, "attention": only_attention }))
        }

        fn send(&self, session: &str, text: &str, submit: bool) -> Result<Value> {
            if session == "gone" {
                anyhow::bail!("no session matches `gone`");
            }
            self.sent
                .borrow_mut()
                .push((session.into(), text.into(), submit));
            Ok(json!({ "ok": true, "session": { "id": "a1", "name": session } }))
        }

        fn notes(&self, session: Option<&str>) -> Result<Value> {
            Ok(json!({ "ok": true, "session": session, "note": "keep v2" }))
        }
    }

    fn ask(stub: &Stub, req: Value) -> Value {
        handle(stub, &req.to_string()).unwrap()
    }

    #[test]
    fn lists_the_tools() {
        let r = ask(
            &Stub::default(),
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        );
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "berth_sessions",
                "berth_attention",
                "berth_send",
                "berth_notes"
            ]
        );
    }

    #[test]
    fn initialize_echoes_the_protocol_and_notifications_get_no_reply() {
        let stub = Stub::default();
        let r = ask(
            &stub,
            json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}),
        );
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let note = json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string();
        assert!(handle(&stub, &note).is_none());
    }

    #[test]
    fn tools_call_round_trip_through_serve() {
        let stub = Stub::default();
        let input = [
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"berth_attention"}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"berth_send","arguments":{"session":"docs","text":"hi"}}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"berth_send","arguments":{"session":"gone","text":"hi"}}}),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let mut out = Vec::new();
        serve(&stub, input.as_bytes(), &mut out).unwrap();
        let replies: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 3);
        let doc: Value =
            serde_json::from_str(replies[0]["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(doc["attention"], true);
        assert_eq!(replies[1]["result"]["isError"], false);
        assert_eq!(
            *stub.sent.borrow(),
            [("docs".to_string(), "hi".to_string(), true)]
        );
        assert_eq!(replies[2]["result"]["isError"], true);
        assert!(replies[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("no session"));
    }

    #[test]
    fn bad_input_is_a_json_rpc_error() {
        let stub = Stub::default();
        assert_eq!(handle(&stub, "{").unwrap()["error"]["code"], -32700);
        let r = ask(&stub, json!({"jsonrpc":"2.0","id":1,"method":"nope"}));
        assert_eq!(r["error"]["code"], -32601);
        let r = ask(
            &stub,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"berth_send","arguments":{}}}),
        );
        assert_eq!(r["error"]["code"], -32602);
    }
}
