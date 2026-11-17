use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use berth_core::{
    Agent, AgentId, AgentKind, AgentStatus, Entity, EntityId, ServerEvent, SessionRef, TerminalId,
    Worktree, WorktreeId,
};

use crate::pty::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::registry::{cli_missing_message, epoch_ms, sanitize_title, Daemon};

static ONE_CLI_PER_TRANSCRIPT: Mutex<()> = Mutex::new(());

const ALREADY_RUNNING: &str = "that conversation is already running — stop its session first";

impl Daemon {
    pub async fn resume_orphaned_session(
        self: &Arc<Self>,
        session_id: &str,
        worktree_id: &WorktreeId,
    ) -> Result<EntityId> {
        let worktree = self
            .store
            .get_worktree(worktree_id)?
            .context("worktree not found")?;
        let (project, live) = self.project_with_worktrees(&worktree.project_id)?;
        let orphan = crate::orphans::list(&self.store, &project, &live, true)?
            .into_iter()
            .find(|o| o.session_id == session_id)
            .context("that session is not on record any more")?;
        self.refuse_unresumable(orphan.kind, &orphan.session_id)
            .await?;
        let claude_only_notice = (orphan.kind == AgentKind::Claude)
            .then(|| crate::orphans::resume_notice(&orphan, &worktree))
            .flatten();
        if orphan.kind == AgentKind::Claude {
            carry_transcript_into_reach(&orphan.session_id, &live, &worktree);
        }
        let id = self.spawn_resumed(
            &worktree,
            orphan.kind,
            &orphan.session_id,
            orphan.name.clone(),
            claude_only_notice.as_deref(),
        )?;
        if let Err(e) = self.store.set_orphan_resumed(&orphan.session_id) {
            tracing::warn!(error = %e, "stamping the resumed orphan failed");
        }
        self.retire_idle_host_terminals(&orphan.session_id).await;
        Ok(EntityId::Agent(id))
    }

    pub async fn resume_conversation(
        self: &Arc<Self>,
        worktree_id: &WorktreeId,
        kind: AgentKind,
        session_id: &str,
        name: &str,
    ) -> Result<EntityId> {
        if !matches!(kind, AgentKind::Claude | AgentKind::Codex) {
            bail!("only a Claude or Codex conversation resumes as an agent");
        }
        let session_id = session_id.trim();
        if session_id.is_empty() {
            bail!("no conversation to resume");
        }
        let worktree = self
            .store
            .get_worktree(worktree_id)?
            .context("worktree not found")?;
        self.refuse_unresumable(kind, session_id).await?;
        if let Some(owner) = self.owning_agent(session_id)? {
            let id = self.resume_in_owner_row(owner, &worktree)?;
            self.retire_idle_host_terminals(session_id).await;
            return Ok(EntityId::Agent(id));
        }
        let (project, live) = self.project_with_worktrees(&worktree.project_id)?;
        let orphan_record = match kind {
            AgentKind::Claude => crate::orphans::list(&self.store, &project, &live, true)?
                .into_iter()
                .find(|o| o.session_id == session_id),
            _ => None,
        };
        let notice = orphan_record
            .as_ref()
            .and_then(|o| crate::orphans::resume_notice(o, &worktree));
        let given = sanitize_title(name);
        let name = match &orphan_record {
            Some(o) if !o.live || given.is_empty() => o.name.clone(),
            _ if given.is_empty() => session_id.chars().take(8).collect(),
            _ => given,
        };
        if kind == AgentKind::Claude {
            carry_transcript_into_reach(session_id, &live, &worktree);
        }
        let id = self.spawn_resumed(&worktree, kind, session_id, name, notice.as_deref())?;
        if orphan_record.is_some() {
            if let Err(e) = self.store.set_orphan_resumed(session_id) {
                tracing::warn!(error = %e, "stamping the resumed orphan failed");
            }
        }
        self.retire_idle_host_terminals(session_id).await;
        Ok(EntityId::Agent(id))
    }

    async fn refuse_unresumable(&self, kind: AgentKind, session_id: &str) -> Result<()> {
        if !self.cli_available_for_create(kind).await {
            bail!("{}", cli_missing_message(kind));
        }
        self.refuse_if_an_agent_runs(session_id)?;
        if let Some(terminal) = self.terminal_running_claude_on(session_id).await {
            bail!("that conversation is running in the terminal \"{terminal}\" — close it first");
        }
        Ok(())
    }

    fn refuse_if_an_agent_runs(&self, session_id: &str) -> Result<()> {
        let running = self
            .store
            .agent_ids_by_session_id(session_id)?
            .into_iter()
            .any(|id| self.is_alive(&SessionRef::Agent(id)));
        if running {
            bail!("{ALREADY_RUNNING}");
        }
        Ok(())
    }

    async fn terminal_running_claude_on(&self, session_id: &str) -> Option<String> {
        let terminals: Vec<(TerminalId, u32)> = self
            .session_pids()
            .into_iter()
            .filter_map(|(sref, pid, _)| match sref {
                SessionRef::Terminal(id) => Some((id, pid)),
                SessionRef::Agent(_) => None,
            })
            .collect();
        if terminals.is_empty() {
            return None;
        }
        let wanted = session_id.to_string();
        let found = tokio::task::spawn_blocking(move || {
            let table = berth_core::platform::process_table();
            let dir = crate::cli_title::claude_sessions_dir();
            crate::cli_title::terminal_sessions(&table, &terminals, &dir)
                .into_iter()
                .find(|(_, running)| *running == wanted)
                .map(|(id, _)| id)
        })
        .await
        .ok()
        .flatten()?;
        let name = self
            .store
            .get_terminal(&found)
            .ok()
            .flatten()
            .map(|terminal| terminal.name)
            .unwrap_or_else(|| found.to_string());
        Some(name)
    }

    fn owning_agent(&self, session_id: &str) -> Result<Option<Agent>> {
        let mut owners = Vec::new();
        for id in self.store.agent_ids_by_session_id(session_id)? {
            if let Some(agent) = self.store.get_agent(&id)? {
                owners.push(agent);
            }
        }
        owners.sort_by_key(|agent| agent.archived);
        Ok(owners.into_iter().next())
    }

    fn resume_in_owner_row(self: &Arc<Self>, owner: Agent, worktree: &Worktree) -> Result<AgentId> {
        let _gate = ONE_CLI_PER_TRANSCRIPT
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let sref = SessionRef::Agent(owner.id.clone());
        if self.is_alive(&sref) {
            bail!("{ALREADY_RUNNING}");
        }
        if owner.archived {
            self.unarchive_agent(&owner.id)?;
        }
        if owner.worktree_id != worktree.id {
            if let Err(e) = self.move_agent(&owner.id, &worktree.id) {
                tracing::warn!(agent = %owner.id, error = %e, "the conversation resumes in its own worktree");
            }
        }
        self.ensure_session(&sref, DEFAULT_COLS, DEFAULT_ROWS)?;
        Ok(owner.id)
    }

    fn spawn_resumed(
        self: &Arc<Self>,
        worktree: &Worktree,
        kind: AgentKind,
        session_id: &str,
        name: String,
        notice: Option<&str>,
    ) -> Result<AgentId> {
        let _gate = ONE_CLI_PER_TRANSCRIPT
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.refuse_if_an_agent_runs(session_id)?;
        let agent = Agent {
            id: AgentId::generate(),
            worktree_id: worktree.id.clone(),
            name,
            status: AgentStatus::Fresh,
            archived: false,
            archived_at: 0,
            unseen: false,
            kind,
            model: None,
            effort: None,
            session_id: Some(session_id.to_string()),
            cloud_session_id: None,
            sort_order: 0,
            status_changed_at: epoch_ms(),
            alive: false,
            cloud_mirroring: false,
            suggested_prompt: None,
            provider_error: None,
            routed_from: None,
            route_suggestion: None,
            acp_provider: None,
        };
        self.store.insert_agent(&agent)?;
        let spawned = self.spawn_agent_session_with(
            &agent,
            worktree,
            DEFAULT_COLS,
            DEFAULT_ROWS,
            None,
            notice,
        );
        self.rollback_agent_on_spawn_error(&agent.id, spawned)?;
        let mut broadcast_agent = agent.clone();
        broadcast_agent.alive = true;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Agent(Box::new(broadcast_agent)),
        });
        Ok(agent.id)
    }

    async fn retire_idle_host_terminals(self: &Arc<Self>, session_id: &str) {
        let terminals = match self.store.load_tree() {
            Ok((_, _, _, terminals)) => terminals,
            Err(e) => {
                tracing::warn!(error = %e, "looking for the conversation's old terminal failed");
                return;
            }
        };
        let hosts: Vec<(TerminalId, Option<Option<u32>>)> = terminals
            .into_iter()
            .filter(|terminal| terminal.session_id.as_deref() == Some(session_id))
            .map(|terminal| {
                let shell_pid = self
                    .session(&SessionRef::Terminal(terminal.id.clone()))
                    .map(|session| session.child_pid);
                (terminal.id, shell_pid)
            })
            .collect();
        if hosts.is_empty() {
            return;
        }
        let idle = tokio::task::spawn_blocking(move || {
            hosts
                .into_iter()
                .filter(|(_, shell_pid)| match shell_pid {
                    None => true,
                    Some(None) => false,
                    Some(Some(pid)) => !berth_core::platform::has_child_processes(*pid),
                })
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        for id in idle {
            if let Err(e) = self.close_terminal(&id) {
                tracing::warn!(terminal = %id, error = %e, "closing the conversation's old terminal failed");
            }
        }
    }
}

fn carry_transcript_into_reach(session_id: &str, live: &[Worktree], worktree: &Worktree) {
    let checkouts: Vec<PathBuf> = live.iter().map(|w| w.path.clone()).collect();
    if let Err(e) = crate::orphans::carry_transcript(session_id, &checkouts, &worktree.path) {
        tracing::warn!(error = %e, "transcript carry failed");
    }
}
