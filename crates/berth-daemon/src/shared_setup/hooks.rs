use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

pub(super) const SHARED_HOOK_TAG: &str = "_berthShared";
pub(super) const CODEX_GUARD_FILE: &str = "berth-shared-hooks";

const CODEX_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];

const GROK_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionDenied",
    "Stop",
    "StopFailure",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
];

const CURSOR_EVENTS: &[(&str, &str)] = &[
    ("SessionStart", "sessionStart"),
    ("SessionEnd", "sessionEnd"),
    ("UserPromptSubmit", "beforeSubmitPrompt"),
    ("PreToolUse", "preToolUse"),
    ("PostToolUse", "postToolUse"),
    ("PostToolUseFailure", "postToolUseFailure"),
    ("SubagentStart", "subagentStart"),
    ("SubagentStop", "subagentStop"),
    ("PreCompact", "preCompact"),
    ("Stop", "stop"),
];

const CURSOR_TOOLS: &[(&str, &str)] = &[
    ("Bash", "Shell"),
    ("Read", "Read"),
    ("Write", "Write"),
    ("Edit", "Write"),
    ("MultiEdit", "Write"),
    ("Grep", "Grep"),
    ("Task", "Task"),
    ("Agent", "Task"),
    ("WebFetch", "WebFetch"),
    ("WebSearch", "WebSearch"),
];

const CURSOR_TOOL_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "PostToolUseFailure"];

pub(super) struct HookItem {
    pub name: String,
    events: Vec<(String, Vec<Map<String, Value>>)>,
}

impl HookItem {
    pub fn read(root: &Path, name: &str) -> Result<Self> {
        let path = root.join("hooks").join(format!("{name}.json"));
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(name, &text).with_context(|| format!("{}", path.display()))
    }

    pub fn parse(name: &str, text: &str) -> Result<Self> {
        let Value::Object(object) = serde_json::from_str(text).context("not valid JSON")? else {
            bail!("not an object of hook events");
        };
        let mut events = Vec::new();
        for (event, groups) in object {
            let Value::Array(groups) = groups else {
                bail!("`{event}` is not an array of hook groups");
            };
            let mut list = Vec::new();
            for group in groups {
                let Value::Object(group) = group else {
                    bail!("a `{event}` group is not an object");
                };
                list.push(group);
            }
            events.push((event, list));
        }
        Ok(Self {
            name: name.to_string(),
            events,
        })
    }

    pub fn claude(&self) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        for (event, groups) in &self.events {
            for group in groups {
                let mut group = group.clone();
                group.insert(SHARED_HOOK_TAG.into(), json!(true));
                out.push((event.clone(), Value::Object(group)));
            }
        }
        out
    }

    pub fn codex(&self, skipped: &mut Vec<String>) -> Vec<(String, Value)> {
        let guard = codex_guard(&self.name);
        self.matcher_groups("codex", CODEX_EVENTS, &["command"], skipped)
            .into_iter()
            .map(|(event, mut group)| {
                for handler in group["hooks"].as_array_mut().into_iter().flatten() {
                    if let Some(command) = handler.get("command").and_then(Value::as_str) {
                        handler["command"] = json!(format!("{guard}{command}"));
                    }
                }
                group[SHARED_HOOK_TAG] = json!(true);
                (event, group)
            })
            .collect()
    }

    pub fn grok(&self, skipped: &mut Vec<String>) -> Vec<(String, Value)> {
        self.matcher_groups("grok", GROK_EVENTS, &["command", "http"], skipped)
    }

    pub fn cursor(&self, skipped: &mut Vec<String>) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        for (event, groups) in &self.events {
            let Some((_, cursor_event)) = CURSOR_EVENTS.iter().find(|(e, _)| e == event) else {
                skipped.push(self.no_event("cursor", event));
                continue;
            };
            for group in groups {
                let matcher = group.get("matcher").and_then(Value::as_str);
                let matcher = match matcher {
                    Some(m) if CURSOR_TOOL_EVENTS.contains(&event.as_str()) => {
                        match cursor_tools(m) {
                            Some(mapped) => Some(mapped),
                            None => {
                                skipped.push(format!(
                                    "hook `{}`: cursor has no tool matching `{m}` in `{event}`",
                                    self.name
                                ));
                                continue;
                            }
                        }
                    }
                    _ => None,
                };
                for handler in handlers(group) {
                    if handler_type(handler) != "command" {
                        skipped.push(self.no_handler("cursor", event, handler));
                        continue;
                    }
                    let mut entry = Map::new();
                    entry.insert("command".into(), handler["command"].clone());
                    if let Some(m) = &matcher {
                        entry.insert("matcher".into(), json!(m));
                    }
                    if let Some(timeout) = handler.get("timeout") {
                        entry.insert("timeout".into(), timeout.clone());
                    }
                    entry.insert(SHARED_HOOK_TAG.into(), json!(true));
                    out.push((cursor_event.to_string(), Value::Object(entry)));
                }
            }
        }
        out
    }

    fn matcher_groups(
        &self,
        kind: &str,
        supported: &[&str],
        handler_types: &[&str],
        skipped: &mut Vec<String>,
    ) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        for (event, groups) in &self.events {
            if !supported.contains(&event.as_str()) {
                skipped.push(self.no_event(kind, event));
                continue;
            }
            for group in groups {
                let mut kept = Vec::new();
                for handler in handlers(group) {
                    if handler_types.contains(&handler_type(handler)) {
                        kept.push(handler.clone());
                    } else {
                        skipped.push(self.no_handler(kind, event, handler));
                    }
                }
                if kept.is_empty() {
                    continue;
                }
                let mut group = group.clone();
                group.insert("hooks".into(), Value::Array(kept));
                out.push((event.clone(), Value::Object(group)));
            }
        }
        out
    }

    fn no_event(&self, kind: &str, event: &str) -> String {
        format!("hook `{}`: {kind} has no `{event}` hook", self.name)
    }

    fn no_handler(&self, kind: &str, event: &str, handler: &Value) -> String {
        format!(
            "hook `{}`: {kind} cannot run a `{}` handler in `{event}`",
            self.name,
            handler_type(handler)
        )
    }
}

fn handlers(group: &Map<String, Value>) -> impl Iterator<Item = &Value> {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn handler_type(handler: &Value) -> &str {
    handler
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("command")
}

fn cursor_tools(matcher: &str) -> Option<String> {
    let matcher = matcher.trim();
    if matcher.is_empty() || matcher == "*" {
        return Some(matcher.to_string());
    }
    let mut mapped: Vec<String> = Vec::new();
    for tool in matcher.split('|') {
        let tool = tool.trim();
        let cursor = match tool.strip_prefix("mcp__").map(|rest| rest.split("__")) {
            Some(mut parts) => match (parts.next(), parts.next(), parts.next()) {
                (Some(_server), Some(name), None) if !name.is_empty() => format!("MCP:{name}"),
                _ => continue,
            },
            None => match CURSOR_TOOLS.iter().find(|(claude, _)| *claude == tool) {
                Some((_, cursor)) => cursor.to_string(),
                None => continue,
            },
        };
        if !mapped.contains(&cursor) {
            mapped.push(cursor);
        }
    }
    (!mapped.is_empty()).then(|| mapped.join("|"))
}

fn codex_guard(item: &str) -> String {
    let quoted = item.replace('\'', r"'\''");
    format!(
        "grep -qxF '{quoted}' \"$(git rev-parse --git-common-dir 2>/dev/null)/{CODEX_GUARD_FILE}\" 2>/dev/null || exit 0; "
    )
}

pub(super) fn grok_file(groups: Vec<(String, Value)>) -> Value {
    let mut events = Map::new();
    for (event, group) in groups {
        events
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("built as an array")
            .push(group);
    }
    json!({ "hooks": events })
}
