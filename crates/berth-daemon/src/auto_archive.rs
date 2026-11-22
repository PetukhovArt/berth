use std::sync::Arc;
use std::time::Duration;

use berth_core::{Agent, AgentId};

use crate::config::Config;
use crate::registry::{epoch_ms, Daemon};

pub(crate) fn due_for_archive(agents: &[Agent], now_ms: i64, after: Duration) -> Vec<AgentId> {
    let after_ms = i64::try_from(after.as_millis()).unwrap_or(i64::MAX);
    agents
        .iter()
        .filter(|agent| agent.is_ended() && now_ms.saturating_sub(agent.ended_at) >= after_ms)
        .map(|agent| agent.id.clone())
        .collect()
}

impl Daemon {
    pub fn archive_stale_ended_sessions(self: &Arc<Self>) {
        let Some(after) = Config::load().session_archive_after() else {
            return;
        };
        let Ok((_, _, agents, _)) = self.store.load_tree() else {
            return;
        };
        for id in due_for_archive(&agents, epoch_ms(), after) {
            if let Err(e) = self.archive_agent(&id) {
                tracing::warn!(agent = %id, error = %e, "archiving an ended session failed");
            }
        }
    }

    pub(crate) fn ensure_not_ended(&self, id: &AgentId) -> anyhow::Result<()> {
        let ended = matches!(self.store.get_agent(id), Ok(Some(agent)) if agent.is_ended());
        anyhow::ensure!(!ended, "agent is ended");
        Ok(())
    }

    pub(crate) fn leave_ended(&self, id: &AgentId) {
        if !matches!(self.store.get_agent(id), Ok(Some(agent)) if agent.ended_at > 0) {
            return;
        }
        if let Err(e) = self.store.set_agent_ended(id, false) {
            tracing::warn!(agent = %id, error = %e, "leaving Ended failed");
            return;
        }
        let _ = self.broadcast_agent(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::{seed_agent, seed_projects, seed_worktree, test_daemon};

    const DAY_MS: i64 = 86_400_000;

    #[test]
    fn only_agents_ended_for_the_whole_period_and_not_archived_are_due() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_agent(&daemon, "base", "root", None);
        let base = daemon
            .store
            .get_agent(&AgentId("base".into()))
            .unwrap()
            .unwrap();
        let agent = |id: &str, ended_at: i64, archived: bool| Agent {
            id: AgentId(id.into()),
            ended_at,
            archived,
            ..base.clone()
        };
        let now = 100 * DAY_MS;
        let after = Duration::from_secs(7 * 86_400);
        let agents = [
            agent("old", now - 8 * DAY_MS, false),
            agent("exact", now - 7 * DAY_MS, false),
            agent("recent", now - 6 * DAY_MS, false),
            agent("live", 0, false),
            agent("archived", now - 30 * DAY_MS, true),
        ];
        let due: Vec<String> = due_for_archive(&agents, now, after)
            .iter()
            .map(|id| id.to_string())
            .collect();
        assert_eq!(due, ["old", "exact"]);
    }

    #[test]
    fn opening_an_ended_agent_or_restoring_it_clears_ended() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_agent(&daemon, "a1", "root", None);
        let id = AgentId("a1".into());
        let ended = |daemon: &Arc<Daemon>| daemon.store.get_agent(&id).unwrap().unwrap().is_ended();

        daemon.end_agent(&id).unwrap();
        assert!(ended(&daemon));
        daemon.leave_ended(&id);
        assert!(!ended(&daemon), "a spawned session is not Ended");

        daemon.end_agent(&id).unwrap();
        daemon.archive_agent(&id).unwrap();
        assert!(!ended(&daemon), "archived outranks ended");
        daemon.unarchive_agent(&id).unwrap();
        let agent = daemon.store.get_agent(&id).unwrap().unwrap();
        assert!(!agent.archived);
        assert_eq!(agent.ended_at, 0, "the restore action serves both groups");
    }
}
