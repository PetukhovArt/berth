use crate::registry::{relocation_prompt, Daemon};
use anyhow::Result;
use berth_core::{Agent, AgentId, ServerEvent, Worktree};
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl Daemon {
    pub(crate) fn acp_take_queue(&self, id: &AgentId) -> Vec<String> {
        let Some(live) = self.acp.get(id) else {
            return Vec::new();
        };
        let cleared = std::mem::take(&mut live.state.lock().unwrap().queue);
        cleared
            .into_iter()
            .map(|queued| queued.prompt.text)
            .filter(|text| !text.is_empty())
            .collect()
    }

    pub(crate) fn acp_announce_cleared(&self, id: &AgentId, texts: Vec<String>) {
        if !texts.is_empty() {
            self.broadcast(ServerEvent::AcpQueueCleared {
                agent: id.clone(),
                texts,
            });
        }
    }

    pub(crate) fn acp_restart(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        self.acp.kill(id);
        self.acp_ensure(id)?;
        self.broadcast_agent(id)
    }

    pub(crate) fn acp_relocate(self: &Arc<Self>, agent: &Agent, target: &Worktree) {
        let id = &agent.id;
        let carried = self.acp.get(id).map(|live| {
            let queue = std::mem::take(&mut live.state.lock().unwrap().queue);
            (queue, live.next_queue_id.load(Ordering::SeqCst))
        });
        self.acp.kill(id);
        self.carry_claude_transcript(agent, target);
        let live = match self.acp_ensure(id) {
            Ok(live) => live,
            Err(error) => {
                tracing::warn!(agent = %id, error = %error, "restart after worktree relocation failed");
                return;
            }
        };
        if let Some((queue, next_id)) = carried {
            live.state.lock().unwrap().queue = queue;
            live.next_queue_id.store(next_id, Ordering::SeqCst);
        }
        let (daemon, id, prompt) = (self.clone(), id.clone(), relocation_prompt(target));
        tokio::spawn(async move {
            if let Err(error) = daemon.acp_instruct(&id, &prompt).await {
                tracing::warn!(agent = %id, error = %error, "relocation prompt failed");
            }
        });
    }
}
