use crate::context::Touching;
use crate::presence::Present;
use crate::store::{ChannelSummary, Device, Event, Mention, KIND_MESSAGE};
use crate::stream::Subscription;
use crate::tls::Endpoint;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct HubClient {
    endpoint: Endpoint,
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
        Self::trusting(url, token, None)
    }

    pub fn trusting(url: &str, token: &str, extra_ca: Option<&Path>) -> Result<Self> {
        Ok(Self {
            endpoint: Endpoint::parse(url, extra_ca)?,
            token: token.to_string(),
        })
    }

    async fn call(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let mut stream = self.endpoint.connect().await?;
        let payload = body.map(|b| b.to_string()).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            self.endpoint.authority,
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).await?;
        let mut raw = Vec::new();
        if let Err(e) = stream.read_to_end(&mut raw).await {
            let closed_without_tls_goodbye = e.kind() == std::io::ErrorKind::UnexpectedEof;
            if !closed_without_tls_goodbye || raw.is_empty() {
                return Err(e.into());
            }
        }
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
        Self::new(url, "")?.pair_device(name).await
    }

    pub async fn pair_device(&self, name: &str) -> Result<(String, String)> {
        let v = self
            .call("POST", "/pair", Some(json!({ "name": name })))
            .await?;
        Ok((
            v["id"].as_str().unwrap_or("").into(),
            v["token"].as_str().unwrap_or("").into(),
        ))
    }

    pub async fn pending(&self) -> Result<Value> {
        self.call("GET", "/admin/pending", None).await
    }

    pub async fn approve(&self, id: &str, scopes: &[&str]) -> Result<()> {
        self.call(
            "POST",
            "/admin/approve",
            Some(json!({ "id": id, "scopes": scopes })),
        )
        .await?;
        Ok(())
    }

    pub async fn devices(&self) -> Result<Vec<Device>> {
        Ok(serde_json::from_value(
            self.call("GET", "/admin/devices", None).await?,
        )?)
    }

    pub async fn revoke(&self, id: &str) -> Result<()> {
        self.call("POST", "/admin/revoke", Some(json!({ "id": id })))
            .await?;
        Ok(())
    }

    pub async fn agent(&self, name: &str) -> Result<String> {
        self.register_agent(name, None).await
    }

    pub async fn register_agent(&self, name: &str, display: Option<&str>) -> Result<String> {
        let v = self
            .call(
                "POST",
                "/agents",
                Some(json!({ "name": name, "display": display })),
            )
            .await?;
        Ok(v["id"].as_str().unwrap_or("").into())
    }

    pub async fn create_channel(&self, key: &str) -> Result<()> {
        self.call("POST", "/channels", Some(json!({ "key": key })))
            .await?;
        Ok(())
    }

    pub async fn add_member(&self, key: &str, member: &str) -> Result<()> {
        self.call(
            "POST",
            "/channels/members",
            Some(json!({ "key": key, "member": member })),
        )
        .await?;
        Ok(())
    }

    pub async fn channels(&self) -> Result<Vec<ChannelSummary>> {
        Ok(serde_json::from_value(
            self.call("GET", "/channels", None).await?,
        )?)
    }

    pub async fn post(
        &self,
        channel: &str,
        content: &str,
        as_agent: Option<&str>,
    ) -> Result<Event> {
        self.publish(channel, KIND_MESSAGE, content, &[], as_agent)
            .await
    }

    pub async fn publish(
        &self,
        channel: &str,
        kind: u32,
        content: &str,
        tags: &[Vec<String>],
        as_agent: Option<&str>,
    ) -> Result<Event> {
        let v = self
            .call(
                "POST",
                "/events",
                Some(json!({
                    "channel": channel,
                    "kind": kind,
                    "content": content,
                    "tags": tags,
                    "as_agent": as_agent,
                })),
            )
            .await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn me(&self) -> Result<String> {
        let v = self.call("GET", "/me", None).await?;
        v["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("the hub did not say who this device is"))
    }

    pub async fn mentions(&self, after: i64) -> Result<Vec<Mention>> {
        let path = format!("/mentions?after={after}");
        Ok(serde_json::from_value(
            self.call("GET", &path, None).await?,
        )?)
    }

    pub async fn read(
        &self,
        channel: &str,
        after: i64,
        as_agent: Option<&str>,
    ) -> Result<Vec<Event>> {
        let mut path = format!("/events?channel={}&after={after}", percent_encode(channel));
        if let Some(a) = as_agent {
            path.push_str(&format!("&as_agent={}", percent_encode(a)));
        }
        Ok(serde_json::from_value(
            self.call("GET", &path, None).await?,
        )?)
    }

    pub async fn subscribe(&self, after: Option<i64>) -> Result<Subscription> {
        Subscription::open(&self.endpoint, &self.token, after).await
    }

    pub async fn report_presence(&self, channels: &HashMap<String, Vec<String>>) -> Result<()> {
        self.call("POST", "/presence", Some(json!({ "channels": channels })))
            .await?;
        Ok(())
    }

    pub async fn post_note(
        &self,
        channel: &str,
        content: &str,
        as_agent: Option<&str>,
    ) -> Result<Event> {
        let v = self
            .call(
                "POST",
                "/notes",
                Some(json!({ "channel": channel, "content": content, "as_agent": as_agent })),
            )
            .await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn notes(&self) -> Result<Vec<Event>> {
        Ok(serde_json::from_value(
            self.call("GET", "/notes", None).await?,
        )?)
    }

    pub async fn report_touched(
        &self,
        channels: &HashMap<String, BTreeMap<String, Vec<String>>>,
    ) -> Result<()> {
        self.call("POST", "/touched", Some(json!({ "channels": channels })))
            .await?;
        Ok(())
    }

    pub async fn touched(&self, channel: &str) -> Result<Vec<Touching>> {
        let path = format!("/touched?channel={}", percent_encode(channel));
        Ok(serde_json::from_value(
            self.call("GET", &path, None).await?,
        )?)
    }

    pub async fn presence(&self, channel: &str) -> Result<Vec<Present>> {
        let path = format!("/presence?channel={}", percent_encode(channel));
        Ok(serde_json::from_value(
            self.call("GET", &path, None).await?,
        )?)
    }
}
