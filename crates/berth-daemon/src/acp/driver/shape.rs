use super::translator::str_at;
use serde_json::{json, Map, Value};

pub const ALLOW_ONCE: &str = "allow_once";
pub const ALLOW_ALWAYS: &str = "allow_always";
pub const REJECT_ONCE: &str = "reject_once";

pub fn permission_options(always: bool) -> Value {
    let mut options = vec![json!({"optionId": ALLOW_ONCE, "name": "Allow", "kind": ALLOW_ONCE})];
    if always {
        options
            .push(json!({"optionId": ALLOW_ALWAYS, "name": "Always allow", "kind": ALLOW_ALWAYS}));
    }
    options.push(json!({"optionId": REJECT_ONCE, "name": "Deny", "kind": REJECT_ONCE}));
    Value::Array(options)
}

fn question_id(index: usize) -> String {
    format!("q{index}")
}

pub fn elicitation_params(questions: &[Value], fallback: &str) -> Value {
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
        let mut property = if question.get("multiSelect").and_then(Value::as_bool) == Some(true) {
            json!({"type": "array", "items": {"anyOf": choices}})
        } else {
            json!({"type": "string", "oneOf": choices})
        };
        property["title"] = json!(header);
        property["description"] = json!(str_at(question, "question"));
        properties.insert(question_id(index), property);
    }
    json!({
        "mode": "form",
        "message": questions.first().and_then(|q| str_at(q, "question")).unwrap_or(fallback),
        "requestedSchema": {"type": "object", "properties": properties},
    })
}

pub fn answer_values(content: &Value, index: usize) -> Option<Vec<String>> {
    match content.get(question_id(index))? {
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        ),
        other => Some(vec![other.as_str()?.to_string()]),
    }
}

pub fn tool_update(id: &str, status: &str, text: &str) -> Value {
    let mut update = json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": id,
        "status": status,
    });
    if !text.is_empty() {
        update["content"] = json!([{"type": "content", "content": {"type": "text", "text": text}}]);
    }
    update
}

pub fn plan_update<'a>(steps: impl Iterator<Item = (&'a str, &'a str)>) -> Value {
    let entries: Vec<Value> = steps
        .map(
            |(content, status)| json!({"content": content, "priority": "medium", "status": status}),
        )
        .collect();
    json!({"sessionUpdate": "plan", "entries": entries})
}
