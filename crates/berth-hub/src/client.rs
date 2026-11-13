use crate::presence::Present;
use crate::store::{ChannelSummary, Event, Mention};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
pub struct HubClient {
    authority: String,
    token: String,
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl HubClient {
    pub fn new(url: &str, token: &str) -> Result<Self> {
        let authority = url
            .strip_prefix("http://")
            .ok_or_else(|| anyhow!("the hub url must start with http://"))?
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            authority,
            token: token.to_string(),
        })
    }

    async fn call(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let mut stream = TcpStream::connect(&self.authority)
            .await
            .with_context(|| format!("cannot reach the hub at {}", self.authority))?;
        let payload = body.map(|b| b.to_string()).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            self.authority,
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).await?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await?;
        let text = String::from_utf8_lossy(&raw);
        let (head, body) = text
            .split_once("\r\n\r\n")
            .ok_or_else(|| anyhow!("the hub sent no response"))?;
        let status: u16 = head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("the hub sent a malformed response"))?;
        if status >= 300 {
            bail!("hub: {} ({status})", body.trim());
        }
        Ok(serde_json::from_str(body)?)
    }

    pub async fn pair(url: &str, name: &str) -> Result<(String, String)> {
        let v = Self::new(url, "")?
            .call("POST", "/pair", Some(json!({ "name": name })))
            .await?;
        Ok((v["id"].as_str().unwrap_or("").into(), v["token"].as_str().unwrap_or("").into()))
    }

    pub async fn pending(&self) -> Result<Value> {
        self.call("GET", "/admin/pending", None).await
    }

    pub async fn approve(&self, id: &str, scopes: &[&str]) -> Result<()> {
        self.call("POST", "/admin/approve", Some(json!({ "id": id, "scopes": scopes })))
            .await?;
        Ok(())
    }

    pub async fn agent(&self, name: &str) -> Result<String> {
        let v = self.call("POST", "/agents", Some(json!({ "name": name }))).await?;
        Ok(v["id"].as_str().unwrap_or("").into())
    }

    pub async fn create_channel(&self, key: &str) -> Result<()> {
        self.call("POST", "/channels", Some(json!({ "key": key }))).await?;
        Ok(())
    }

    pub async fn add_member(&self, key: &str, member: &str) -> Result<()> {
        self.call("POST", "/channels/members", Some(json!({ "key": key, "member": member })))
            .await?;
        Ok(())
    }

    pub async fn channels(&self) -> Result<Vec<ChannelSummary>> {
        Ok(serde_json::from_value(self.call("GET", "/channels", None).await?)?)
    }

    pub async fn post(
        &self,
        channel: &str,
        content: &str,
        as_agent: Option<&str>,
    ) -> Result<Event> {
        let v = self
            .call(
                "POST",
                "/events",
                Some(json!({ "channel": channel, "content": content, "as_agent": as_agent })),
            )
            .await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn mentions(&self, after: i64) -> Result<Vec<Mention>> {
        let path = format!("/mentions?after={after}");
        Ok(serde_json::from_value(self.call("GET", &path, None).await?)?)
    }

    pub async fn read(&self, channel: &str, after: i64, as_agent: Option<&str>) -> Result<Vec<Event>> {
        let mut path = format!("/events?channel={}&after={after}", percent_encode(channel));
        if let Some(a) = as_agent {
            path.push_str(&format!("&as_agent={}", percent_encode(a)));
        }
        Ok(serde_json::from_value(self.call("GET", &path, None).await?)?)
    }

    pub async fn report_presence(&self, channels: &HashMap<String, Vec<String>>) -> Result<()> {
        self.call("POST", "/presence", Some(json!({ "channels": channels })))
            .await?;
        Ok(())
    }

    pub async fn presence(&self, channel: &str) -> Result<Vec<Present>> {
        let path = format!("/presence?channel={}", percent_encode(channel));
        Ok(serde_json::from_value(self.call("GET", &path, None).await?)?)
    }
}
