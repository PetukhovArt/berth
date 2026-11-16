use anyhow::Result;
use berth_core::{AgentId, Project};
use serde_json::Value;

use crate::registry::Daemon;

impl Daemon {
    pub(crate) fn acp_shared_mcp_servers(&self, agent: &AgentId, http: bool) -> Vec<Value> {
        match self.project_of_agent(agent) {
            Ok(Some(project)) => super::acp_mcp_servers(&project, http),
            Ok(None) => Vec::new(),
            Err(e) => {
                tracing::warn!(agent = %agent, error = %format!("{e:#}"), "finding the agent's project failed");
                Vec::new()
            }
        }
    }

    fn project_of_agent(&self, agent: &AgentId) -> Result<Option<Project>> {
        let Some(agent) = self.store.get_agent(agent)? else {
            return Ok(None);
        };
        let Some(worktree) = self.store.get_worktree(&agent.worktree_id)? else {
            return Ok(None);
        };
        self.store.get_project(&worktree.project_id)
    }
}
