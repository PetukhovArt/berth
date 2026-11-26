use super::driver::{MCP_CONTROL, MCP_STATUS};
use super::AcpLive;
use crate::registry::Daemon;
use anyhow::{anyhow, ensure, Result};
use berth_core::acp::{McpAccess, McpAction, McpServer};
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
            error: None,
            auth_url: None,
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

    pub async fn acp_mcp_control(
        self: &Arc<Self>,
        agent_id: &AgentId,
        server: &str,
        action: McpAction,
    ) -> Result<()> {
        let live = self.acp_ensure(agent_id)?;
        ensure!(
            live.state.lock().unwrap().caps.mcp == McpAccess::Toggle,
            "this agent cannot manage its MCP servers"
        );
        let sid = live.session_id().await?;
        let params = json!({"sessionId": sid, "server": server, "action": action});
        let outcome = live.conn.request(MCP_CONTROL, params).await;
        if let Err(error) = &outcome {
            tracing::debug!(agent = %agent_id, server, error = %error, "MCP control failed");
        }
        self.acp_refresh_mcp(agent_id).await?;
        let auth_url = outcome
            .ok()
            .and_then(|reply| reply.get("authUrl")?.as_str().map(str::to_string));
        if let Some(url) = auth_url {
            let mut state = live.state.lock().unwrap();
            if let Some(found) = state.mcp.iter_mut().find(|s| s.name == server) {
                found.auth_url = Some(url);
            }
            drop(state);
            self.acp_push_state(agent_id, &live);
        }
        Ok(())
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
