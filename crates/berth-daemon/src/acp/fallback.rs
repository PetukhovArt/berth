use crate::agent;
use crate::pty::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::registry::Daemon;
use berth_core::{AgentId, Backend};
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl Daemon {
    pub(crate) fn acp_fall_back(
        self: &Arc<Self>,
        id: &AgentId,
        prompt: &str,
        reason: &str,
    ) -> bool {
        let Ok(Some(mut agent)) = self.store.get_agent(id) else {
            return false;
        };
        if agent.backend == Backend::Terminal || agent::terminal(agent.kind).is_none() {
            return false;
        }
        if let Err(error) = self.set_backend(id, Backend::Terminal, None) {
            tracing::warn!(agent = %id, error = %error, "fallback to the terminal failed");
            return false;
        }
        agent.backend = Backend::Terminal;
        self.acp_notice(
            id,
            format!("{reason}\nThis agent cannot run as a chat here and now runs in its terminal."),
        );
        if self.acp.skip_terminal_spawn.load(Ordering::SeqCst) {
            return true;
        }
        let Ok(Some(worktree)) = self.store.get_worktree(&agent.worktree_id) else {
            return true;
        };
        let prompt = (!prompt.is_empty()).then_some(prompt);
        if let Err(error) =
            self.spawn_agent_session_with(&agent, &worktree, DEFAULT_COLS, DEFAULT_ROWS, prompt)
        {
            tracing::warn!(agent = %id, error = %error, "terminal after the chat fallback failed");
        }
        let _ = self.broadcast_agent(id);
        true
    }
}
