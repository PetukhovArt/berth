use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use berth_core::{AgentId, SessionRef};

use crate::registry::Daemon;
use crate::status::HookEvent;

pub const CLAUDE_END_GUIDANCE: &str = "[berth] When the user asks you to end, close or kill this \
session or its agents (\"kill session\", \"end session\", \"закрой сессию\", \"закрой агентов\"), run \
this shell command exactly once:\n\n  berth end-session\n\nberth ends this session once your current \
turn ends and moves it to Ended in the Sessions tree, where opening it resumes the conversation; \
later it moves to the Archive. It ends only this session, never another one. Run it only when the user asks. When the command succeeds, end your \
turn at once with a one-line goodbye, passing on any warning it printed, and make no further tool \
calls. If it fails, report the error.";

const TRANSCRIPT_GRACE: Duration = Duration::from_secs(3);

pub(crate) fn is_turn_end(event: &HookEvent) -> bool {
    match event {
        HookEvent::Stop => true,
        HookEvent::Notification { notification_type } => {
            notification_type.as_deref() == Some("idle_prompt")
        }
        _ => false,
    }
}

impl Daemon {
    pub fn end_self(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if agent.archived {
            bail!("agent is archived");
        }
        if !self.is_alive(&SessionRef::Agent(id.clone())) {
            return self.end_agent(id);
        }
        self.pending_ends.lock().unwrap().insert(id.clone());
        Ok(())
    }

    pub fn end_agent(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        self.pending_ends.lock().unwrap().remove(id);
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.store.set_agent_ended(id, true)?;
        self.broadcast_agent(id)?;
        Ok(())
    }

    pub fn complete_pending_end(self: &Arc<Self>, id: &AgentId, event: &HookEvent) {
        if !is_turn_end(event) || !self.pending_ends.lock().unwrap().remove(id) {
            return;
        }
        let daemon = self.clone();
        let id = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(TRANSCRIPT_GRACE).await;
            if let Err(e) = daemon.end_agent(&id) {
                tracing::warn!(agent = %id, error = %e, "end at turn end failed");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::{seed_agent, seed_projects, seed_worktree, test_daemon};

    fn ended(daemon: &Daemon, id: &AgentId) -> bool {
        daemon.store.get_agent(id).unwrap().unwrap().is_ended()
    }

    #[tokio::test]
    async fn a_stopped_session_ends_at_once_and_a_pending_one_after_its_turn_end() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_agent(&daemon, "a1", "root", None);
        seed_agent(&daemon, "a2", "root", None);
        let (a1, a2) = (AgentId("a1".into()), AgentId("a2".into()));

        daemon.end_self(&a2).unwrap();
        assert!(ended(&daemon, &a2), "a stopped session ends at once");
        assert!(
            !daemon.store.get_agent(&a2).unwrap().unwrap().archived,
            "ending is not archiving"
        );
        daemon.archive_agent(&a2).unwrap();
        assert!(daemon.end_self(&a2).is_err(), "an archived row is refused");

        daemon.pending_ends.lock().unwrap().insert(a1.clone());
        daemon.complete_pending_end(
            &a1,
            &HookEvent::PostToolUse {
                tool_name: Some("Bash".into()),
                subagent_id: None,
                detail: None,
            },
        );
        assert!(!ended(&daemon, &a1), "a tool hook is not a turn end");
        daemon.complete_pending_end(&a1, &HookEvent::Stop);
        assert!(
            !ended(&daemon, &a1),
            "the CLI gets time to write its last answer"
        );
        tokio::time::sleep(TRANSCRIPT_GRACE + Duration::from_millis(500)).await;
        assert!(ended(&daemon, &a1));
        assert!(daemon.pending_ends.lock().unwrap().is_empty());
    }
}
