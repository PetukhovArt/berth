use crate::acp::driver::blocks::{pieces, Piece};
use serde_json::{json, Value};

pub fn user_message(blocks: &[Value], uuid: Option<&str>) -> Value {
    let content: Vec<Value> = pieces(blocks)
        .map(|piece| match piece {
            Piece::Text(text) => json!({"type": "text", "text": text}),
            Piece::Image { mime, data } => json!({
                "type": "image",
                "source": {"type": "base64", "media_type": mime, "data": data},
            }),
        })
        .collect();
    let mut message = json!({"type": "user", "message": {"role": "user", "content": content}});
    if let Some(uuid) = uuid {
        message["uuid"] = json!(uuid);
    }
    message
}
