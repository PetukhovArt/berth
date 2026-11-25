use berth_core::acp::{
    AcpCommand, AcpDiff, AcpPlanEntry, AcpTool, Applies, ChatCaps, Choice, ConfigOption, McpAccess,
    OptionCategory, QuestionForm, Questions,
};
use berth_core::{FeedbackKind, FeedbackRequest};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    User,
    Assistant,
    Thought,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolPatch {
    pub id: String,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub detail: Option<String>,
    pub diff: Option<AcpDiff>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    Text(TextKind, String),
    Tool(ToolPatch),
    Plan(Vec<AcpPlanEntry>),
    Title(String),
    Ignored,
}

pub struct Permission {
    pub request: FeedbackRequest,
    pub option_ids: Vec<String>,
}

pub fn session_update(params: &Value) -> Update {
    let Some(update) = params.get("update") else {
        return Update::Ignored;
    };
    let text_kind = match str_at(update, "sessionUpdate").unwrap_or_default() {
        "user_message_chunk" => TextKind::User,
        "agent_message_chunk" => TextKind::Assistant,
        "agent_thought_chunk" => TextKind::Thought,
        "tool_call" | "tool_call_update" => return Update::Tool(tool_patch(update)),
        "plan" => return Update::Plan(plan(update)),
        "session_info_update" => {
            return str_at(update, "title")
                .map(|t| Update::Title(t.to_string()))
                .unwrap_or(Update::Ignored)
        }
        _ => return Update::Ignored,
    };
    match update.get("content").and_then(text_of) {
        Some(text) => Update::Text(text_kind, text.to_string()),
        None => Update::Ignored,
    }
}

pub fn available_commands(params: &Value) -> Option<Vec<AcpCommand>> {
    let update = params.get("update")?;
    if str_at(update, "sessionUpdate") != Some("available_commands_update") {
        return None;
    }
    let commands = update
        .get("availableCommands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some(AcpCommand {
                name: str_at(c, "name")?.trim_start_matches('/').to_string(),
                description: str_at(c, "description").unwrap_or_default().to_string(),
            })
        })
        .collect();
    Some(commands)
}

pub fn config_option_update(params: &Value) -> Option<Vec<ConfigOption>> {
    let update = params.get("update")?;
    (str_at(update, "sessionUpdate") == Some("config_option_update"))
        .then(|| config_options(update))
        .flatten()
}

pub fn config_options(holder: &Value) -> Option<Vec<ConfigOption>> {
    let options = holder.get("configOptions")?.as_array()?;
    Some(options.iter().filter_map(config_option).collect())
}

fn config_option(option: &Value) -> Option<ConfigOption> {
    if str_at(option, "type") != Some("select") {
        return None;
    }
    let id = str_at(option, "id")?;
    let mut choices = Vec::new();
    for entry in option.get("options")?.as_array()? {
        match entry.get("options").and_then(Value::as_array) {
            Some(group) => choices.extend(group.iter().filter_map(choice)),
            None => choices.extend(choice(entry)),
        }
    }
    Some(ConfigOption {
        id: id.to_string(),
        name: str_at(option, "name").unwrap_or(id).to_string(),
        category: match str_at(option, "category") {
            Some("model") => OptionCategory::Model,
            Some("thought_level") => OptionCategory::ThoughtLevel,
            Some("mode") => OptionCategory::Mode,
            _ => OptionCategory::Other,
        },
        current: str_at(option, "currentValue")?.to_string(),
        choices,
        applies: Applies::Now,
    })
}

fn choice(entry: &Value) -> Option<Choice> {
    let value = str_at(entry, "value")?;
    Some(Choice {
        value: value.to_string(),
        name: str_at(entry, "name").unwrap_or(value).to_string(),
        description: str_at(entry, "description").map(str::to_string),
    })
}

pub fn chat_caps(initialized: &Value) -> ChatCaps {
    let prompt = |key: &str| {
        initialized
            .pointer("/agentCapabilities/promptCapabilities")
            .and_then(|caps| caps.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let berth = initialized.pointer("/agentCapabilities/_meta/berth");
    ChatCaps {
        images: prompt("image"),
        embedded_files: prompt("embeddedContext"),
        questions: Questions::Typed,
        rewind: berth
            .and_then(|meta| meta.get("rewind"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        mcp: match berth.and_then(|meta| str_at(meta, "mcp")) {
            Some("list") => McpAccess::List,
            Some("toggle") => McpAccess::Toggle,
            _ => McpAccess::None,
        },
        ..ChatCaps::default()
    }
}

pub fn elicitation(params: &Value) -> Option<(String, QuestionForm)> {
    if str_at(params, "mode") != Some("form") {
        return None;
    }
    let form = crate::acp::questions::form_from_schema(params.get("requestedSchema")?)?;
    Some((str_at(params, "message")?.to_string(), form))
}

pub fn permission(params: &Value) -> Permission {
    let call = params.get("toolCall").cloned().unwrap_or(Value::Null);
    let patch = tool_patch(&call);
    let (names, option_ids) = params
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(|o| Some((str_at(o, "name")?, str_at(o, "optionId")?)))
                .take(crate::feedback::MAX_OPTIONS)
                .map(|(name, id)| (name.to_string(), id.to_string()))
                .unzip()
        })
        .unwrap_or_default();
    Permission {
        request: FeedbackRequest {
            kind: FeedbackKind::Permission,
            title: patch.title.clone().unwrap_or_else(|| "Permission".into()),
            detail: call
                .get("rawInput")
                .and_then(crate::feedback::detail)
                .or(patch.detail)
                .or(patch.diff.map(|d| d.path)),
            options: names,
            always_allow: false,
            form: None,
        },
        option_ids,
    }
}

pub fn apply_patch(tool: &mut AcpTool, patch: ToolPatch) {
    let ToolPatch {
        id: _,
        title,
        kind,
        status,
        detail,
        diff,
        output,
    } = patch;
    if let Some(v) = title {
        tool.title = v;
    }
    if let Some(v) = kind {
        tool.kind = v;
    }
    if let Some(v) = status {
        tool.status = v;
    }
    if detail.is_some() {
        tool.detail = detail;
    }
    if diff.is_some() {
        tool.diff = diff;
    }
    if output.is_some() {
        tool.output = output;
    }
}

fn tool_patch(call: &Value) -> ToolPatch {
    let content = call.get("content").and_then(Value::as_array);
    let output: String = content
        .into_iter()
        .flatten()
        .filter(|c| str_at(c, "type") == Some("content"))
        .filter_map(|c| c.get("content").and_then(text_of))
        .collect::<Vec<_>>()
        .join("\n");
    let diff = content
        .into_iter()
        .flatten()
        .find(|c| str_at(c, "type") == Some("diff"))
        .and_then(|d| {
            Some(AcpDiff {
                path: str_at(d, "path")?.to_string(),
                old: str_at(d, "oldText").map(str::to_string),
                new: str_at(d, "newText").unwrap_or_default().to_string(),
            })
        });
    let location = call
        .get("locations")
        .and_then(Value::as_array)
        .and_then(|l| l.first())
        .and_then(|l| str_at(l, "path"))
        .map(str::to_string);
    ToolPatch {
        id: str_at(call, "toolCallId").unwrap_or_default().to_string(),
        title: str_at(call, "title").map(str::to_string),
        kind: str_at(call, "kind").map(str::to_string),
        status: str_at(call, "status").map(str::to_string),
        detail: call
            .get("rawInput")
            .and_then(crate::feedback::preview)
            .or(location),
        diff,
        output: (!output.is_empty()).then_some(output),
    }
}

fn plan(update: &Value) -> Vec<AcpPlanEntry> {
    update
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(AcpPlanEntry {
                content: str_at(e, "content")?.to_string(),
                priority: str_at(e, "priority").unwrap_or("medium").to_string(),
                status: str_at(e, "status").unwrap_or("pending").to_string(),
            })
        })
        .collect()
}

fn text_of(block: &Value) -> Option<&str> {
    (str_at(block, "type") == Some("text"))
        .then(|| str_at(block, "text"))
        .flatten()
}

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chunks_map_to_their_text_kind() {
        let p = json!({"sessionId": "s", "update": {"sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "Hi"}}});
        assert_eq!(
            session_update(&p),
            Update::Text(TextKind::Assistant, "Hi".into())
        );
        let image = json!({"update": {"sessionUpdate": "agent_thought_chunk",
            "content": {"type": "image", "data": ""}}});
        assert_eq!(session_update(&image), Update::Ignored);
    }

    #[test]
    fn available_commands_list_name_and_description() {
        let p = json!({"update": {"sessionUpdate": "available_commands_update",
        "availableCommands": [
            {"name": "review", "description": "Review changes", "input": {"hint": "path"}},
            {"name": "/compact"},
            {"description": "nameless"}
        ]}});
        let commands = available_commands(&p).unwrap();
        assert_eq!(
            commands,
            vec![
                AcpCommand {
                    name: "review".into(),
                    description: "Review changes".into()
                },
                AcpCommand {
                    name: "compact".into(),
                    description: String::new()
                },
            ]
        );
        let other = json!({"update": {"sessionUpdate": "plan", "entries": []}});
        assert_eq!(available_commands(&other), None);
    }

    #[test]
    fn a_tool_call_carries_its_command_diff_and_output() {
        let p = json!({"update": {"sessionUpdate": "tool_call", "toolCallId": "t1",
        "title": "Edit a.rs", "kind": "edit", "status": "pending",
        "locations": [{"path": "/r/a.rs"}],
        "content": [
            {"type": "diff", "path": "/r/a.rs", "oldText": "a", "newText": "b"},
            {"type": "content", "content": {"type": "text", "text": "done"}}
        ]}});
        let Update::Tool(patch) = session_update(&p) else {
            panic!("not a tool")
        };
        assert_eq!(patch.id, "t1");
        assert_eq!(patch.detail.as_deref(), Some("/r/a.rs"));
        assert_eq!(patch.diff.unwrap().new, "b");
        assert_eq!(patch.output.as_deref(), Some("done"));
    }

    #[test]
    fn an_update_changes_only_the_fields_it_names() {
        let mut tool = AcpTool {
            id: "t1".into(),
            title: "Run tests".into(),
            status: "pending".into(),
            ..Default::default()
        };
        let p = json!({"update": {"sessionUpdate": "tool_call_update", "toolCallId": "t1",
            "status": "completed"}});
        let Update::Tool(patch) = session_update(&p) else {
            panic!("not a tool")
        };
        apply_patch(&mut tool, patch);
        assert_eq!(tool.title, "Run tests");
        assert_eq!(tool.status, "completed");
    }

    #[test]
    fn permission_keeps_option_ids_beside_their_names() {
        let p = json!({"sessionId": "s", "toolCall": {"toolCallId": "t1", "title": "Run pnpm test",
        "rawInput": {"command": "pnpm test"}},
        "options": [
            {"optionId": "allow-once", "name": "Allow", "kind": "allow_once"},
            {"optionId": "reject-once", "name": "Reject", "kind": "reject_once"}
        ]});
        let permission = permission(&p);
        assert_eq!(permission.request.kind, FeedbackKind::Permission);
        assert_eq!(permission.request.title, "Run pnpm test");
        assert_eq!(permission.request.detail.as_deref(), Some("pnpm test"));
        assert_eq!(permission.request.options, ["Allow", "Reject"]);
        assert_eq!(permission.option_ids, ["allow-once", "reject-once"]);
    }

    #[test]
    fn permission_carries_a_long_command_whole() {
        let command = format!("echo {}", "x".repeat(995));
        let p = json!({"sessionId": "s", "toolCall": {"toolCallId": "t1", "title": "Run",
        "rawInput": {"command": command}}, "options": []});
        assert_eq!(
            permission(&p).request.detail.as_deref(),
            Some(command.as_str())
        );
    }
}
