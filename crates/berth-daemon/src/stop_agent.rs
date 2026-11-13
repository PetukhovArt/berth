use anyhow::Result;
use berth_core::{AgentId, SessionRef};
use std::sync::Arc;

use crate::registry::Daemon;

impl Daemon {
    pub fn stop_agent(&self, id: &AgentId) -> Result<()> {
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.broadcast_agent(id)
    }

    pub async fn stop_agent_or_its_acp_turn(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        if !self.acp.is_live(id) {
            return self.stop_agent(id);
        }
        self.acp_stop(id).await;
        self.broadcast_agent(id)
    }
}
