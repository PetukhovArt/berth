use std::collections::BTreeMap;

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct McpServer {
    pub command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    pub url: Option<String>,
}

pub(super) enum RemoteShape {
    Typed,
    BareUrl,
}

impl McpServer {
    pub fn json(&self, remote: RemoteShape) -> Value {
        if let Some(url) = &self.url {
            return match remote {
                RemoteShape::Typed => json!({ "type": "http", "url": url }),
                RemoteShape::BareUrl => json!({ "url": url }),
            };
        }
        let mut entry = json!({ "command": self.command, "args": self.args });
        if !self.env.is_empty() {
            entry["env"] = json!(self.env);
        }
        entry
    }

    pub fn toml(&self) -> toml::Value {
        let mut table = toml::Table::new();
        if let Some(url) = &self.url {
            table.insert("url".into(), url.clone().into());
            return toml::Value::Table(table);
        }
        table.insert(
            "command".into(),
            self.command.clone().unwrap_or_default().into(),
        );
        table.insert(
            "args".into(),
            toml::Value::Array(self.args.iter().cloned().map(Into::into).collect()),
        );
        if !self.env.is_empty() {
            let env = self.env.iter().map(|(k, v)| (k.clone(), v.clone().into()));
            table.insert("env".into(), toml::Value::Table(env.collect()));
        }
        toml::Value::Table(table)
    }

    pub fn opencode(&self) -> Value {
        if let Some(url) = &self.url {
            return json!({ "type": "remote", "url": url });
        }
        let mut command = vec![self.command.clone().unwrap_or_default()];
        command.extend(self.args.iter().cloned());
        let mut entry = json!({ "type": "local", "command": command });
        if !self.env.is_empty() {
            entry["environment"] = json!(self.env);
        }
        entry
    }

    pub fn acp(&self, name: &str, http: bool) -> Option<Value> {
        if let Some(url) = &self.url {
            return http
                .then(|| json!({ "type": "http", "name": name, "url": url, "headers": [] }));
        }
        let env: Vec<Value> = self
            .env
            .iter()
            .map(|(k, v)| json!({ "name": k, "value": v }))
            .collect();
        Some(json!({
            "name": name,
            "command": self.command,
            "args": self.args,
            "env": env,
        }))
    }
}

pub(super) fn mcp_json(servers: Map<String, Value>) -> Result<Vec<u8>> {
    pretty(&json!({ "mcpServers": servers }))
}

pub(super) fn opencode_json(servers: Map<String, Value>) -> Result<Vec<u8>> {
    pretty(&json!({ "mcp": { "servers": servers } }))
}

pub(super) fn mcp_toml(servers: toml::Table) -> Result<Vec<u8>> {
    let mut config = toml::Table::new();
    config.insert("mcp_servers".into(), toml::Value::Table(servers));
    Ok(toml::to_string(&config)?.into_bytes())
}

pub(super) fn pretty(value: &Value) -> Result<Vec<u8>> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text.into_bytes())
}
