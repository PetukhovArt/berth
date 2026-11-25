use super::driver::MCP_STATUS;
use super::AcpLive;
use crate::registry::Daemon;
use anyhow::{anyhow, Result};
use berth_core::acp::McpServer;
use berth_core::AgentId;
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) fn configured(servers: &[Value]) -> Vec<McpServer> {
    servers
        .iter()
        .filter_map(|server| server.get("name")?.as_str())
        .map(|name| McpServer {
            name: name.into(),
            status: None,
        })
        .collect()
}

impl Daemon {
    pub(super) fn acp_refresh_mcp_later(self: &Arc<Self>, agent_id: &AgentId, live: &AcpLive) {
        if !live.capabilities.lock().unwrap().mcp_status {
            return;
        }
        let daemon = self.clone();
        let agent_id = agent_id.clone();
        tokio::spawn(async move {
            if let Err(error) = daemon.acp_refresh_mcp(&agent_id).await {
                tracing::debug!(agent = %agent_id, error = %error, "MCP status not refreshed");
            }
        });
    }

    pub async fn acp_refresh_mcp(self: &Arc<Self>, agent_id: &AgentId) -> Result<()> {
        let Some(live) = self.acp.get(agent_id) else {
            return Ok(());
        };
        if !live.capabilities.lock().unwrap().mcp_status {
            return Ok(());
        }
        let sid = live.session_id().await?;
        let reply = live
            .conn
            .request(MCP_STATUS, json!({"sessionId": sid}))
            .await
            .map_err(|error| anyhow!("{error}"))?;
        live.state.lock().unwrap().mcp = serde_json::from_value(reply["servers"].clone())?;
        self.acp_push_state(agent_id, &live);
        Ok(())
    }
}
