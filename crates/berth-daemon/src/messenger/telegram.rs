use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use super::{curl, curl_line};
use crate::config::Config;

const API: &str = "https://api.telegram.org";
const CALL_SECS: u64 = 20;
const FILE_SECS: u64 = 90;
const POLL_SLACK_SECS: u64 = 15;

#[derive(Clone)]
pub(crate) struct Bot {
    token: String,
}

#[derive(Clone)]
pub(crate) struct Link {
    pub bot: Bot,
    pub chat: Value,
}

pub(crate) enum Upload {
    Voice(PathBuf),
    Audio(PathBuf),
}

impl Link {
    pub fn from_config(config: &Config) -> Option<Self> {
        let token = config.messenger_telegram_token.as_deref()?.trim();
        let chat = config.messenger_telegram_chat.clone()?;
        (!token.is_empty()).then(|| Self {
            bot: Bot {
                token: token.to_owned(),
            },
            chat,
        })
    }

    fn chat_field(&self) -> String {
        match &self.chat {
            Value::String(text) => text.trim().to_owned(),
            other => other.to_string(),
        }
    }

    pub async fn send_text(&self, text: &str, reply_to: Option<i64>) -> Result<i64> {
        let mut body = json!({ "chat_id": self.chat, "text": text });
        if let Some(message) = reply_to {
            body["reply_parameters"] =
                json!({ "message_id": message, "allow_sending_without_reply": true });
        }
        let config = curl_line("url", &self.bot.method("sendMessage"))
            + &curl_line("header", "Content-Type: application/json")
            + &curl_line("data", &body.to_string());
        message_id(&self.bot.call(&config, CALL_SECS).await?)
    }

    pub async fn send_upload(&self, upload: &Upload, reply_to: Option<i64>) -> Result<i64> {
        let (method, field, path) = match upload {
            Upload::Voice(path) => ("sendVoice", "voice", path),
            Upload::Audio(path) => ("sendAudio", "audio", path),
        };
        let mut config = curl_line("url", &self.bot.method(method))
            + &curl_line("form-string", &format!("chat_id={}", self.chat_field()))
            + &curl_line("form", &format!("{field}=@{}", path.display()));
        if let Some(message) = reply_to {
            let reply = json!({ "message_id": message, "allow_sending_without_reply": true });
            config += &curl_line("form-string", &format!("reply_parameters={reply}"));
        }
        message_id(&self.bot.call(&config, FILE_SECS).await?)
    }
}

fn message_id(result: &Value) -> Result<i64> {
    result["message_id"]
        .as_i64()
        .context("Telegram did not return a message id")
}

impl Bot {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn id(&self) -> Option<i64> {
        self.token.split(':').next()?.parse().ok()
    }

    fn method(&self, name: &str) -> String {
        format!("{API}/bot{}/{name}", self.token)
    }

    async fn call(&self, config: &str, max_secs: u64) -> Result<Value> {
        let body = curl(config, max_secs).await?;
        let reply: Value = serde_json::from_slice(&body).context("Telegram sent no JSON")?;
        ensure!(
            reply["ok"].as_bool() == Some(true),
            "{}",
            reply["description"]
                .as_str()
                .unwrap_or("Telegram refused the call")
        );
        Ok(reply["result"].clone())
    }

    pub async fn updates(&self, offset: i64, wait_secs: u64) -> Result<Vec<Value>> {
        let url = format!(
            "{}?offset={offset}&timeout={wait_secs}&allowed_updates=%5B%22message%22%5D",
            self.method("getUpdates")
        );
        let result = self
            .call(&curl_line("url", &url), wait_secs + POLL_SLACK_SECS)
            .await?;
        Ok(result.as_array().cloned().unwrap_or_default())
    }

    pub async fn download(&self, file_id: &str, to: &Path) -> Result<()> {
        ensure!(
            !file_id.is_empty()
                && file_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "unexpected Telegram file id"
        );
        let file = self
            .call(
                &curl_line(
                    "url",
                    &format!("{}?file_id={file_id}", self.method("getFile")),
                ),
                CALL_SECS,
            )
            .await?;
        let file_path = file["file_path"]
            .as_str()
            .context("Telegram gave no file path for the voice note")?;
        let config = curl_line("url", &format!("{API}/file/bot{}/{file_path}", self.token))
            + &curl_line("output", &to.to_string_lossy());
        curl(&config, FILE_SECS).await?;
        Ok(())
    }
}
