use anyhow::Result;
use berth_core::{AgentId, SessionRef};

use crate::registry::Daemon;

impl Daemon {
    pub fn stop_agent(&self, id: &AgentId) -> Result<()> {
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.broadcast_agent(id)
    }
}
