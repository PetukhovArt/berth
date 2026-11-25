use crate::acp::driver::shape;
use crate::acp::driver::translator::{str_at, text_at};
use serde_json::{json, Value};

pub const COMMAND_APPROVAL: &str = "item/commandExecution/requestApproval";
pub const FILE_APPROVAL: &str = "item/fileChange/requestApproval";
pub const PERMISSIONS_APPROVAL: &str = "item/permissions/requestApproval";

fn status_of(item: &Value) -> &'static str {
    match text_at(item, "status") {
        "completed" => "completed",
        "failed" | "declined" => "failed",
        _ => "in_progress",
    }
}

fn first_change(item: &Value) -> Option<&Value> {
    item.get("changes")
        .and_then(Value::as_array)
        .and_then(|changes| changes.first())
}

fn split_unified(diff: &str) -> (String, String) {
    let (mut old, mut new) = (Vec::new(), Vec::new());
    let mut in_hunk = false;
    for line in diff.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk {
            match line.split_at_checked(1) {
                Some(("-", rest)) => old.push(rest),
                Some(("+", rest)) => new.push(rest),
                Some((" ", rest)) => {
                    old.push(rest);
                    new.push(rest);
                }
                _ => {}
            }
        }
    }
    (old.join("\n"), new.join("\n"))
}

fn diff_of(change: &Value) -> Value {
    let (path, text) = (text_at(change, "path"), text_at(change, "diff"));
    match text_at(&change["kind"], "type") {
        "add" => json!({"type": "diff", "path": path, "newText": text}),
        "delete" => json!({"type": "diff", "path": path, "oldText": text, "newText": ""}),
        _ => {
            let (old, new) = split_unified(text);
            json!({"type": "diff", "path": path, "oldText": old, "newText": new})
        }
    }
}

fn file_call(item: &Value) -> Value {
    let count = item
        .get("changes")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let change = first_change(item);
    let path = change.map(|change| text_at(change, "path"));
    let (title, kind) = match change.map(|change| text_at(&change["kind"], "type")) {
        _ if count > 1 => (format!("Edit {count} files"), "edit"),
        Some("add") => ("Write".to_string(), "edit"),
        Some("delete") => ("Delete".to_string(), "delete"),
        _ => ("Edit".to_string(), "edit"),
    };
    let mut call = json!({
        "toolCallId": text_at(item, "id"),
        "title": title,
        "kind": kind,
        "rawInput": {"path": path},
        "locations": path.map(|path| vec![json!({"path": path})]).unwrap_or_default(),
    });
    if let Some(change) = change {
        call["content"] = json!([diff_of(change)]);
    }
    call
}

fn command_call(item: &Value) -> Value {
    json!({
        "toolCallId": text_at(item, "id"),
        "title": "Command",
        "kind": "execute",
        "rawInput": {"command": text_at(item, "command"), "cwd": text_at(item, "cwd")},
        "locations": [],
    })
}

fn call_of(item: &Value) -> Option<Value> {
    let id = text_at(item, "id");
    match text_at(item, "type") {
        "commandExecution" => Some(command_call(item)),
        "fileChange" => Some(file_call(item)),
        "mcpToolCall" => Some(json!({
            "toolCallId": id,
            "title": format!("{}.{}", text_at(item, "server"), text_at(item, "tool")),
            "kind": "other",
            "rawInput": item["arguments"],
            "locations": [],
        })),
        "webSearch" => Some(json!({
            "toolCallId": id,
            "title": "Web search",
            "kind": "fetch",
            "rawInput": {"query": text_at(item, "query")},
            "locations": [],
        })),
        _ => None,
    }
}

pub fn started(item: &Value) -> Option<Value> {
    let mut call = call_of(item)?;
    call["sessionUpdate"] = json!("tool_call");
    call["status"] = json!("in_progress");
    Some(call)
}

pub fn pending(call: &Value) -> Value {
    let mut call = call.clone();
    call["status"] = json!("pending");
    call
}

pub fn requested(method: &str, params: &Value) -> Value {
    let id = text_at(params, "itemId");
    let reason = str_at(params, "reason");
    let mut call = match method {
        COMMAND_APPROVAL => {
            command_call(&json!({"id": id, "command": params["command"], "cwd": params["cwd"]}))
        }
        FILE_APPROVAL => json!({
            "toolCallId": id, "title": "Edit", "kind": "edit",
            "rawInput": {}, "locations": [],
        }),
        _ => json!({
            "toolCallId": id, "title": "Extra permissions", "kind": "other",
            "rawInput": {"permissions": params["permissions"]}, "locations": [],
        }),
    };
    call["status"] = json!("pending");
    if let Some(reason) = reason {
        call["rawInput"]["reason"] = json!(reason);
    }
    call
}

fn mcp_text(item: &Value) -> String {
    if let Some(message) = item.pointer("/error/message").and_then(Value::as_str) {
        return message.to_string();
    }
    item.pointer("/result/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|block| str_at(block, "text"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn finished(item: &Value, streamed_output: Option<&str>) -> Option<Value> {
    let id = text_at(item, "id");
    let mut status = status_of(item);
    let update = match text_at(item, "type") {
        "commandExecution" => {
            if item
                .get("exitCode")
                .and_then(Value::as_i64)
                .is_some_and(|code| code != 0)
            {
                status = "failed";
            }
            let output = str_at(item, "aggregatedOutput").or(streamed_output);
            shape::tool_update(id, status, output.unwrap_or_default())
        }
        "fileChange" => shape::tool_update(id, status, ""),
        "mcpToolCall" => shape::tool_update(id, status, &mcp_text(item)),
        "webSearch" => {
            let mut update = shape::tool_update(id, status, "");
            update["title"] = json!(format!("Web search: {}", text_at(item, "query")));
            update
        }
        _ => return None,
    };
    Some(update)
}

pub fn plan(params: &Value) -> Value {
    shape::plan_update(
        params
            .get("plan")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|step| {
                let status = match text_at(step, "status") {
                    "inProgress" => "in_progress",
                    "completed" => "completed",
                    _ => "pending",
                };
                Some((str_at(step, "step")?, status))
            }),
    )
}
