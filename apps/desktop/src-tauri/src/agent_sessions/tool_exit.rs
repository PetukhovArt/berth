use serde_json::Value;

use super::{content_text, Harness};

const CODEX_OUTPUTS: [&str; 3] = [
    "function_call_output",
    "custom_tool_call_output",
    "local_shell_call_output",
];
const CODEX_CALLS: [&str; 3] = ["function_call", "custom_tool_call", "local_shell_call"];
const HEAD_CHARS: usize = 400;

fn int_after(text: &str, marker: &str) -> Option<i32> {
    let rest = text[text.find(marker)? + marker.len()..]
        .trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    let end = rest
        .char_indices()
        .find(|(index, c)| !(c.is_ascii_digit() || (*index == 0 && *c == '-')))
        .map_or(rest.len(), |(index, _)| index);
    rest[..end].parse().ok()
}

fn claude_exit(block: &Value) -> Option<i32> {
    if block["is_error"].as_bool() != Some(true) {
        return Some(0);
    }
    let text = content_text(&block["content"])?;
    let first = text.lines().next()?.to_ascii_lowercase();
    int_after(&first, "exit code")
}

fn codex_exit(payload: &Value) -> Option<i32> {
    let text = match &payload["output"] {
        Value::String(text) => text.clone(),
        Value::Null => return None,
        other => other.to_string(),
    };
    if let Some(code) = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|value| value["metadata"]["exit_code"].as_i64())
    {
        return i32::try_from(code).ok();
    }
    let lower: String = text.to_ascii_lowercase().chars().take(HEAD_CHARS).collect();
    let head = lower.split("output:").next().unwrap_or_default();
    int_after(head, "process exited with code").or_else(|| int_after(head, "exit code"))
}

pub fn call_ids(harness: Harness, line: &Value, in_subagent: bool) -> Vec<String> {
    match harness {
        Harness::Claude => {
            if line["type"].as_str() != Some("assistant")
                || line["isMeta"].as_bool() == Some(true)
                || (line["isSidechain"].as_bool() == Some(true) && !in_subagent)
            {
                return Vec::new();
            }
            line["message"]["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|block| block["type"].as_str() == Some("tool_use"))
                        .map(|block| block["id"].as_str().unwrap_or_default().to_owned())
                        .collect()
                })
                .unwrap_or_default()
        }
        Harness::Codex => {
            let payload = &line["payload"];
            if line["type"].as_str() == Some("response_item")
                && payload["type"]
                    .as_str()
                    .is_some_and(|kind| CODEX_CALLS.contains(&kind))
            {
                vec![payload["call_id"].as_str().unwrap_or_default().to_owned()]
            } else {
                Vec::new()
            }
        }
    }
}

pub fn results(harness: Harness, line: &Value) -> Vec<(String, i32)> {
    match harness {
        Harness::Claude => {
            if line["type"].as_str() != Some("user") {
                return Vec::new();
            }
            line["message"]["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|block| block["type"].as_str() == Some("tool_result"))
                        .filter_map(|block| {
                            let id = block["tool_use_id"].as_str()?.to_owned();
                            Some((id, claude_exit(block)?))
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        Harness::Codex => {
            let payload = &line["payload"];
            if line["type"].as_str() != Some("response_item")
                || !payload["type"]
                    .as_str()
                    .is_some_and(|kind| CODEX_OUTPUTS.contains(&kind))
            {
                return Vec::new();
            }
            payload["call_id"]
                .as_str()
                .zip(codex_exit(payload))
                .map(|(id, code)| vec![(id.to_owned(), code)])
                .unwrap_or_default()
        }
    }
}
