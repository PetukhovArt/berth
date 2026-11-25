use crate::acp::driver::shape::{self, ALLOW_ALWAYS, ALLOW_ONCE};
use crate::acp::driver::translator::str_at;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub const ASK_USER: &str = "AskUserQuestion";
pub const TODO_WRITE: &str = "TodoWrite";
pub const TASK_CREATE: &str = "TaskCreate";
pub const TASK_UPDATE: &str = "TaskUpdate";

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

#[derive(Default)]
pub struct Tasks {
    items: Vec<(String, String, String)>,
    creating: HashMap<String, String>,
}

impl Tasks {
    pub fn creating(&mut self, tool_id: &str, input: &Value) {
        let subject = str_at(input, "subject").unwrap_or_default();
        self.creating
            .insert(tool_id.to_string(), subject.to_string());
    }

    pub fn created(&mut self, tool_id: &str, failed: bool, content: &Value) -> bool {
        let Some(subject) = self.creating.remove(tool_id) else {
            return false;
        };
        let text = result_text(content);
        let id = text
            .strip_prefix("Task #")
            .map(|rest| {
                rest.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
            })
            .filter(|id| !failed && !id.is_empty());
        if let Some(id) = id {
            self.items.push((id, subject, "pending".into()));
            return true;
        }
        false
    }

    pub fn update(&mut self, input: &Value) -> bool {
        let Some(at) = self
            .items
            .iter()
            .position(|(id, ..)| Some(id.as_str()) == str_at(input, "taskId"))
        else {
            return false;
        };
        if str_at(input, "status") == Some("deleted") {
            self.items.remove(at);
            return true;
        }
        if let Some(subject) = str_at(input, "subject") {
            self.items[at].1 = subject.to_string();
        }
        if let Some(status) = str_at(input, "status") {
            self.items[at].2 = status.to_string();
        }
        true
    }

    pub fn plan(&self) -> Value {
        shape::plan_update(
            self.items
                .iter()
                .map(|(_, subject, status)| (subject.as_str(), status.as_str())),
        )
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.creating.clear();
    }
}

pub fn result_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| str_at(block, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub fn result_update(id: &str, failed: bool, content: &Value) -> Value {
    let status = if failed { "failed" } else { "completed" };
    shape::tool_update(id, status, &result_text(content))
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
