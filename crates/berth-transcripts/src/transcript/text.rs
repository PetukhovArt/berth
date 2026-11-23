use serde_json::Value;

const ID_CHARS: usize = 100;
pub(super) const TEXT_CHARS: usize = 20_000;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_CHARS
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn owned(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

pub(super) fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut clipped: String = text.chars().take(max.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

pub(super) fn single_line(text: &str, max: usize) -> String {
    clip(&text.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

pub(super) fn content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => blocks.iter().find_map(|block| {
            matches!(
                block["type"].as_str(),
                Some("text" | "input_text" | "output_text")
            )
            .then(|| block["text"].as_str().map(str::to_owned))
            .flatten()
        }),
        _ => None,
    }
}

pub(super) fn tag_value<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim())
}

pub(super) fn command_line(text: &str) -> Option<String> {
    let name = tag_value(text, "command-name")?;
    let args = tag_value(text, "command-args").unwrap_or_default();
    Some(if args.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {args}")
    })
}

pub(super) fn prompt_of(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.starts_with("Caveat:") {
        return None;
    }
    if text.starts_with('<') {
        let args = tag_value(text, "command-args").filter(|args| !args.is_empty())?;
        let name = tag_value(text, "command-name").unwrap_or_default();
        return Some(format!("{name} {args}").trim().to_owned());
    }
    Some(text.to_owned())
}

pub(super) fn shown_user_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("<local-command-caveat>")
        || trimmed.starts_with("<local-command-stdout>")
        || trimmed.starts_with("<environment_context>")
        || trimmed.starts_with("<user_instructions>")
        || trimmed.starts_with("<task-notification>")
        || trimmed.starts_with("<system-reminder>")
        || trimmed.starts_with("Caveat:")
    {
        return None;
    }
    if trimmed.starts_with("<command-name>") || trimmed.starts_with("<command-message>") {
        return command_line(trimmed);
    }
    Some(clip(trimmed, TEXT_CHARS))
}
