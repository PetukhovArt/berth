use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;

const SCRIPT_ARG: &str = "acp-mock-script=";
const MOCK_TEST_NAME: &str = "acp::mock::mock_agent_entry";

pub fn mock_agent_args(script: &Path) -> Vec<String> {
    vec![
        MOCK_TEST_NAME.into(),
        "--exact".into(),
        "--nocapture".into(),
        "--quiet".into(),
        "--test-threads=1".into(),
        format!("{SCRIPT_ARG}{}", script.display()),
    ]
}

#[test]
fn mock_agent_entry() {
    let Some(script) =
        std::env::args().find_map(|a| a.strip_prefix(SCRIPT_ARG).map(str::to_string))
    else {
        return;
    };
    let steps: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(script).unwrap()).unwrap();
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut out = std::io::stdout();
    let mut ids: HashMap<String, Value> = HashMap::new();
    let mut send = |message: &Value| {
        writeln!(out, "{message}").unwrap();
        out.flush().unwrap();
    };
    for step in steps {
        if let Some(method) = step.get("expect").and_then(Value::as_str) {
            let id = loop {
                let message: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
                if message.get("method").and_then(Value::as_str) == Some(method) {
                    break message.get("id").cloned().unwrap_or(Value::Null);
                }
            };
            match (step.get("result"), step.get("error")) {
                (Some(result), _) => send(&json!({"jsonrpc": "2.0", "id": id, "result": result})),
                (None, Some(error)) => send(&json!({"jsonrpc": "2.0", "id": id, "error": error})),
                (None, None) => {
                    ids.insert(method.to_string(), id);
                }
            }
        } else if let Some(message) = step.get("send") {
            send(message);
        } else if step.get("await_response").is_some() {
            loop {
                let message: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
                if message.get("method").is_none() {
                    break;
                }
            }
        } else if let Some(method) = step.get("reply_to").and_then(Value::as_str) {
            let id = ids.remove(method).unwrap();
            send(&json!({"jsonrpc": "2.0", "id": id, "result": step["result"]}));
        }
    }
    for line in lines {
        if line.is_err() {
            break;
        }
    }
    std::process::exit(0);
}
