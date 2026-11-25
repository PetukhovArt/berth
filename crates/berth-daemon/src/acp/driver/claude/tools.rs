use serde_json::{json, Map, Value};

pub const ASK_USER: &str = "AskUserQuestion";
pub const TODO_WRITE: &str = "TodoWrite";
pub const ALLOW_ONCE: &str = "allow_once";
pub const ALLOW_ALWAYS: &str = "allow_always";
pub const REJECT_ONCE: &str = "reject_once";

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

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
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
    let entries: Vec<Value> = input
        .get("todos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|todo| {
            Some(json!({
                "content": str_at(todo, "content")?,
                "priority": "medium",
                "status": str_at(todo, "status").unwrap_or("pending"),
            }))
        })
        .collect();
    json!({"sessionUpdate": "plan", "entries": entries})
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
    let mut update = json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": id,
        "status": if failed { "failed" } else { "completed" },
    });
    if !text.is_empty() {
        update["content"] = json!([{"type": "content", "content": {"type": "text", "text": text}}]);
    }
    update
}

pub fn permission_params(session_id: &str, request: &Value) -> Value {
    let name = str_at(request, "tool_name").unwrap_or("tool");
    let input = request.get("input").cloned().unwrap_or(Value::Null);
    let always = request
        .get("permission_suggestions")
        .and_then(Value::as_array)
        .is_some_and(|suggestions| !suggestions.is_empty());
    let mut options = vec![json!({"optionId": ALLOW_ONCE, "name": "Allow", "kind": ALLOW_ONCE})];
    if always {
        options
            .push(json!({"optionId": ALLOW_ALWAYS, "name": "Always allow", "kind": ALLOW_ALWAYS}));
    }
    options.push(json!({"optionId": REJECT_ONCE, "name": "Deny", "kind": REJECT_ONCE}));
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
        "options": options,
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

fn question_id(index: usize) -> String {
    format!("q{index}")
}

pub fn elicitation_params(questions: &[Value]) -> Value {
    let mut properties = Map::new();
    for (index, question) in questions.iter().enumerate() {
        let mut choices: Vec<Value> = question
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| {
                let label = str_at(option, "label")?;
                Some(json!({
                    "const": label,
                    "title": label,
                    "description": str_at(option, "description"),
                }))
            })
            .collect();
        choices.push(json!({"type": "string"}));
        let header = str_at(question, "header").or(str_at(question, "question"));
        let schema = if question.get("multiSelect").and_then(Value::as_bool) == Some(true) {
            json!({"type": "array", "items": {"anyOf": choices}})
        } else {
            json!({"type": "string", "oneOf": choices})
        };
        let mut property = schema;
        property["title"] = json!(header);
        property["description"] = json!(str_at(question, "question"));
        properties.insert(question_id(index), property);
    }
    json!({
        "mode": "form",
        "message": questions.first().and_then(|q| str_at(q, "question")).unwrap_or("Claude has a question"),
        "requestedSchema": {"type": "object", "properties": properties},
    })
}

pub fn question_reply(questions: &[Value], content: &Value) -> Option<Value> {
    let mut answers = Map::new();
    for (index, question) in questions.iter().enumerate() {
        let given = content.get(question_id(index))?;
        let text = match given {
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            other => other.as_str()?.to_string(),
        };
        answers.insert(str_at(question, "question")?.to_string(), json!(text));
    }
    Some(Value::Object(answers))
}
