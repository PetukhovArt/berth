use serde::Serialize;
use serde_json::{Map, Value};

use crate::settings::{berth_config_object, berth_config_update};

const TOKEN_KEY: &str = "messenger_telegram_token";
const CHAT_KEY: &str = "messenger_telegram_chat";
const URL_KEY: &str = "messenger_webhook_url";

#[derive(Debug, Serialize)]
pub struct ChatBridge {
    telegram_token: String,
    telegram_chat: String,
    webhook_url: String,
}

fn text_of(map: &Map<String, Value>, key: &str) -> String {
    match map.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

fn entry(key: &'static str, text: &str) -> (&'static str, Option<Value>) {
    let text = text.trim();
    (
        key,
        (!text.is_empty()).then(|| Value::String(text.to_owned())),
    )
}

pub async fn chat_bridge_read() -> Result<ChatBridge, String> {
    let map = berth_config_object(&berth_core::paths::config_path())?;
    Ok(ChatBridge {
        telegram_token: text_of(&map, TOKEN_KEY),
        telegram_chat: text_of(&map, CHAT_KEY),
        webhook_url: text_of(&map, URL_KEY),
    })
}

pub async fn chat_bridge_write(
    telegram_token: String,
    telegram_chat: String,
    webhook_url: String,
) -> Result<(), String> {
    let url = webhook_url.trim();
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("адрес вебхука должен начинаться с http:// или https://".to_owned());
    }
    berth_config_update(&[
        entry(TOKEN_KEY, &telegram_token),
        entry(CHAT_KEY, &telegram_chat),
        entry(URL_KEY, &webhook_url),
    ])
}
