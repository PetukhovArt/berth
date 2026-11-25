use berth_core::AgentKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::subagents::{self, Subagent};
use super::text::{
    clip, content_text, grok_user_text, owned, shown_user_text, single_line, TEXT_CHARS,
};
use super::turn_files::{file_edits, FileEdit};

const TOOL_CHARS: usize = 240;
const TOOL_KEYS: [&str; 13] = [
    "command",
    "cmd",
    "file_path",
    "target_file",
    "target_directory",
    "path",
    "pattern",
    "url",
    "query",
    "description",
    "prompt",
    "skill",
    "subject",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Claude,
    Codex,
    Grok,
}

impl Harness {
    const ALL: [Harness; 3] = [Harness::Claude, Harness::Codex, Harness::Grok];

    pub fn program(self) -> &'static str {
        match self {
            Harness::Claude => "claude",
            Harness::Codex => "codex",
            Harness::Grok => "grok",
        }
    }

    pub fn from_program(program: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|harness| harness.program() == program)
    }

    pub fn of(kind: AgentKind) -> Option<Self> {
        Self::from_program(kind.program()?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Serialize)]
pub struct TranscriptMessage {
    pub key: String,
    pub role: Role,
    pub text: String,
    pub tool: Option<String>,
    pub at: Option<String>,
    input: Option<Value>,
    pub(super) model: Option<String>,
    edits: Vec<FileEdit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) exit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) subagent: Option<Subagent>,
}

const PLAN_TOOLS: [&str; 4] = ["TodoWrite", "TaskCreate", "TaskUpdate", "update_plan"];

fn plan_input(name: Option<&str>, input: &Value) -> Option<Value> {
    if !PLAN_TOOLS.contains(&name?) {
        return None;
    }
    match input {
        Value::String(raw) => serde_json::from_str(raw).ok(),
        other => Some(other.clone()),
    }
}

fn tool_summary(input: &Value) -> String {
    let input = match input {
        Value::String(raw) => serde_json::from_str::<Value>(raw).unwrap_or_else(|_| input.clone()),
        _ => input.clone(),
    };
    if let Value::String(text) = &input {
        return single_line(text, TOOL_CHARS);
    }
    for key in TOOL_KEYS {
        let text = match &input[key] {
            Value::String(text) => text.clone(),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            _ => continue,
        };
        if !text.trim().is_empty() {
            return single_line(&text, TOOL_CHARS);
        }
    }
    if input.is_null() {
        return String::new();
    }
    single_line(&input.to_string(), TOOL_CHARS)
}

pub(super) fn message_key(record: u64, part: usize) -> String {
    format!("{record}:{part}")
}

pub(super) fn record_of(key: &str) -> Option<u64> {
    key.split(':').next()?.parse().ok()
}

pub fn claude_messages(
    line: &Value,
    record: u64,
    in_subagent: bool,
    out: &mut Vec<TranscriptMessage>,
) {
    let role = match line["type"].as_str() {
        Some("user") => Role::User,
        Some("assistant") => Role::Assistant,
        _ => return,
    };
    if line["isMeta"].as_bool() == Some(true)
        || (line["isSidechain"].as_bool() == Some(true) && !in_subagent)
    {
        return;
    }
    if role == Role::User
        && line["origin"]["kind"]
            .as_str()
            .is_some_and(|kind| kind != "human")
    {
        return;
    }
    let at = owned(&line["timestamp"]);
    let content = &line["message"]["content"];
    let blocks: Vec<&Value> = match content {
        Value::String(_) => vec![content],
        Value::Array(items) => items.iter().collect(),
        _ => return,
    };
    for (index, block) in blocks.into_iter().enumerate() {
        let key = message_key(record, index);
        let text = block.as_str().or_else(|| {
            (block["type"].as_str() == Some("text"))
                .then(|| block["text"].as_str())
                .flatten()
        });
        if let Some(text) = text {
            let shown = match role {
                Role::User => shown_user_text(text),
                Role::Assistant => {
                    Some(clip(text.trim(), TEXT_CHARS)).filter(|text| !text.is_empty())
                }
            };
            if let Some(text) = shown {
                out.push(TranscriptMessage {
                    key,
                    role,
                    text,
                    tool: None,
                    at: at.clone(),
                    input: None,
                    model: None,
                    edits: Vec::new(),
                    exit: None,
                    subagent: None,
                });
            }
        } else if block["type"].as_str() == Some("tool_use") {
            out.push(TranscriptMessage {
                key,
                role: Role::Assistant,
                text: tool_summary(&block["input"]),
                tool: owned(&block["name"]).or_else(|| Some("tool".to_owned())),
                at: at.clone(),
                input: plan_input(block["name"].as_str(), &block["input"]),
                model: None,
                edits: file_edits(block["name"].as_str(), &block["input"]),
                exit: None,
                subagent: subagents::launch(Harness::Claude, block),
            });
        }
    }
}

pub fn codex_messages(line: &Value, record: u64, out: &mut Vec<TranscriptMessage>) {
    if line["type"].as_str() != Some("response_item") {
        return;
    }
    let payload = &line["payload"];
    let at = owned(&line["timestamp"]);
    let kind = payload["type"].as_str().unwrap_or_default();
    let key = message_key(record, 0);
    let message = match kind {
        "message" => {
            let role = match payload["role"].as_str() {
                Some("user") => Role::User,
                Some("assistant") => Role::Assistant,
                _ => return,
            };
            let text = payload["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|block| block["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let shown = match role {
                Role::User => shown_user_text(&text),
                Role::Assistant => {
                    Some(clip(text.trim(), TEXT_CHARS)).filter(|text| !text.is_empty())
                }
            };
            let Some(text) = shown else {
                return;
            };
            TranscriptMessage {
                key,
                role,
                text,
                tool: None,
                at,
                input: None,
                model: None,
                edits: Vec::new(),
                exit: None,
                subagent: None,
            }
        }
        "function_call" | "custom_tool_call" => TranscriptMessage {
            key,
            role: Role::Assistant,
            text: tool_summary(if kind == "function_call" {
                &payload["arguments"]
            } else {
                &payload["input"]
            }),
            tool: owned(&payload["name"]).or_else(|| Some("tool".to_owned())),
            at,
            input: plan_input(
                payload["name"].as_str(),
                if kind == "function_call" {
                    &payload["arguments"]
                } else {
                    &payload["input"]
                },
            ),
            model: None,
            edits: file_edits(
                payload["name"].as_str(),
                if kind == "function_call" {
                    &payload["arguments"]
                } else {
                    &payload["input"]
                },
            ),
            exit: None,
            subagent: None,
        },
        "local_shell_call" => TranscriptMessage {
            key,
            role: Role::Assistant,
            text: tool_summary(&payload["action"]),
            tool: Some("shell".to_owned()),
            at,
            input: None,
            model: None,
            edits: file_edits(None, &payload["action"]),
            exit: None,
            subagent: None,
        },
        _ => return,
    };
    out.push(message);
}

pub fn grok_messages(line: &Value, record: u64, out: &mut Vec<TranscriptMessage>) {
    let plain = |part: usize, role: Role, text: String| TranscriptMessage {
        key: message_key(record, part),
        role,
        text,
        tool: None,
        at: None,
        input: None,
        model: None,
        edits: Vec::new(),
        exit: None,
        subagent: None,
    };
    match line["type"].as_str() {
        Some("user") => {
            if let Some(text) = grok_user_text(line) {
                out.push(plain(0, Role::User, text));
            }
        }
        Some("assistant") => {
            let text = content_text(&line["content"]).unwrap_or_default();
            let text = clip(text.trim(), TEXT_CHARS);
            if !text.is_empty() {
                out.push(plain(0, Role::Assistant, text));
            }
            let calls = line["tool_calls"].as_array().into_iter().flatten();
            for (index, call) in calls.enumerate() {
                let arguments = &call["arguments"];
                out.push(TranscriptMessage {
                    tool: owned(&call["name"]).or_else(|| Some("tool".to_owned())),
                    edits: file_edits(call["name"].as_str(), arguments),
                    ..plain(index + 1, Role::Assistant, tool_summary(arguments))
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_chat_history_gives_prompts_replies_and_tools_keyed_by_record_offset() {
        let lines = [
            r#"{"type":"user","content":[{"type":"text","text":"<user_info>os</user_info>"}]}"#,
            r#"{"type":"user","content":[{"type":"text","text":"<user_query>\nfix it\n</user_query>"}],"prompt_index":0}"#,
            r#"{"type":"reasoning","id":"rs_1","summary":[]}"#,
            r#"{"type":"assistant","content":"on it","tool_calls":[{"id":"c1","name":"search_replace","arguments":"{\"file_path\":\"a.rs\",\"old_string\":\"x\",\"new_string\":\"y\\nz\"}"}]}"#,
            r#"{"type":"tool_result","tool_call_id":"c1","content":"done"}"#,
        ];
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        let mut at = 0u64;
        for line in lines {
            offsets.push(at);
            grok_messages(&serde_json::from_str(line).unwrap(), at, &mut out);
            at += line.len() as u64 + 1;
        }
        assert_eq!(out.len(), 3);
        assert_eq!((out[0].role, out[0].text.as_str()), (Role::User, "fix it"));
        assert_eq!(out[0].key, format!("{}:0", offsets[1]));
        assert_eq!(out[1].text, "on it");
        assert_eq!(out[2].tool.as_deref(), Some("search_replace"));
        assert_eq!(out[2].key, format!("{}:1", offsets[3]));
        assert_eq!(out[2].edits[0].additions, 2);
    }
}
