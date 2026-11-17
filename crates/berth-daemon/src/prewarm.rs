use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use berth_core::{Agent, AgentId, AgentKind, AgentStatus, PrewarmInfo, SessionRef, WorktreeId};

use crate::pty::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::registry::Daemon;
use crate::status::HookEvent;

const PREWARM_MAX_AGE: Duration = Duration::from_secs(15 * 60);
const REBOOT_SPARE_OLDER_THAN: Duration = Duration::from_secs(10 * 60);
const SPARE_BOOT_BOUND: Duration = Duration::from_secs(8);
const PREWARM_HOOK_BUFFER_CAP: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    worktree: WorktreeId,
    kind: AgentKind,
    model: Option<String>,
    effort: Option<String>,
}

pub(crate) struct WarmSpare {
    pub(crate) agent_id: AgentId,
    target: Target,
    spawned_at: Instant,
    pub(crate) hooks_before_adoption: Vec<(HookEvent, Option<String>)>,
    input_ready: bool,
}

fn signals_input_ready(kind: AgentKind) -> bool {
    kind == AgentKind::Claude
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptedAdoption {
    TypeNow,
    WaitForReady(Duration),
    Cold,
}

fn prompted_adoption(
    kind: AgentKind,
    input_ready: bool,
    age: Duration,
    prompt: &str,
) -> PromptedAdoption {
    if !signals_input_ready(kind) || prompt.contains(['\0', '\x1b']) {
        PromptedAdoption::Cold
    } else if input_ready {
        PromptedAdoption::TypeNow
    } else if age < SPARE_BOOT_BOUND {
        PromptedAdoption::WaitForReady(SPARE_BOOT_BOUND - age)
    } else {
        PromptedAdoption::Cold
    }
}

impl WarmSpare {
    fn session(&self) -> SessionRef {
        SessionRef::Agent(self.agent_id.clone())
    }
}

#[derive(Default)]
pub(crate) struct PrewarmPool {
    spare: Mutex<Option<WarmSpare>>,
    newest_request: AtomicU64,
    input_ready: tokio::sync::Notify,
}

impl PrewarmPool {
    pub(crate) fn ticket(&self) -> u64 {
        self.newest_request.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_newest(&self, ticket: u64) -> bool {
        self.newest_request.load(Ordering::SeqCst) == ticket
    }

    fn install_returning_the_one_to_kill(
        &self,
        ticket: u64,
        spare: WarmSpare,
    ) -> Option<WarmSpare> {
        let mut slot = self.spare.lock().unwrap();
        if !self.is_newest(ticket) {
            return Some(spare);
        }
        slot.replace(spare)
    }

    fn take_if(&self, pred: impl FnOnce(&WarmSpare) -> bool) -> Option<WarmSpare> {
        let mut slot = self.spare.lock().unwrap();
        if slot.as_ref().is_some_and(pred) {
            slot.take()
        } else {
            None
        }
    }

    pub(crate) fn buffer_hook(
        &self,
        agent_id: &AgentId,
        event: HookEvent,
        session_id: Option<String>,
    ) {
        let mut slot = self.spare.lock().unwrap();
        let Some(spare) = slot.as_mut().filter(|s| &s.agent_id == agent_id) else {
            return;
        };
        let session_started = matches!(event, HookEvent::SessionStart { .. });
        if spare.hooks_before_adoption.len() >= PREWARM_HOOK_BUFFER_CAP {
            spare.hooks_before_adoption.remove(0);
        }
        spare.hooks_before_adoption.push((event, session_id));
        if session_started && signals_input_ready(spare.target.kind) {
            spare.input_ready = true;
            self.input_ready.notify_waiters();
        }
    }

    pub(crate) fn info(&self) -> Option<(AgentId, PrewarmInfo)> {
        self.spare.lock().unwrap().as_ref().map(|s| {
            (
                s.agent_id.clone(),
                PrewarmInfo {
                    worktree: s.target.worktree.clone(),
                    kind: s.target.kind,
                    model: s.target.model.clone(),
                },
            )
        })
    }
}

impl Daemon {
    pub async fn prewarm_agent(
        self: &Arc<Self>,
        ticket: u64,
        worktree_id: &WorktreeId,
        kind: AgentKind,
        model: Option<String>,
        effort: Option<String>,
    ) -> Result<()> {
        let config = crate::config::Config::load();
        if kind == AgentKind::Acp
            || !config.prewarm
            || crate::acp::providers::ensure_native_enabled(&config.providers, kind).is_err()
        {
            return Ok(());
        }
        let Some(worktree) = self.store.get_worktree(worktree_id)? else {
            return Ok(());
        };
        let target = Target {
            worktree: worktree_id.clone(),
            kind,
            model,
            effort,
        };
        let already_serves = |s: &WarmSpare| {
            s.target == target
                && s.spawned_at.elapsed() < REBOOT_SPARE_OLDER_THAN
                && self.is_alive(&s.session())
        };
        if self
            .prewarm
            .spare
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(already_serves)
            || !self.prewarm.is_newest(ticket)
        {
            return Ok(());
        }
        if let Some(old) = self.prewarm.take_if(|_| true) {
            self.kill_session(&old.session());
        }
        if !self.cli_available(kind).await {
            tracing::debug!(kind = kind.as_str(), "prewarm skipped: CLI not installed");
            return Ok(());
        }
        let agent = Agent {
            id: AgentId::generate(),
            worktree_id: worktree_id.clone(),
            name: "prewarm".into(),
            status: AgentStatus::Fresh,
            archived: false,
            archived_at: 0,
            unseen: false,
            kind,
            model: target.model.clone(),
            effort: target.effort.clone(),
            session_id: None,
            sort_order: 0,
            status_changed_at: 0,
            alive: false,
            suggested_prompt: None,
            provider_error: None,
            routed_from: None,
            route_suggestion: None,
            acp_provider: None,
        };
        self.spawn_agent_session(&agent, &worktree, DEFAULT_COLS, DEFAULT_ROWS)?;
        tracing::info!(agent = %agent.id, kind = kind.as_str(), worktree = %worktree.branch, "prewarmed agent session");
        let spare = WarmSpare {
            agent_id: agent.id,
            target,
            spawned_at: Instant::now(),
            hooks_before_adoption: Vec::new(),
            input_ready: false,
        };
        if let Some(doomed) = self
            .prewarm
            .install_returning_the_one_to_kill(ticket, spare)
        {
            self.kill_session(&doomed.session());
        }
        Ok(())
    }

    pub(crate) fn take_prewarmed(
        &self,
        worktree_id: &WorktreeId,
        kind: AgentKind,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Option<WarmSpare> {
        let spare = self
            .prewarm
            .take_if(|s| &s.target.worktree == worktree_id && s.target.kind == kind)?;
        if !self.is_alive(&spare.session()) {
            return None;
        }
        if spare.target.model.as_deref() != model || spare.target.effort.as_deref() != effort {
            self.kill_session(&spare.session());
            return None;
        }
        Some(spare)
    }

    pub(crate) async fn take_prewarmed_for_prompt(
        &self,
        worktree_id: &WorktreeId,
        kind: AgentKind,
        model: Option<&str>,
        effort: Option<&str>,
        prompt: &str,
    ) -> Option<WarmSpare> {
        let asked_at = Instant::now();
        loop {
            let ready = self.prewarm.input_ready.notified();
            tokio::pin!(ready);
            ready.as_mut().enable();
            let decision = {
                let slot = self.prewarm.spare.lock().unwrap();
                let spare = slot
                    .as_ref()
                    .filter(|s| &s.target.worktree == worktree_id && s.target.kind == kind)?;
                if spare.target.model.as_deref() != model
                    || spare.target.effort.as_deref() != effort
                {
                    None
                } else {
                    Some(prompted_adoption(
                        kind,
                        spare.input_ready,
                        spare.spawned_at.elapsed(),
                        prompt,
                    ))
                }
            };
            match decision {
                None => return self.take_prewarmed(worktree_id, kind, model, effort),
                Some(PromptedAdoption::TypeNow) => {
                    let spare = self.take_prewarmed(worktree_id, kind, model, effort)?;
                    tracing::info!(
                        agent = %spare.agent_id,
                        waited_ms = asked_at.elapsed().as_millis() as u64,
                        "prewarmed agent adopted for a starting prompt"
                    );
                    return Some(spare);
                }
                Some(PromptedAdoption::Cold) => {
                    tracing::info!(
                        kind = kind.as_str(),
                        waited_ms = asked_at.elapsed().as_millis() as u64,
                        "starting prompt goes to a cold spawn: the spare cannot take typed input"
                    );
                    return None;
                }
                Some(PromptedAdoption::WaitForReady(left)) => {
                    let _ = tokio::time::timeout(left, ready).await;
                }
            }
        }
    }

    pub(crate) fn type_starting_prompt(&self, id: &AgentId, prompt: String) {
        let Some(session) = self.session(&SessionRef::Agent(id.clone())) else {
            tracing::warn!(agent = %id, "adopted spare is gone before its starting prompt was typed");
            return;
        };
        let id = id.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::triggers::type_prompt(&session, &prompt).await {
                tracing::warn!(agent = %id, error = %e, "typing the starting prompt failed");
            }
        });
    }

    pub fn reap_prewarmed(&self) {
        let enabled = crate::config::Config::load().prewarm;
        let doomed = self.prewarm.take_if(|s| {
            !enabled || s.spawned_at.elapsed() > PREWARM_MAX_AGE || !self.is_alive(&s.session())
        });
        if let Some(spare) = doomed {
            tracing::debug!(agent = %spare.agent_id, "reaping prewarmed session");
            self.kill_session(&spare.session());
        }
    }

    pub(crate) fn kill_prewarmed_in(&self, worktree_ids: &[WorktreeId]) {
        if let Some(spare) = self
            .prewarm
            .take_if(|s| worktree_ids.contains(&s.target.worktree))
        {
            self.kill_session(&spare.session());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::test_daemon;

    fn spare(id: &str, worktree: &str, kind: AgentKind) -> WarmSpare {
        WarmSpare {
            agent_id: AgentId(id.into()),
            target: Target {
                worktree: WorktreeId(worktree.into()),
                kind,
                model: None,
                effort: None,
            },
            spawned_at: Instant::now(),
            hooks_before_adoption: Vec::new(),
            input_ready: false,
        }
    }

    fn install(pool: &PrewarmPool, spare: WarmSpare) -> Option<String> {
        let ticket = pool.ticket();
        pool.install_returning_the_one_to_kill(ticket, spare)
            .map(|s| s.agent_id.0)
    }

    fn spare_id(pool: &PrewarmPool) -> Option<String> {
        pool.info().map(|(id, _)| id.0)
    }

    #[test]
    fn one_spare_in_total_and_the_newest_request_wins() {
        let pool = PrewarmPool::default();
        assert_eq!(install(&pool, spare("a", "w1", AgentKind::Claude)), None);
        assert_eq!(
            install(&pool, spare("b", "w2", AgentKind::Codex)).as_deref(),
            Some("a")
        );

        let older = pool.ticket();
        let newer = pool.ticket();
        let late =
            pool.install_returning_the_one_to_kill(older, spare("c", "w1", AgentKind::Claude));
        assert_eq!(late.map(|s| s.agent_id.0).as_deref(), Some("c"));
        assert_eq!(spare_id(&pool).as_deref(), Some("b"));
        let current =
            pool.install_returning_the_one_to_kill(newer, spare("d", "w1", AgentKind::Claude));
        assert_eq!(current.map(|s| s.agent_id.0).as_deref(), Some("b"));
        assert_eq!(spare_id(&pool).as_deref(), Some("d"));
    }

    #[test]
    fn hooks_wait_on_the_spare_and_a_dead_spare_is_never_adopted() {
        let daemon = test_daemon();
        install(&daemon.prewarm, spare("warm-1", "w1", AgentKind::Claude));

        daemon.apply_hook_event(
            &AgentId("warm-1".into()),
            HookEvent::SessionStart { source: None },
            Some("sid-9".into()),
        );
        daemon.apply_hook_event(&AgentId("stranger".into()), HookEvent::Stop, None);
        for i in 0..(PREWARM_HOOK_BUFFER_CAP + 5) {
            daemon.apply_hook_event(
                &AgentId("warm-1".into()),
                HookEvent::Notification {
                    notification_type: Some(format!("n{i}")),
                },
                None,
            );
        }
        {
            let slot = daemon.prewarm.spare.lock().unwrap();
            let hooks = &slot.as_ref().unwrap().hooks_before_adoption;
            assert_eq!(hooks.len(), PREWARM_HOOK_BUFFER_CAP);
            assert!(!hooks
                .iter()
                .any(|(e, _)| matches!(e, HookEvent::SessionStart { .. })));
        }

        assert!(daemon
            .take_prewarmed(&WorktreeId("w1".into()), AgentKind::Codex, None, None)
            .is_none());
        assert_eq!(spare_id(&daemon.prewarm).as_deref(), Some("warm-1"));
        assert!(daemon
            .take_prewarmed(&WorktreeId("w1".into()), AgentKind::Claude, None, None)
            .is_none());
        assert_eq!(spare_id(&daemon.prewarm), None);

        install(&daemon.prewarm, spare("warm-2", "w1", AgentKind::Claude));
        daemon.reap_prewarmed();
        assert_eq!(spare_id(&daemon.prewarm), None);
    }

    #[test]
    fn a_prompt_is_typed_only_into_a_spare_that_signalled_input_ready() {
        use PromptedAdoption::*;
        let young = Duration::from_secs(1);
        let old = SPARE_BOOT_BOUND + Duration::from_secs(1);
        let claude = AgentKind::Claude;
        assert_eq!(prompted_adoption(claude, true, old, "fix it"), TypeNow);
        assert_eq!(
            prompted_adoption(claude, false, young, "fix it"),
            WaitForReady(SPARE_BOOT_BOUND - young)
        );
        assert_eq!(prompted_adoption(claude, false, old, "fix it"), Cold);
        assert_eq!(prompted_adoption(claude, true, young, "a\x1b[201~b"), Cold);
        for kind in [
            AgentKind::Codex,
            AgentKind::Cursor,
            AgentKind::OpenCode,
            AgentKind::Grok,
        ] {
            assert_eq!(prompted_adoption(kind, true, young, "fix it"), Cold);
        }
    }

    #[tokio::test]
    async fn a_prompted_create_waits_for_session_start_and_leaves_a_stuck_spare() {
        let daemon = test_daemon();
        let w1 = WorktreeId("w1".into());
        install(&daemon.prewarm, spare("booting", "w1", AgentKind::Claude));
        let waiter = {
            let daemon = daemon.clone();
            let w1 = w1.clone();
            tokio::spawn(async move {
                daemon
                    .take_prewarmed_for_prompt(&w1, AgentKind::Claude, None, None, "fix it")
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!waiter.is_finished());
        assert_eq!(spare_id(&daemon.prewarm).as_deref(), Some("booting"));
        daemon.apply_hook_event(
            &AgentId("booting".into()),
            HookEvent::SessionStart { source: None },
            Some("sid".into()),
        );
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("the SessionStart woke the waiting create")
            .unwrap();
        assert_eq!(spare_id(&daemon.prewarm), None, "the ready spare was taken");

        let mut stuck = spare("stuck", "w1", AgentKind::Claude);
        stuck.spawned_at = Instant::now() - SPARE_BOOT_BOUND;
        install(&daemon.prewarm, stuck);
        let started = Instant::now();
        assert!(daemon
            .take_prewarmed_for_prompt(&w1, AgentKind::Claude, None, None, "fix it")
            .await
            .is_none());
        assert!(started.elapsed() < Duration::from_millis(100));
        assert_eq!(spare_id(&daemon.prewarm).as_deref(), Some("stuck"));
    }

    #[test]
    fn kill_prewarmed_in_scopes_to_worktrees() {
        let daemon = test_daemon();
        install(&daemon.prewarm, spare("b", "w2", AgentKind::Codex));
        daemon.kill_prewarmed_in(&[WorktreeId("w1".into())]);
        assert_eq!(spare_id(&daemon.prewarm).as_deref(), Some("b"));
        daemon.kill_prewarmed_in(&[WorktreeId("w2".into())]);
        assert_eq!(spare_id(&daemon.prewarm), None);
    }
}
