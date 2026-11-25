use crate::acp::driver::shape::{self, ALLOW_ALWAYS, ALLOW_ONCE};
use crate::acp::driver::translator::str_at;
use serde_json::{json, Map, Value};

pub const ASK_USER: &str = "AskUserQuestion";
pub const TODO_WRITE: &str = "TodoWrite";

fn kind(tool: &str) -> &'static str {
    match tool {
        "Read" => "read",
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "edit",
        "Bash" | "PowerShell" => "execute",
        "Grep" | "Glob" => "search",
        "WebFetch" | "WebSearch" => "fetch",
        _ => "other",
    }
}

fn path_of(input: &Value) -> Option<&str> {
    ["file_path", "notebook_path", "path"]
        .iter()
        .find_map(|key| str_at(input, key))
}

fn diff_of(name: &str, input: &Value) -> Option<Value> {
    let path = path_of(input)?;
    match name {
        "Edit" => Some(json!({
            "type": "diff",
            "path": path,
            "oldText": str_at(input, "old_string")?,
            "newText": str_at(input, "new_string")?,
        })),
        "Write" => Some(json!({
            "type": "diff",
            "path": path,
            "newText": str_at(input, "content")?,
        })),
        _ => None,
    }
}

pub fn tool_call(id: &str, name: &str, input: &Value) -> Value {
    let mut call = json!({
        "sessionUpdate": "tool_call",
        "toolCallId": id,
        "title": name,
        "kind": kind(name),
        "status": "in_progress",
        "rawInput": input,
        "locations": path_of(input).map(|path| vec![json!({"path": path})]).unwrap_or_default(),
    });
    if let Some(diff) = diff_of(name, input) {
        call["content"] = json!([diff]);
    }
    call
}

pub fn plan(input: &Value) -> Value {
    shape::plan_update(
        input
            .get("todos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|todo| {
                Some((
                    str_at(todo, "content")?,
                    str_at(todo, "status").unwrap_or("pending"),
                ))
            }),
    )
}

pub fn result_update(id: &str, failed: bool, content: &Value) -> Value {
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| str_at(block, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    shape::tool_update(id, if failed { "failed" } else { "completed" }, &text)
}

pub fn permission_params(session_id: &str, request: &Value) -> Value {
    let name = str_at(request, "tool_name").unwrap_or("tool");
    let input = request.get("input").cloned().unwrap_or(Value::Null);
    let always = request
        .get("permission_suggestions")
        .and_then(Value::as_array)
        .is_some_and(|suggestions| !suggestions.is_empty());
    json!({
        "sessionId": session_id,
        "toolCall": {
            "toolCallId": str_at(request, "tool_use_id").unwrap_or_default(),
            "title": name,
            "kind": kind(name),
            "status": "pending",
            "rawInput": input,
            "locations": path_of(&input).map(|path| vec![json!({"path": path})]).unwrap_or_default(),
        },
        "options": shape::permission_options(always),
    })
}

pub fn permission_reply(option_id: Option<&str>, input: &Value, suggestions: &Value) -> Value {
    match option_id {
        Some(ALLOW_ONCE) => json!({"behavior": "allow", "updatedInput": input}),
        Some(ALLOW_ALWAYS) => {
            json!({"behavior": "allow", "updatedInput": input, "updatedPermissions": suggestions})
        }
        _ => json!({"behavior": "deny", "message": "The user denied this tool call."}),
    }
}

pub fn elicitation_params(questions: &[Value]) -> Value {
    shape::elicitation_params(questions, "Claude has a question")
}

pub fn question_reply(questions: &[Value], content: &Value) -> Option<Value> {
    let mut answers = Map::new();
    for (index, question) in questions.iter().enumerate() {
        let text = shape::answer_values(content, index)?.join(", ");
        answers.insert(str_at(question, "question")?.to_string(), json!(text));
    }
    Some(Value::Object(answers))
}
