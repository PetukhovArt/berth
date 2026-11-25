use serde_json::Value;

pub enum Piece<'a> {
    Text(String),
    Image { mime: &'a str, data: &'a str },
}

pub fn pieces(blocks: &[Value]) -> impl Iterator<Item = Piece<'_>> {
    blocks.iter().filter_map(piece)
}

fn piece(block: &Value) -> Option<Piece<'_>> {
    let text = |key: &str| block.get(key).and_then(Value::as_str);
    match text("type")? {
        "text" => Some(Piece::Text(text("text")?.to_string())),
        "image" => Some(Piece::Image {
            mime: text("mimeType")?,
            data: text("data")?,
        }),
        "resource_link" => Some(Piece::Text(format!("@{}", text("name").or(text("uri"))?))),
        "resource" => {
            let resource = block.get("resource")?;
            let body = resource.get("text").and_then(Value::as_str)?;
            let uri = resource
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("file");
            let name = uri.rsplit('/').next().unwrap_or(uri);
            Some(Piece::Text(format!("{name}:\n```\n{body}\n```")))
        }
        _ => None,
    }
}
