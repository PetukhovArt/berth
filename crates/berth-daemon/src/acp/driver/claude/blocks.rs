use serde_json::{json, Value};

pub fn user_message(blocks: &[Value]) -> Value {
    let content: Vec<Value> = blocks.iter().filter_map(content_block).collect();
    json!({"type": "user", "message": {"role": "user", "content": content}})
}

fn content_block(block: &Value) -> Option<Value> {
    let text = |key: &str| block.get(key).and_then(Value::as_str);
    match text("type")? {
        "text" => Some(json!({"type": "text", "text": text("text")?})),
        "image" => Some(json!({
            "type": "image",
            "source": {"type": "base64", "media_type": text("mimeType")?, "data": text("data")?},
        })),
        "resource_link" => Some(json!({
            "type": "text",
            "text": format!("@{}", text("name").or(text("uri"))?),
        })),
        "resource" => {
            let resource = block.get("resource")?;
            let body = resource.get("text").and_then(Value::as_str)?;
            let uri = resource
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("file");
            let name = uri.rsplit('/').next().unwrap_or(uri);
            Some(json!({"type": "text", "text": format!("{name}:\n```\n{body}\n```")}))
        }
        _ => None,
    }
}
