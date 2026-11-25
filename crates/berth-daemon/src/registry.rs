//! The daemon's world: persisted entity tree + live PTY sessions, and the
//! operations the IPC surface exposes over them.

use crate::agent;
use crate::git;
use crate::hooks::HookEnv;
use crate::launch;
use crate::pty::{PtyEvent, PtySession, SpawnSpec, DEFAULT_COLS, DEFAULT_ROWS};
use crate::resume_check;
use crate::status::{AgentStatusMachine, Effect, HookEvent};
use crate::store::Store;
use anyhow::{bail, Context, Result};
use berth_core::env;
use berth_core::paths;
use berth_core::{
    Agent, AgentId, AgentKind, AgentStatus, Backend, EnterOutcome, Entity, EntityId,
    MetricsSnapshot, OrphanedSession, PrewarmInfo, Project, ProjectId, ServerEvent, SessionRef,
    TerminalId, TerminalTab, Workspace, WorkspaceId, Worktree, WorktreeId,
    MAX_STARTING_PROMPT_BYTES,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// `$SHELL -l -i -c <cmd>`: a login *and* interactive shell, so zsh sources
/// ~/.zprofile and ~/.zshrc both and the child sees the PATH the user's
/// terminal has. The CLI probe and the spawn wrapper share it so they can
/// never disagree about what "on the user's PATH" means.
/// Cap on one CLI probe. A heavy rc file costs ~1s; a hung one must not
/// stall a create forever, so on timeout the CLI is assumed present and
/// the spawn itself gets to report.
const CLI_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct CreateAgentSpec {
    pub worktree: WorktreeId,
    pub name: String,
    pub kind: AgentKind,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub auto_title: bool,
    /// The CLI's positional first prompt (an AGENT PRESET launch). Request-only.
    pub starting_prompt: Option<String>,
}

/// Wall-clock epoch ms, matching the store's `status_changed_at` stamps.
pub(crate) fn epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub struct Daemon {
    sessions: Mutex<HashMap<SessionRef, Arc<PtySession>>>,
    status_machines: Mutex<HashMap<AgentId, AgentStatusMachine>>,
    /// The STOP GATE's escape hatch for subagents Claude killed without a
    /// `SubagentStop`. See `crate::subagents`.
    killed_subagents: crate::subagents::KilledSubagents,
    pub(crate) feedback: crate::feedback::PendingFeedback,
    pub(crate) provider_errors: crate::router::ProviderErrors,
    pub hook_env: HookEnv,
    /// Shared with the hook HTTP server, which reads agent rows to decide
    /// auto-title injection.
    pub store: Arc<Store>,
    /// Entity/status deltas fanned out to every subscribed client.
    pub events: broadcast::Sender<ServerEvent>,
    pub(crate) installed_sessions: broadcast::Sender<Arc<PtySession>>,
    pub shutdown: tokio_util::sync::CancellationToken,
    /// Serializes worktree create/delete with the background auto-sync so
    /// a checkout is never adopted twice while its row is mid-insert.
    worktree_ops: tokio::sync::Mutex<()>,
    pub(crate) prewarm: crate::prewarm::PrewarmPool,
    /// Cached `command -v` results per CLI so a missing binary doesn't get
    /// re-probed (login shell spawn) on every prewarm request.
    pub(crate) cli_probes: Mutex<HashMap<AgentKind, (bool, Instant)>>,
    /// How many client connections are attached per session — a session
    /// with attachments (and its whole worktree) is "in view" and exempt
    /// from idle reaping.
    attach_counts: Mutex<HashMap<SessionRef, usize>>,
    /// When each live session was last "looked at": spawned, prewarmed,
    /// attached, or covered by the in-view sweep refresh. The idle reaper
    /// kills sessions whose stamp ages past `session_idle_timeout`.
    session_interest: Mutex<HashMap<SessionRef, Instant>>,
    /// Last hook-reported cwd per agent, recorded only for payloads that
    /// passed the foreign-session gate. An agent that walks into a checkout
    /// berth hasn't adopted yet leaves its cwd here, so the worktree sync
    /// can finish the re-home once the row exists.
    last_cwd: Mutex<HashMap<AgentId, PathBuf>>,
    /// Agents that ran `berth worktree` and are waiting for their turn to
    /// end: the row already sits under the target worktree while the PTY
    /// still runs in the old checkout. Drained by `complete_pending_move`
    /// on the turn-end hook (kill + respawn resumed in the target), cleared
    /// by any other spawn of the agent, and consulted by the cwd reparent so
    /// the old checkout's cwd can't drag the row back in the meantime.
    pending_moves: Mutex<HashMap<AgentId, Worktree>>,
    pub(crate) pending_ends: Mutex<HashSet<AgentId>>,
    /// Serializes the check-and-spawn inside [`Daemon::ensure_session`].
    spawn_gate: Mutex<()>,
    /// The last reading [`crate::sampler`] published. `GetMetrics` answers
    /// from here rather than sweeping: CPU is a delta and the token tally
    /// is a file offset, so only one owner may measure them. The sampler
    /// task is the only writer; every request handler only reads.
    last_sample: Mutex<Option<MetricsSnapshot>>,
    pub(crate) acp: crate::acp::AcpSessions,
}

impl Daemon {
    pub fn new(store: Arc<Store>, hook_env: HookEnv) -> Arc<Self> {
        let (events, _) = broadcast::channel(1024);
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            status_machines: Mutex::new(HashMap::new()),
            killed_subagents: Default::default(),
            feedback: Default::default(),
            provider_errors: Default::default(),
            hook_env,
            store,
            events,
            installed_sessions: broadcast::channel(64).0,
            shutdown: tokio_util::sync::CancellationToken::new(),
            worktree_ops: tokio::sync::Mutex::new(()),
            prewarm: Default::default(),
            cli_probes: Mutex::new(HashMap::new()),
            attach_counts: Mutex::new(HashMap::new()),
            session_interest: Mutex::new(HashMap::new()),
            last_cwd: Mutex::new(HashMap::new()),
            pending_moves: Mutex::new(HashMap::new()),
            pending_ends: Mutex::new(HashSet::new()),
            spawn_gate: Mutex::new(()),
            last_sample: Mutex::new(None),
            acp: Default::default(),
        })
    }

    // ---- status machine plumbing ----

    /// Feed one hook (or synthetic) event through the agent's status machine
    /// and apply the resulting effects (persist + broadcast).
    pub fn apply_hook_event(
        &self,
        agent_id: &AgentId,
        event: HookEvent,
        session_id: Option<String>,
    ) {
        enum Outcome {
            Effects(Vec<Effect>),
            UnknownAgent(HookEvent, Option<String>),
        }
        let outcome = {
            let mut machines = self.status_machines.lock().unwrap();
            match machines.entry(agent_id.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => Outcome::Effects(
                    e.into_mut()
                        .handle(event, session_id.as_deref(), Instant::now()),
                ),
                std::collections::hash_map::Entry::Vacant(slot) => {
                    // Lazily seed from the persisted row.
                    match self.store.get_agent(agent_id) {
                        Ok(Some(agent)) => Outcome::Effects(
                            slot.insert(AgentStatusMachine::new(agent.status, agent.session_id))
                                .handle(event, session_id.as_deref(), Instant::now()),
                        ),
                        _ => Outcome::UnknownAgent(event, session_id),
                    }
                }
            }
        };
        match outcome {
            Outcome::Effects(effects) => self.apply_status_effects(agent_id, effects),
            // Ids with no row are prewarmed sessions (buffer for replay at
            // adoption) or stale env / deleted agents (dropped, as before).
            Outcome::UnknownAgent(event, session_id) => {
                self.prewarm.buffer_hook(agent_id, event, session_id)
            }
        }
    }

    /// Where a session's conversation lives on disk, as the CLI reports it
    /// on every hook. Only Claude sends one.
    pub fn note_transcript_path(&self, agent_id: &AgentId, path: &str) {
        self.killed_subagents.note_transcript(agent_id, path);
    }

    /// The last transcript path the hooks reported for this agent; None
    /// until one lands, and after a daemon restart.
    pub fn transcript_path(&self, agent_id: &AgentId) -> Option<PathBuf> {
        self.killed_subagents.transcript(agent_id)
    }

    pub fn publish_sample(&self, snapshot: MetricsSnapshot) {
        *self.last_sample.lock().unwrap() = Some(snapshot);
    }

    /// The sampler's last reading. None before its first tick.
    pub fn last_sample(&self) -> Option<MetricsSnapshot> {
        self.last_sample.lock().unwrap().clone()
    }

    /// Deferred-finish recheck across all machines (runs on a timer). A held
    /// Stop first drops the subagents Claude killed without a `SubagentStop`,
    /// so the hold drains instead of waiting out `SUBAGENT_QUIET_GRACE`.
    /// Each machine also gets its CLI's current progress level, which is how
    /// a turn ended by an Esc cancel stops wedging the row — see
    /// `AgentStatusMachine::tick`.
    pub fn tick_status_machines(&self) {
        let now = Instant::now();
        self.drop_killed_subagents();
        // Gathered before the machines are locked: a session's progress lock
        // is a leaf, and nothing here should hold two of the daemon's own.
        let levels: HashMap<AgentId, bool> = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .iter()
                .filter_map(|(sref, session)| match sref {
                    SessionRef::Agent(id) => Some((id.clone(), session.progress_busy()?)),
                    SessionRef::Terminal(_) => None,
                })
                .collect()
        };
        let ticked: Vec<(AgentId, Vec<Effect>)> = {
            let mut machines = self.status_machines.lock().unwrap();
            machines
                .iter_mut()
                .map(|(id, m)| (id.clone(), m.tick(now, levels.get(id).copied())))
                .collect()
        };
        for (id, effects) in ticked {
            self.apply_status_effects(&id, effects);
        }
    }

    /// Read the meta files for every held Stop and forget the workers the
    /// user killed. The disk is touched only for agents actually holding —
    /// `held_subagents` is `None` for everyone else.
    fn drop_killed_subagents(&self) {
        let held: Vec<(AgentId, Vec<String>)> = {
            let machines = self.status_machines.lock().unwrap();
            machines
                .iter()
                .filter_map(|(id, m)| Some((id.clone(), m.held_subagents()?)))
                .collect()
        };
        for (agent_id, tracked) in held {
            let killed = self.killed_subagents.killed(&agent_id, &tracked);
            if killed.is_empty() {
                continue;
            }
            let mut machines = self.status_machines.lock().unwrap();
            let Some(machine) = machines.get_mut(&agent_id) else {
                continue;
            };
            for id in killed {
                tracing::debug!(agent = %agent_id, subagent = %id, "subagent killed by user");
                machine.forget_subagent(&id);
            }
        }
    }

    fn apply_status_effects(&self, agent_id: &AgentId, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::SetStatus(status) => {
                    if status != AgentStatus::NeedsFeedback {
                        self.clear_feedback(agent_id);
                    }
                    let (changed_at, change) = match self.store.set_agent_status(agent_id, status) {
                        Ok((changed_at, change)) => (changed_at, Some(change)),
                        Err(e) => {
                            tracing::warn!(error = %e, "persist status failed");
                            (epoch_ms(), None)
                        }
                    };
                    self.broadcast(ServerEvent::StatusChanged {
                        agent: agent_id.clone(),
                        status,
                        changed_at,
                        unseen: change.is_some_and(|c| c.after.unseen),
                        raised: change.and_then(|c| c.raised),
                    });
                }
                Effect::SaveSessionId(sid) => {
                    match self.store.set_agent_session_id(agent_id, Some(&sid)) {
                        Ok(()) => self.try_broadcast_agent(agent_id),
                        Err(e) => tracing::warn!(error = %e, "persist session id failed"),
                    }
                }
                Effect::ArchiveClearedSession(sid) => self.archive_cleared_session(agent_id, &sid),
            }
        }
    }

    pub fn broadcast(&self, ev: ServerEvent) {
        let _ = self.events.send(ev);
    }

    pub fn session(&self, sref: &SessionRef) -> Option<Arc<PtySession>> {
        self.sessions.lock().unwrap().get(sref).cloned()
    }

    pub fn is_alive(&self, sref: &SessionRef) -> bool {
        self.sessions.lock().unwrap().contains_key(sref)
            || matches!(sref, SessionRef::Agent(id) if self.acp.is_live(id))
    }

    /// (session, child pid, prewarm-pool home) for every live PTY — the
    /// metrics reading's input. A pool spare has no agent row, so the only
    /// way a client can name or place it is the home reported here.
    pub fn session_pids(&self) -> Vec<(SessionRef, u32, Option<PrewarmInfo>)> {
        // Snapshot the pool first and drop its lock: `prewarm_agent` holds
        // the pool lock while it asks the sessions map, so the two are
        // never held together here in the other order.
        let spare = self.prewarm.info();
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(sref, s)| {
                let pid = s.child_pid?;
                let prewarm = match (sref, &spare) {
                    (SessionRef::Agent(id), Some((spare_id, info))) if id == spare_id => {
                        Some(info.clone())
                    }
                    _ => None,
                };
                Some((sref.clone(), pid, prewarm))
            })
            .collect()
    }

    pub fn remove_session(&self, sref: &SessionRef) -> Option<Arc<PtySession>> {
        self.session_interest.lock().unwrap().remove(sref);
        self.sessions.lock().unwrap().remove(sref)
    }

    pub fn kill_session(&self, sref: &SessionRef) {
        if let Some(s) = self.remove_session(sref) {
            s.kill();
        }
        if let SessionRef::Agent(id) = sref {
            self.acp.kill(id);
            self.note_session_killed(id);
        }
    }

    pub fn kill_all(&self) {
        for (_, s) in self.sessions.lock().unwrap().drain() {
            s.kill();
        }
        self.acp.kill_all();
    }

    // ---- attach tracking & idle reaping ----

    /// A client attached to `sref` (the server dedupes re-attaches per
    /// connection). While any attachment exists, the session — and its
    /// whole worktree — counts as "in view".
    pub fn note_attached(&self, sref: &SessionRef) {
        *self
            .attach_counts
            .lock()
            .unwrap()
            .entry(sref.clone())
            .or_insert(0) += 1;
        self.touch_session(sref);
    }

    /// A client detached from `sref` (or its connection dropped). Restamps
    /// the session so the idle clock starts at "stopped looking", not at
    /// spawn time.
    pub fn note_detached(&self, sref: &SessionRef) {
        let mut counts = self.attach_counts.lock().unwrap();
        if let Some(n) = counts.get_mut(sref) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                counts.remove(sref);
            }
        }
        drop(counts);
        self.touch_session(sref);
    }

    /// Stamp `sref` as just-looked-at for the idle reaper.
    fn touch_session(&self, sref: &SessionRef) {
        self.session_interest
            .lock()
            .unwrap()
            .insert(sref.clone(), Instant::now());
    }

    /// Kill idle sessions in worktrees no client is looking at, per
    /// `session_idle_timeout` — this bounds what prewarming and
    /// walked-away-from sessions cost. "In view" = the worktree holding any
    /// attached session; in-view sessions get their stamps refreshed
    /// instead, so the full timeout starts only when the user leaves.
    /// Spared regardless of age: agents that are running or waiting on
    /// feedback, terminals with a command running, and prewarm-pool sessions
    /// (`reap_prewarmed` owns those). A reaped session revives on the next
    /// attach; agents resume their conversation.
    pub fn reap_idle_sessions(self: &Arc<Self>) {
        let Some(timeout) = crate::config::Config::load().session_idle_timeout() else {
            return;
        };
        self.reap_idle_acp(timeout);
        let sessions: Vec<(SessionRef, Arc<PtySession>)> = self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let attached: std::collections::HashSet<SessionRef> =
            self.attach_counts.lock().unwrap().keys().cloned().collect();
        let viewed_worktrees: std::collections::HashSet<WorktreeId> = attached
            .iter()
            .filter_map(|sref| self.session_worktree(sref))
            .collect();
        let now = Instant::now();
        for (sref, session) in sessions {
            // No store row = prewarm-pool session (or deleted mid-sweep).
            let Some(worktree_id) = self.session_worktree(&sref) else {
                continue;
            };
            if attached.contains(&sref) || viewed_worktrees.contains(&worktree_id) {
                self.touch_session(&sref);
                continue;
            }
            let age = {
                let mut interest = self.session_interest.lock().unwrap();
                // A missing stamp (session predating the map) starts aging now.
                now.duration_since(*interest.entry(sref.clone()).or_insert(now))
            };
            if age < timeout {
                continue;
            }
            let spared = match &sref {
                SessionRef::Agent(id) => match self.store.get_agent(id).ok().flatten() {
                    Some(agent) => matches!(
                        agent.status,
                        AgentStatus::Running | AgentStatus::NeedsFeedback
                    ),
                    // Row vanished mid-sweep: its delete kills the PTY anyway.
                    None => true,
                },
                SessionRef::Terminal(_) => shell_has_children(&session),
            };
            if spared {
                continue;
            }
            tracing::info!(session = ?sref, idle_secs = age.as_secs(), "reaping idle session");
            self.kill_session(&sref);
            let upsert = match &sref {
                SessionRef::Agent(id) => self
                    .agent_entity(id)
                    .map(|agent| Entity::Agent(Box::new(agent))),
                SessionRef::Terminal(id) => self.terminal_entity(id).map(Entity::Terminal),
            };
            if let Ok(entity) = upsert {
                self.broadcast(ServerEvent::EntityUpserted { entity });
            }
        }
    }

    /// The worktree a session's row lives under; None when the row is gone
    /// or never existed (prewarm pool).
    fn session_worktree(&self, sref: &SessionRef) -> Option<WorktreeId> {
        match sref {
            SessionRef::Agent(id) => self
                .store
                .get_agent(id)
                .ok()
                .flatten()
                .map(|a| a.worktree_id),
            SessionRef::Terminal(id) => self
                .store
                .get_terminal(id)
                .ok()
                .flatten()
                .map(|t| t.worktree_id),
        }
    }

    // ---- snapshot ----

    pub fn snapshot(&self) -> Result<ServerEvent> {
        let (projects, worktrees, mut agents, mut terminals) = self.store.load_tree()?;
        {
            let sessions = self.sessions.lock().unwrap();
            for a in &mut agents {
                a.alive = sessions.contains_key(&SessionRef::Agent(a.id.clone()));
                self.decorate_route(a);
            }
            for t in &mut terminals {
                t.alive = sessions.contains_key(&SessionRef::Terminal(t.id.clone()));
            }
        }
        for a in &mut agents {
            a.alive |= self.acp.is_live(&a.id);
        }
        Ok(ServerEvent::Snapshot {
            workspaces: self.store.load_workspaces()?,
            active_workspace: self.store.active_workspace_id()?,
            projects,
            worktrees,
            agents,
            terminals,
            archived_external_sessions: self.store.load_archived_external_sessions()?,
        })
    }

    fn agent_entity(&self, id: &AgentId) -> Result<Agent> {
        let mut agent = self.store.get_agent(id)?.context("agent not found")?;
        agent.alive = self.is_alive(&SessionRef::Agent(id.clone()));
        agent.suggested_prompt = self
            .sessions
            .lock()
            .unwrap()
            .get(&SessionRef::Agent(id.clone()))
            .and_then(|session| session.suggested_prompt());
        self.decorate_route(&mut agent);
        Ok(agent)
    }

    /// Push the agent's current row — liveness and mirror flags included —
    /// to every subscriber. The tail of every mutation that changes how
    /// the row renders; fails only when the row is gone.
    pub(crate) fn broadcast_agent(&self, id: &AgentId) -> Result<()> {
        let agent = self.agent_entity(id)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Agent(Box::new(agent)),
        });
        Ok(())
    }

    /// [`Self::broadcast_agent`] for the best-effort sites — background
    /// tasks and post-respawn refreshes — where a row deleted meanwhile is
    /// not an error: nothing to show, so nothing to say.
    fn try_broadcast_agent(&self, id: &AgentId) {
        let _ = self.broadcast_agent(id);
    }

    fn terminal_entity(&self, id: &TerminalId) -> Result<TerminalTab> {
        let mut term = self.store.get_terminal(id)?.context("terminal not found")?;
        term.alive = self.is_alive(&SessionRef::Terminal(id.clone()));
        Ok(term)
    }

    // ---- workspaces ----

    /// Validated, trimmed workspace name, checked for collisions (excluding
    /// `except` on renames).
    fn checked_workspace_name(&self, name: &str, except: Option<&WorkspaceId>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!("workspace name is empty");
        }
        if let Some(existing) = self.store.workspace_by_name(name)? {
            if Some(&existing) != except {
                bail!("a workspace named '{name}' already exists");
            }
        }
        Ok(name.to_string())
    }

    /// Create a workspace. Does not open it — `workspace open` stays a
    /// separate, explicit step.
    pub fn add_workspace(self: &Arc<Self>, name: &str) -> Result<EntityId> {
        let name = self.checked_workspace_name(name, None)?;
        let workspace = Workspace {
            id: WorkspaceId::generate(),
            name,
        };
        self.store.insert_workspace(&workspace)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Workspace(workspace.clone()),
        });
        Ok(EntityId::Workspace(workspace.id))
    }

    pub fn rename_workspace(self: &Arc<Self>, id: &WorkspaceId, name: &str) -> Result<()> {
        let mut workspace = self
            .store
            .get_workspace(id)?
            .context("workspace not found")?;
        workspace.name = self.checked_workspace_name(name, Some(id))?;
        self.store.rename_workspace(id, &workspace.name)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Workspace(workspace),
        });
        Ok(())
    }

    /// Delete a workspace. Only empty ones go — its projects are the user's
    /// to move or remove first — and never the last one. Deleting the
    /// remembered default moves that default to a survivor; clients still
    /// scoped to it re-scope themselves off the EntityRemoved.
    pub fn remove_workspace(self: &Arc<Self>, id: &WorkspaceId) -> Result<()> {
        self.store
            .get_workspace(id)?
            .context("workspace not found")?;
        let projects = self.store.count_workspace_projects(id)?;
        if projects > 0 {
            bail!(
                "workspace still has {projects} project{} — remove them first",
                if projects == 1 { "" } else { "s" }
            );
        }
        if self.store.count_workspaces()? <= 1 {
            bail!("cannot delete the last workspace");
        }
        if self.store.active_workspace_id()? == *id {
            let fallback = self
                .store
                .load_workspaces()?
                .into_iter()
                .find(|w| &w.id != id)
                .context("no workspace left to open")?;
            self.store.set_active_workspace(&fallback.id)?;
        }
        self.store.delete_workspace(id)?;
        self.broadcast(ServerEvent::EntityRemoved {
            id: EntityId::Workspace(id.clone()),
        });
        Ok(())
    }

    /// Remember `id` as the workspace a fresh client opens into. Which
    /// workspace a *live* client is looking at is that client's own state —
    /// see `ClientRequest::OpenWorkspace` — so this deliberately notifies
    /// nobody: one instance switching must leave the others where they are.
    pub fn set_default_workspace(self: &Arc<Self>, id: &WorkspaceId) -> Result<()> {
        self.store
            .get_workspace(id)?
            .context("workspace not found")?;
        if self.store.active_workspace_id()? == *id {
            return Ok(()); // already the default
        }
        self.store.set_active_workspace(id)?;
        Ok(())
    }

    // ---- projects ----

    /// Register a repo as a project. `workspace` is the caller's own scope
    /// (a TUI that switched with OpenWorkspace); `None` — a one-shot
    /// `berth add`, or a client still on whatever it booted into — means
    /// the remembered default. A scope naming a workspace that has since
    /// been deleted falls back the same way rather than failing the add.
    pub async fn add_project(
        self: &Arc<Self>,
        path: &Path,
        name: Option<String>,
        create_missing: bool,
        workspace: Option<WorkspaceId>,
    ) -> Result<EntityId> {
        if create_missing && !path.exists() {
            tokio::fs::create_dir_all(path)
                .await
                .with_context(|| format!("create {}", path.display()))?;
            if crate::config::Config::load().git_init_on_create {
                git::init(path).await?;
            }
        }
        // "not a git repository" is the right explanation only when git ran and
        // said no — if git itself is missing, that message blames the wrong
        // thing, so let git.rs's own diagnosis through untouched.
        let toplevel = git::repo_toplevel(path).await.map_err(|e| {
            if git::is_missing(&e) {
                e
            } else {
                e.context(format!("{} is not a git repository", path.display()))
            }
        })?;
        // `--show-toplevel` answers with the checkout it was run in, so inside a
        // linked worktree it names the worktree rather than the repo. A project
        // is the repo: root it at the main checkout, which `git worktree list`
        // always puts first. Adding from inside a worktree used to name the
        // project after that worktree and leave its ⌂ root row pointing at a
        // directory the project did not own.
        let entries = git::list_worktrees(&toplevel)
            .await
            .with_context(|| format!("list checkouts of {}", toplevel.display()))?;
        let repo_path = match entries.first() {
            Some(main) => main.path.clone(),
            // git listing no checkout at all for a path it just called a work
            // tree would leave the root unknowable; refuse rather than seed a
            // project with no rows, which is how a project loses its root row.
            None => bail!("git listed no checkout for {}", toplevel.display()),
        };
        // New projects land in the caller's own workspace; the same repo
        // may be added to any number of workspaces, just not twice to one.
        let workspace_id = match workspace {
            Some(id) if self.store.get_workspace(&id)?.is_some() => id,
            _ => self.store.active_workspace_id()?,
        };
        if self
            .store
            .project_in_workspace(&repo_path, &workspace_id)?
            .is_some()
        {
            bail!(
                "project already added to this workspace: {}",
                repo_path.display()
            );
        }
        let name = name.unwrap_or_else(|| Project::folder_name(&repo_path));
        let project = Project {
            id: ProjectId::generate(),
            name,
            workspace_id,
            repo_path: repo_path.clone(),
            sort_order: self.store.next_project_sort_order()?,
            pin_order: None,
        };
        self.store.insert_project(&project)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Project(project.clone()),
        });

        // Main checkout is modeled as a worktree row; adopt pre-existing
        // worktrees too so `berth` matches reality on day one. Root-ness is
        // the path test the reconcile uses, not insert order — the two agreeing
        // is what keeps `repo_path` and the ⌂ root row the same directory.
        for entry in entries {
            let worktree = Worktree {
                id: WorktreeId::generate(),
                project_id: project.id.clone(),
                is_main: entry.path == repo_path,
                path: entry.path.clone(),
                branch: entry.branch,
                sort_order: 0,
            };
            self.store.insert_worktree(&worktree)?;
            crate::shared_setup::sync_checkout(&project, &worktree.path);
            self.broadcast(ServerEvent::EntityUpserted {
                entity: Entity::Worktree(worktree),
            });
        }
        Ok(EntityId::Project(project.id))
    }

    /// Retitle a project's row. Cosmetic only — the checkout on disk is never
    /// renamed, and every worktree under the project keeps its own path. An
    /// empty name resets the row to the folder's name, which is the only way
    /// back once a project has been renamed.
    pub fn rename_project(self: &Arc<Self>, id: &ProjectId, name: &str) -> Result<()> {
        let mut project = self.store.get_project(id)?.context("project not found")?;
        let name = name.trim();
        project.name = if name.is_empty() {
            Project::folder_name(&project.repo_path)
        } else {
            name.to_string()
        };
        self.store.rename_project(id, &project.name)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Project(project),
        });
        Ok(())
    }

    pub fn remove_project(self: &Arc<Self>, id: &ProjectId) -> Result<()> {
        // Kill any live sessions under this project first.
        let (_, worktrees, agents, terminals) = self.store.load_tree()?;
        let wt_ids: Vec<WorktreeId> = worktrees
            .into_iter()
            .filter(|w| &w.project_id == id)
            .map(|w| w.id)
            .collect();
        self.kill_sessions_in(&wt_ids, &agents, &terminals);
        // Removing a project only forgets it in berth — never touches disk.
        self.store.delete_project(id)?;
        self.broadcast(ServerEvent::EntityRemoved {
            id: EntityId::Project(id.clone()),
        });
        Ok(())
    }

    // ---- worktrees ----

    pub async fn create_worktree(
        self: &Arc<Self>,
        project_id: &ProjectId,
        branch: &str,
        base: Option<&str>,
    ) -> Result<EntityId> {
        let _ops = self.worktree_ops.lock().await;
        let project = self
            .store
            .get_project(project_id)?
            .context("project not found")?;
        let branch = &match branch.trim() {
            "" => {
                let taken = git::list_branches(&project.repo_path)
                    .await
                    .unwrap_or_default();
                berth_core::branch_name::random_name(&taken)
            }
            named => named.to_string(),
        };
        let path = git::add_worktree(&project.repo_path, branch, base).await?;
        // Before the row exists, so the agent that `berth worktree` moves in
        // finds its `.env` and skills already there rather than seconds later.
        crate::worktree_seed::seed(&project.repo_path, &path).await;
        crate::shared_setup::sync_checkout(&project, &path);
        let worktree = Worktree {
            id: WorktreeId::generate(),
            project_id: project_id.clone(),
            path,
            branch: branch.to_string(),
            is_main: false,
            sort_order: 0,
        };
        self.store.insert_worktree(&worktree)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Worktree(worktree.clone()),
        });
        self.run_project_setup(&worktree);
        Ok(EntityId::Worktree(worktree.id))
    }

    pub async fn delete_worktree(self: &Arc<Self>, id: &WorktreeId, force: bool) -> Result<()> {
        let _ops = self.worktree_ops.lock().await;
        let worktree = self.store.get_worktree(id)?.context("worktree not found")?;
        if worktree.is_main {
            bail!("cannot delete the main checkout — remove the project instead");
        }
        let project = self
            .store
            .get_project(&worktree.project_id)?
            .context("project not found")?;

        // Kill sessions living in this worktree.
        let (_, _, agents, terminals) = self.store.load_tree()?;
        self.kill_sessions_in(std::slice::from_ref(id), &agents, &terminals);

        git::remove_worktree(&project.repo_path, &worktree.path, force).await?;
        // Keep the resumable conversations before the FK cascade takes the
        // agent rows: their CLI session ids are the only way back into
        // transcripts the agent CLI itself never deletes. A failure here
        // costs reachability, not the delete the user asked for.
        if let Err(e) = self.store.orphan_sessions_in_worktree(id) {
            tracing::warn!(worktree = %id, error = %e, "keeping orphaned sessions failed");
        }
        self.store.delete_worktree(id)?;
        self.broadcast(ServerEvent::EntityRemoved {
            id: EntityId::Worktree(id.clone()),
        });
        Ok(())
    }

    // ---- orphaned sessions ----

    /// The resumable conversations of `project`, newest first: every
    /// ORPHANED SESSION
    pub fn list_orphaned_sessions(
        &self,
        project: &ProjectId,
        include_live: bool,
    ) -> Result<Vec<OrphanedSession>> {
        let (project, live) = self.project_with_worktrees(project)?;
        crate::orphans::list(&self.store, &project, &live, include_live)
    }

    /// A project and the checkouts it currently has, the pair every orphan
    /// lookup needs: the scan can only call a conversation orphaned by
    /// knowing which checkouts still exist.
    pub(crate) fn project_with_worktrees(
        &self,
        id: &ProjectId,
    ) -> Result<(Project, Vec<Worktree>)> {
        let project = self.store.get_project(id)?.context("project not found")?;
        let (_, worktrees, _, _) = self.store.load_tree()?;
        let live = worktrees
            .into_iter()
            .filter(|w| w.project_id == project.id)
            .collect();
        Ok((project, live))
    }

    /// Reconcile a project's worktree rows with `git worktree list` so
    /// checkouts made outside berth (an agent running `git worktree add`,
    /// manual CLI use) appear without a restart. Adopts unknown checkouts;
    /// refreshes the branch on known rows after an in-place checkout;
    /// drops rows whose checkout vanished — except the main row and rows
    /// that still have sessions, which the user must delete deliberately.
    pub async fn sync_project_worktrees(self: &Arc<Self>, project: &Project) -> Result<()> {
        let adopted = {
            let _ops = self.worktree_ops.lock().await;
            self.reconcile_project_worktrees(project).await?
        };
        // Outside the ops lock: the replay only touches agent rows, and a
        // just-adopted checkout is exactly where a session that ran
        // `git worktree add` itself already lives.
        if adopted {
            self.reparent_agents_by_last_cwd(project);
        }
        Ok(())
    }

    /// The reconcile half of `sync_project_worktrees`. Returns whether any
    /// checkout was newly adopted.
    async fn reconcile_project_worktrees(self: &Arc<Self>, project: &Project) -> Result<bool> {
        let mut adopted = false;
        let entries = git::list_worktrees(&project.repo_path).await?;
        // git lists the main checkout first, and that — not the order rows
        // happened to be inserted in — is what makes a row the ⌂ root row.
        // Deriving it here every pass repairs a project whose rows were seeded
        // before the root was known, and keeps root-ness following the repo
        // when the checkouts underneath it change.
        let main_path = entries.first().map(|e| e.path.clone());
        let is_root = |path: &Path| main_path.as_deref() == Some(path);
        let (_, worktrees, agents, terminals) = self.store.load_tree()?;
        let ours: Vec<&Worktree> = worktrees
            .iter()
            .filter(|w| w.project_id == project.id)
            .collect();
        for entry in &entries {
            if let Some(known) = ours.iter().find(|w| w.path == entry.path) {
                // Branch switched in place (checkout on the root or inside a
                // linked worktree): refresh the stored name so the row tracks
                // reality instead of the branch at adoption time.
                let root = is_root(&entry.path);
                if known.branch != entry.branch || known.is_main != root {
                    if known.branch != entry.branch {
                        self.store
                            .update_worktree_branch(&known.id, &entry.branch)?;
                    }
                    if known.is_main != root {
                        self.store.set_worktree_main(&known.id, root)?;
                    }
                    let mut updated = (*known).clone();
                    updated.branch = entry.branch.clone();
                    updated.is_main = root;
                    self.broadcast(ServerEvent::EntityUpserted {
                        entity: Entity::Worktree(updated),
                    });
                }
                continue;
            }
            let worktree = Worktree {
                id: WorktreeId::generate(),
                project_id: project.id.clone(),
                is_main: is_root(&entry.path),
                path: entry.path.clone(),
                branch: entry.branch.clone(),
                sort_order: 0,
            };
            crate::shared_setup::insert_adopted_worktree(&self.store, project, &worktree)?;
            adopted = true;
            self.broadcast(ServerEvent::EntityUpserted {
                entity: Entity::Worktree(worktree),
            });
        }
        for w in ours {
            // The main checkout is always somewhere in git's list, so a row
            // that isn't there is a linked checkout that went away — including
            // one still carrying an `is_main` from before root-ness was
            // derived, which no longer earns the row a reprieve.
            if entries.iter().any(|e| e.path == w.path) {
                continue;
            }
            let occupied = agents.iter().any(|a| a.worktree_id == w.id)
                || terminals.iter().any(|t| t.worktree_id == w.id);
            if occupied {
                continue;
            }
            self.store.delete_worktree(&w.id)?;
            self.broadcast(ServerEvent::EntityRemoved {
                id: EntityId::Worktree(w.id.clone()),
            });
        }
        Ok(adopted)
    }

    // ---- agents ----

    pub(crate) async fn create_agent(self: &Arc<Self>, spec: CreateAgentSpec) -> Result<EntityId> {
        let CreateAgentSpec {
            worktree: worktree_id,
            name,
            kind,
            model,
            effort,
            auto_title,
            starting_prompt,
        } = spec;
        if kind.program().is_none() {
            bail!(
                "{} is created with its provider (CreateAcpAgent)",
                kind.as_str()
            );
        }
        let starting_prompt = match starting_prompt {
            Some(prompt) => Some(validate_starting_prompt(&prompt)?),
            None => None,
        };
        let worktree = self
            .store
            .get_worktree(&worktree_id)?
            .context("worktree not found")?;
        // A warm session for this (worktree, kind) hands over its PTY and
        // its pre-generated id — the CLI booted while the user typed the
        // name, so the create feels instant.
        let adopted = match &starting_prompt {
            None => self.take_prewarmed(&worktree_id, kind, model.as_deref(), effort.as_deref()),
            Some(prompt) => {
                self.take_prewarmed_for_prompt(
                    &worktree_id,
                    kind,
                    model.as_deref(),
                    effort.as_deref(),
                    prompt,
                )
                .await
            }
        };
        // Only the cold path needs asking: an adopted warm session is proof
        // the CLI runs. Without this, a missing CLI still "succeeds" — the
        // login shell prints `command not found` into a PTY that dies at
        // once, leaving a dead row that looks identical to a fresh one.
        if adopted.is_none() && !self.cli_available_for_create(kind).await {
            bail!("{}", cli_missing_message(kind));
        }
        let agent = Agent {
            id: adopted
                .as_ref()
                .map(|e| e.agent_id.clone())
                .unwrap_or_else(AgentId::generate),
            worktree_id,
            name: if name.trim().is_empty() {
                "agent".into()
            } else {
                name.trim().to_string()
            },
            status: AgentStatus::Fresh,
            archived: false,
            archived_at: 0,
            ended_at: 0,
            unseen: false,
            kind,
            model,
            effort,
            session_id: None,
            sort_order: 0,
            status_changed_at: epoch_ms(),
            alive: false,
            suggested_prompt: None,
            provider_error: None,
            routed_from: None,
            route_suggestion: None,
            acp_provider: None,
            backend: Backend::Terminal,
        };
        self.store
            .insert_agent_with_auto_title(&agent, auto_title)?;
        if adopted.is_none() {
            // Cold path: boot the CLI right away.
            let spawned = self.spawn_agent_session_with(
                &agent,
                &worktree,
                DEFAULT_COLS,
                DEFAULT_ROWS,
                starting_prompt.as_deref(),
            );
            self.rollback_agent_on_spawn_error(&agent.id, spawned)?;
        }
        let mut broadcast_agent = agent.clone();
        broadcast_agent.alive = true;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Agent(Box::new(broadcast_agent)),
        });
        if let Some(entry) = adopted {
            // Now that the row exists, replay the hooks the warm CLI fired
            // before adoption (SessionStart stores the resume session id).
            for (event, sid) in entry.hooks_before_adoption {
                self.apply_hook_event(&agent.id, event, sid);
            }
            if let Some(prompt) = starting_prompt {
                self.type_starting_prompt(&agent.id, prompt);
            }
        }
        Ok(EntityId::Agent(agent.id))
    }

    pub(crate) fn rollback_agent_on_spawn_error<T>(
        &self,
        id: &AgentId,
        result: Result<T>,
    ) -> Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(spawn_error) => {
                if let Err(rollback_error) = self.store.delete_agent(id) {
                    return Err(spawn_error.context(format!(
                        "agent spawn failed and its database rollback also failed: {rollback_error:#}"
                    )));
                }
                Err(spawn_error)
            }
        }
    }

    /// Kill every live agent and terminal PTY homed in these worktrees, and
    /// the warm spares with them — the prelude to dropping their rows
    /// (worktree delete, project remove).
    fn kill_sessions_in(
        &self,
        worktree_ids: &[WorktreeId],
        agents: &[Agent],
        terminals: &[TerminalTab],
    ) {
        for a in agents
            .iter()
            .filter(|a| worktree_ids.contains(&a.worktree_id))
        {
            self.kill_session(&SessionRef::Agent(a.id.clone()));
        }
        for t in terminals
            .iter()
            .filter(|t| worktree_ids.contains(&t.worktree_id))
        {
            self.kill_session(&SessionRef::Terminal(t.id.clone()));
        }
        self.kill_prewarmed_in(worktree_ids);
    }

    /// Is the kind's CLI on the user's PATH (as their login shell sees it)?
    /// Cached: hits for an hour, misses for a minute so a just-installed CLI
    /// gets picked up quickly. Probe trouble (timeout, spawn error) fails
    /// open — a doomed warm spawn is still graceful.
    pub(crate) async fn cli_available(&self, kind: AgentKind) -> bool {
        if env::var(env::AGENT_CMD).is_some() {
            return true; // test override is spawned verbatim
        }
        const OK_TTL: Duration = Duration::from_secs(3600);
        const FAIL_TTL: Duration = Duration::from_secs(60);
        {
            let probes = self.cli_probes.lock().unwrap();
            if let Some((ok, at)) = probes.get(&kind) {
                if at.elapsed() < if *ok { OK_TTL } else { FAIL_TTL } {
                    return *ok;
                }
            }
        }
        self.probe_cli(kind).await
    }

    /// Fill the availability cache for every kind at boot, off the request
    /// loop. Without it the first CreateAgent of a session pays a full
    /// login-shell probe (~1s with a heavy ~/.zshrc) before it can answer.
    pub async fn warm_cli_probes(self: &Arc<Self>) {
        for kind in AgentKind::NATIVE {
            self.cli_available(kind).await;
        }
    }

    /// Same question, asked on behalf of a create the user just triggered.
    /// A cached *hit* is trusted; a cached *miss* is re-probed, so someone who
    /// installs the CLI and immediately retries isn't told for another minute
    /// that it's missing. Misses are rare, so this costs nothing in practice.
    pub(crate) async fn cli_available_for_create(&self, kind: AgentKind) -> bool {
        self.cli_available(kind).await || self.probe_cli(kind).await
    }

    /// Uncached "is this CLI on the PATH it will be launched with"; caches
    /// the answer. *How* the question is asked belongs to the launch
    /// environment (`crate::launch`) — it differs per platform, the caching
    /// does not.
    ///
    /// A probe that could not answer at all reports available and caches
    /// nothing: refusing a create over a shell that would not start would
    /// blame the user's CLI for the daemon's problem.
    async fn probe_cli(&self, kind: AgentKind) -> bool {
        match launch::program_is_installed(kind.program().unwrap_or_default(), CLI_PROBE_TIMEOUT)
            .await
        {
            Some(ok) => {
                self.cli_probes
                    .lock()
                    .unwrap()
                    .insert(kind, (ok, Instant::now()));
                ok
            }
            None => true,
        }
    }

    pub fn rename_agent(self: &Arc<Self>, id: &AgentId, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("name is empty");
        }
        self.store.rename_agent(id, name.trim())?;
        self.broadcast_agent(id)?;
        Ok(())
    }

    /// Agent-initiated one-shot title (`berth rename` inside the session's
    /// CLI). Applies only while the auto-title is still pending; afterwards
    /// it reports the standing title as an error so the CLI (and the model
    /// reading its output) knows nothing changed.
    pub fn auto_rename_agent(self: &Arc<Self>, id: &AgentId, name: &str) -> Result<()> {
        let title = sanitize_title(name);
        if title.is_empty() {
            bail!("title is empty");
        }
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if !self.store.rename_agent_if_auto_pending(id, &title)? {
            bail!(
                "session already has a title ({:?}); leaving it unchanged — a user-set \
                 title is only replaced with `berth rename --force`",
                agent.name
            );
        }
        self.broadcast_agent(id)?;
        Ok(())
    }

    /// Re-home an agent row under another worktree of the same project. A
    /// live PTY still runs — and its hooks still report a cwd — inside the
    /// old checkout, so left alone `reparent_agent_by_cwd` would snap the
    /// row straight back on the next hook event: kill it and respawn resumed
    /// in the target so the process and the row agree. A respawn failure
    /// degrades to a dead session the next attach revives via
    /// `ensure_session`.
    pub fn move_agent(self: &Arc<Self>, id: &AgentId, worktree_id: &WorktreeId) -> Result<()> {
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if &agent.worktree_id == worktree_id {
            return Ok(());
        }
        let target = self.sibling_worktree(&agent, worktree_id)?;
        self.pending_moves.lock().unwrap().remove(id);
        let sref = SessionRef::Agent(id.clone());
        let was_alive = self.session(&sref).is_some();
        if was_alive {
            self.kill_session(&sref);
        }
        // A deliberate move invalidates the remembered hook cwd: it still
        // points at the old checkout, and the next worktree sync would
        // replay it straight back over the user's choice.
        self.last_cwd.lock().unwrap().remove(id);
        self.store.set_agent_worktree(id, worktree_id)?;
        if was_alive {
            if let Err(e) = self.spawn_agent_session(&agent, &target, DEFAULT_COLS, DEFAULT_ROWS) {
                tracing::warn!(agent = %id, error = %e, "respawn after move failed");
            }
        }
        self.broadcast_agent(id)?;
        Ok(())
    }

    /// The worktree `worktree_id`, checked to belong to the same project as
    /// `agent`'s current one — the only kind of move a row can make.
    fn sibling_worktree(&self, agent: &Agent, worktree_id: &WorktreeId) -> Result<Worktree> {
        let target = self
            .store
            .get_worktree(worktree_id)?
            .context("worktree not found")?;
        let current = self
            .store
            .get_worktree(&agent.worktree_id)?
            .context("worktree not found")?;
        if target.project_id != current.project_id {
            bail!("target worktree belongs to a different project");
        }
        Ok(target)
    }

    /// `berth worktree <branch>`, run by the agent inside its own session.
    /// The row moves under `branch`'s worktree of the same project now —
    /// created when the project has no checkout for that branch yet — and
    /// a live PTY follows once its turn ends (`complete_pending_move`),
    /// because the CLI running this command *is* that PTY's foreground
    /// tool call: killing it here would cut the turn off mid-answer.
    pub async fn enter_worktree(
        self: &Arc<Self>,
        id: &AgentId,
        branch: &str,
        base: Option<&str>,
    ) -> Result<(Worktree, EnterOutcome, Option<String>)> {
        let branch = branch.trim();
        if branch.is_empty() {
            bail!("branch name is empty");
        }
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if agent.archived {
            bail!("agent is archived");
        }
        let current = self
            .store
            .get_worktree(&agent.worktree_id)?
            .context("worktree not found")?;
        let (_, worktrees, _, _) = self.store.load_tree()?;
        let existing = worktrees
            .into_iter()
            .find(|w| w.project_id == current.project_id && w.branch == branch);
        let (target, warning) = match existing {
            Some(w) => (w, None),
            None => {
                self.create_worktree_from_base(&current.project_id, branch, base)
                    .await?
            }
        };
        if target.id == current.id {
            return Ok((target, EnterOutcome::AlreadyThere, warning));
        }
        let outcome = self.relocate_agent(id, &target)?;
        Ok((target, outcome, warning))
    }

    /// Re-home the row under `target` now and leave a live PTY to follow at
    /// its turn end (`complete_pending_move`) — the shared half of
    /// `berth worktree` and `berth enter`.
    pub(crate) fn relocate_agent(
        self: &Arc<Self>,
        id: &AgentId,
        target: &Worktree,
    ) -> Result<EnterOutcome> {
        let alive = self.is_alive(&SessionRef::Agent(id.clone()));
        // Same invalidation as `move_agent`: every cwd this process reports
        // until it respawns is the old checkout's.
        self.last_cwd.lock().unwrap().remove(id);
        if alive {
            self.pending_moves
                .lock()
                .unwrap()
                .insert(id.clone(), target.clone());
        }
        self.store.set_agent_worktree(id, &target.id)?;
        self.broadcast_agent(id)?;
        Ok(if alive {
            EnterOutcome::Relocating
        } else {
            EnterOutcome::NextLaunch
        })
    }

    /// The turn an agent ran `berth worktree` in has ended: make the
    /// process match its row. Kill it and respawn it resumed in the target,
    /// with a prompt naming the checkout it now runs in so the conversation
    /// carries straight on (Claude takes that prompt as an argument; codex
    /// resumes silent and waits for the user). Gated on the
    /// turn-end hooks — Stop, and the idle notification a Stop-less end
    /// still fires — so a Bash hook from the same turn never triggers it.
    pub fn complete_pending_move(self: &Arc<Self>, id: &AgentId, event: &HookEvent) {
        if !crate::self_end::is_turn_end(event) {
            return;
        }
        let Some(target) = self.pending_moves.lock().unwrap().remove(id) else {
            return;
        };
        if self.pending_ends.lock().unwrap().contains(id) {
            return;
        }
        let agent = match self.store.get_agent(id) {
            Ok(Some(agent)) if !agent.archived && agent.worktree_id == target.id => agent,
            // Archived, deleted, or moved elsewhere by hand since: the
            // row's current home wins, nothing to relocate into.
            _ => return,
        };
        let sref = SessionRef::Agent(id.clone());
        if !self.is_alive(&sref) {
            // Died since (or the user closed it): the next launch boots in
            // the target on its own, only without the relocation notice.
            return;
        }
        tracing::info!(agent = %id, to = %target.branch, "relocating session into its worktree");
        if agent.backend == Backend::Chat {
            self.acp_relocate(&agent, &target);
            return;
        }
        self.kill_session(&sref);
        self.last_cwd.lock().unwrap().remove(id);
        self.carry_claude_transcript(&agent, &target);
        // Claude only: its `--resume <sid> "<prompt>"` is verified; whether
        // `codex resume` takes a trailing prompt is not, so its relocated
        // sessions keep waiting for the user.
        let prompt = relocation_prompt(&target);
        let prompt = agent::terminal(agent.kind)
            .is_some_and(|terminal| terminal.prompt_on_resume)
            .then_some(prompt.as_str());
        if let Err(e) =
            self.spawn_agent_session_with(&agent, &target, DEFAULT_COLS, DEFAULT_ROWS, prompt)
        {
            tracing::warn!(agent = %id, error = %e, "respawn after worktree relocation failed");
        }
        self.try_broadcast_agent(id);
    }

    /// Whether `id` is between `enter_worktree` and its respawn.
    #[cfg(test)]
    fn relocation_pending(&self, id: &AgentId) -> bool {
        self.pending_moves.lock().unwrap().contains_key(id)
    }

    /// Row-only re-home: store update plus broadcast, never the PTY. The
    /// hook-cwd reparent uses this — there the process already runs in the
    /// target checkout and only the row is stale, so killing it would
    /// interrupt a live conversation for nothing.
    fn move_agent_row(self: &Arc<Self>, id: &AgentId, worktree_id: &WorktreeId) -> Result<()> {
        self.store.set_agent_worktree(id, worktree_id)?;
        self.broadcast_agent(id)?;
        Ok(())
    }

    /// A hook payload reported the agent CLI's working directory. When that
    /// directory sits inside a *different* worktree of the same project (the
    /// session entered a worktree it created mid-conversation), re-home the
    /// agent row so the tree reflects where the work actually happens.
    /// Fail-soft: any error leaves the row where it is.
    pub fn reparent_agent_by_cwd(
        self: &Arc<Self>,
        agent_id: &AgentId,
        cwd: &str,
        payload_session_id: Option<&str>,
        captures_session: bool,
    ) {
        if let Err(e) =
            self.try_reparent_agent_by_cwd(agent_id, cwd, payload_session_id, captures_session)
        {
            tracing::warn!(agent = %agent_id, error = %e, "cwd reparent failed");
        }
    }

    fn try_reparent_agent_by_cwd(
        self: &Arc<Self>,
        agent_id: &AgentId,
        cwd: &str,
        payload_session_id: Option<&str>,
        captures_session: bool,
    ) -> Result<()> {
        let Some(agent) = self.store.get_agent(agent_id)? else {
            self.last_cwd.lock().unwrap().remove(agent_id);
            return Ok(());
        };
        if agent.archived || agent.is_ended() {
            self.last_cwd.lock().unwrap().remove(agent_id);
            return Ok(());
        }
        // Mid-relocation the row already sits under the target while the
        // process still reports the old checkout — ignore it until the
        // respawn lands there.
        if self.pending_moves.lock().unwrap().contains_key(agent_id) {
            return Ok(());
        }
        // Same foreign-session rule as the status machine: a payload from a
        // different CLI session only counts when the event (re)establishes
        // session ownership (UserPromptSubmit / SessionStart).
        if !captures_session {
            if let (Some(mine), Some(theirs)) = (agent.session_id.as_deref(), payload_session_id) {
                if mine != theirs {
                    return Ok(());
                }
            }
        }
        let cwd = paths::canonical_or_raw(Path::new(cwd));
        // Remembered even when it resolves to nothing: an agent that just ran
        // `git worktree add` and stepped into the result reports a cwd berth
        // has no row for yet, and the worktree sync replays this to finish the
        // re-home the moment that row is adopted.
        self.last_cwd
            .lock()
            .unwrap()
            .insert(agent_id.clone(), cwd.clone());
        self.reparent_agent_to_cwd(&agent, &cwd)
    }

    /// Move `agent`'s row under the worktree owning `cwd` when that is a
    /// different worktree of the same project. `cwd` must already be
    /// canonicalized.
    fn reparent_agent_to_cwd(self: &Arc<Self>, agent: &Agent, cwd: &Path) -> Result<()> {
        let Some(current) = self.store.get_worktree(&agent.worktree_id)? else {
            return Ok(());
        };
        let (_, worktrees, _, _) = self.store.load_tree()?;
        // Deepest worktree of the same project containing cwd — nested
        // layouts (checkouts under the repo root) must not resolve to the
        // root row just because the root path is also a prefix.
        let target = worktrees
            .into_iter()
            .filter(|w| w.project_id == current.project_id)
            .map(|w| {
                let canonical = paths::canonical_or_raw(&w.path);
                (w, canonical)
            })
            // Both sides went through `canonical_or_raw` above — the
            // pairing `paths::contains` requires.
            .filter(|(_, canonical)| paths::contains(canonical, cwd))
            .max_by_key(|(_, canonical)| canonical.components().count());
        if let Some((worktree, _)) = target {
            if worktree.id != agent.worktree_id {
                tracing::info!(
                    agent = %agent.id,
                    from = %current.branch,
                    to = %worktree.branch,
                    "agent re-homed by hook cwd"
                );
                self.move_agent_row(&agent.id, &worktree.id)?;
            }
        }
        Ok(())
    }

    /// Replay remembered hook cwds for `project`'s agents. Runs after the
    /// worktree sync adopts checkouts: a session that creates a worktree and
    /// enters it reports the new cwd (often on the very next `Stop`) before
    /// the row exists, and without this replay its row would sit under the
    /// old checkout until the user's next prompt.
    fn reparent_agents_by_last_cwd(self: &Arc<Self>, project: &Project) {
        let known: Vec<(AgentId, PathBuf)> = {
            let map = self.last_cwd.lock().unwrap();
            map.iter().map(|(id, p)| (id.clone(), p.clone())).collect()
        };
        for (agent_id, cwd) in known {
            let agent = match self.store.get_agent(&agent_id) {
                Ok(Some(agent)) => agent,
                Ok(None) => {
                    self.last_cwd.lock().unwrap().remove(&agent_id);
                    continue;
                }
                Err(e) => {
                    tracing::warn!(agent = %agent_id, error = %e, "cwd replay lookup failed");
                    continue;
                }
            };
            if agent.archived || agent.is_ended() {
                continue;
            }
            let in_project = matches!(
                self.store.get_worktree(&agent.worktree_id),
                Ok(Some(w)) if w.project_id == project.id
            );
            if !in_project {
                continue;
            }
            if let Err(e) = self.reparent_agent_to_cwd(&agent, &cwd) {
                tracing::warn!(agent = %agent_id, error = %e, "cwd replay reparent failed");
            }
        }
    }

    pub fn archive_agent(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        self.pending_ends.lock().unwrap().remove(id);
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.store.set_agent_archived(id, true)?;
        self.broadcast_agent(id)?;
        Ok(())
    }

    pub fn unarchive_agent(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        self.pending_ends.lock().unwrap().remove(id);
        self.store.set_agent_archived(id, false)?;
        self.store.set_agent_ended(id, false)?;
        self.broadcast_agent(id)?;
        Ok(())
    }

    /// A client put this agent's session on screen: its unseen-finish flag
    /// (`Agent::unseen`) is cleared, and every subscriber gets the row so
    /// their counts drop together. Nothing is sent when the flag was
    /// already clear — re-attaching to a session you've read is free.
    pub fn mark_agent_seen(&self, id: &AgentId) -> Result<()> {
        if self.store.mark_agent_seen(id)? {
            self.broadcast_agent(id)?;
        }
        Ok(())
    }

    pub fn delete_agent(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.last_cwd.lock().unwrap().remove(id);
        self.pending_moves.lock().unwrap().remove(id);
        self.pending_ends.lock().unwrap().remove(id);
        self.store.delete_agent(id)?;
        self.broadcast(ServerEvent::EntityRemoved {
            id: EntityId::Agent(id.clone()),
        });
        Ok(())
    }

    pub async fn restart_agent(self: &Arc<Self>, id: &AgentId) -> Result<()> {
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if agent.archived {
            bail!("agent is archived — unarchive it first");
        }
        let worktree = self
            .store
            .get_worktree(&agent.worktree_id)?
            .context("worktree not found")?;
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.spawn_agent_session(&agent, &worktree, DEFAULT_COLS, DEFAULT_ROWS)?;
        let mut broadcast_agent = agent.clone();
        broadcast_agent.alive = true;
        broadcast_agent.ended_at = 0;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Agent(Box::new(broadcast_agent)),
        });
        Ok(())
    }

    // ---- terminals ----

    pub fn create_terminal(
        self: &Arc<Self>,
        worktree_id: &WorktreeId,
        name: Option<String>,
    ) -> Result<EntityId> {
        let worktree = self
            .store
            .get_worktree(worktree_id)?
            .context("worktree not found")?;
        let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
            let n = self.store.count_terminals(worktree_id).unwrap_or(0);
            format!("term-{}", n + 1)
        });
        let terminal = TerminalTab {
            id: TerminalId::generate(),
            worktree_id: worktree_id.clone(),
            name,
            sort_order: 0,
            alive: false,
            session_id: None,
        };
        self.store.insert_terminal(&terminal)?;
        self.spawn_terminal_session(&terminal, &worktree, DEFAULT_COLS, DEFAULT_ROWS)?;
        let mut broadcast_term = terminal.clone();
        broadcast_term.alive = true;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Terminal(broadcast_term),
        });
        Ok(EntityId::Terminal(terminal.id))
    }

    pub fn rename_terminal(self: &Arc<Self>, id: &TerminalId, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("name is empty");
        }
        self.store.rename_terminal(id, name.trim())?;
        let term = self.terminal_entity(id)?;
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Terminal(term),
        });
        Ok(())
    }

    pub fn close_terminal(self: &Arc<Self>, id: &TerminalId) -> Result<()> {
        self.kill_session(&SessionRef::Terminal(id.clone()));
        self.store.delete_terminal(id)?;
        self.broadcast(ServerEvent::EntityRemoved {
            id: EntityId::Terminal(id.clone()),
        });
        Ok(())
    }

    // ---- attach / spawn ----

    /// Get the live session for an entity, lazily (re)spawning its PTY when
    /// none is running (restored agents, closed shells).
    pub fn ensure_session(
        self: &Arc<Self>,
        sref: &SessionRef,
        cols: u16,
        rows: u16,
    ) -> Result<Arc<PtySession>> {
        if let Some(s) = self.session(sref) {
            return Ok(s);
        }
        // Hold the gate across the whole check-and-install. Re-check under
        // it — the winner installed while we waited.
        let _gate = self.spawn_gate.lock().unwrap();
        if let Some(s) = self.session(sref) {
            return Ok(s);
        }
        match sref {
            SessionRef::Agent(id) => {
                let agent = self.store.get_agent(id)?.context("agent not found")?;
                if agent.archived {
                    bail!("agent is archived — unarchive it first");
                }
                let worktree = self
                    .store
                    .get_worktree(&agent.worktree_id)?
                    .context("worktree not found")?;
                let session = self.spawn_agent_session(&agent, &worktree, cols, rows)?;
                let mut broadcast_agent = agent;
                broadcast_agent.alive = true;
                broadcast_agent.ended_at = 0;
                self.broadcast(ServerEvent::EntityUpserted {
                    entity: Entity::Agent(Box::new(broadcast_agent)),
                });
                Ok(session)
            }
            SessionRef::Terminal(id) => {
                let term = self.store.get_terminal(id)?.context("terminal not found")?;
                let worktree = self
                    .store
                    .get_worktree(&term.worktree_id)?
                    .context("worktree not found")?;
                let session = self.spawn_terminal_session(&term, &worktree, cols, rows)?;
                crate::terminal_resume::resume_expected_claude(id, &session);
                let mut broadcast_term = term;
                broadcast_term.alive = true;
                self.broadcast(ServerEvent::EntityUpserted {
                    entity: Entity::Terminal(broadcast_term),
                });
                Ok(session)
            }
        }
    }

    pub(crate) fn spawn_agent_session(
        self: &Arc<Self>,
        agent: &Agent,
        worktree: &Worktree,
        cols: u16,
        rows: u16,
    ) -> Result<Arc<PtySession>> {
        self.spawn_agent_session_with(agent, worktree, cols, rows, None)
    }

    /// `initial_prompt` a first turn the CLI submits on its own
    /// (the relocation notice a `berth worktree` respawn opens with, or the
    /// prefix + task + postfix an AGENT PRESET launch composes).
    pub(crate) fn spawn_agent_session_with(
        self: &Arc<Self>,
        agent: &Agent,
        worktree: &Worktree,
        cols: u16,
        rows: u16,
        initial_prompt: Option<&str>,
    ) -> Result<Arc<PtySession>> {
        // Whatever spawns this agent, it runs in `worktree` from here: a
        // relocation still pending for it has been overtaken.
        let Some(terminal) = agent::terminal(agent.kind) else {
            bail!("{} runs only in the chat view", agent.kind.as_str());
        if agent.backend == Backend::Chat {
            bail!("an ACP agent has no terminal");
        }
        };
        self.pending_moves.lock().unwrap().remove(&agent.id);
        self.pending_ends.lock().unwrap().remove(&agent.id);
        // Managed status hooks; a failure here degrades to "no status
        // updates", never blocks the spawn.
        if let Err(e) = (terminal.hooks.install)(&worktree.path) {
            tracing::warn!(error = %e, cwd = %worktree.path.display(), "hook install failed");
        }

        // BERTH_AGENT_CMD overrides for tests; default is the kind's CLI.
        let cmd_override = env::var(env::AGENT_CMD);
        let pr_system_prompt = self
            .store
            .agent_pr_url(&agent.id)?
            .as_deref()
            .map(agent::claude::pr_system_prompt);
        let unsaved_conversation = resume_check::claude_projects_root().is_some_and(|root| {
            resume_check::claude_transcript_missing(agent.kind, agent.session_id.as_deref(), &root)
        });
        let fresh_agent;
        let agent = if unsaved_conversation {
            tracing::info!(agent = %agent.id, "no transcript for the stored session id, starting fresh");
            let _ = self.store.set_agent_session_id(&agent.id, None);
            let mut fresh = agent.clone();
            fresh.session_id = None;
            fresh_agent = fresh;
            &fresh_agent
        } else {
            agent
        };
        let launch_args = agent::Launch {
            session: agent.session_id.as_deref(),
            model: agent.model.as_deref(),
            effort: agent.effort.as_deref(),
            prompt: initial_prompt,
            system_prompt: pr_system_prompt.as_deref(),
        };
        let (program, args, resumed) = terminal.command(&launch_args, cmd_override.as_deref());
        let args = if cmd_override.is_some() {
            args
        } else {
            [(terminal.leading_args)(), args].concat()
        };
        // Put the agent in the environment the user's own terminal would
        // give it (see `crate::launch`). Overrides (tests) stay verbatim.
        let (program, args) = if cmd_override.is_some() {
            (program, args)
        } else {
            launch::wrap_for_user_env(&program, &args)
        };

        let mut agent_env = vec![
            (env::AGENT_ID.into(), agent.id.to_string()),
            (
                env::API_URL.into(),
                format!("http://127.0.0.1:{}", self.hook_env.port),
            ),
            (env::API_TOKEN.into(), self.hook_env.token.clone()),
        ];
        agent_env.extend((terminal.env)());
        let spec = SpawnSpec {
            program,
            args,
            cwd: worktree.path.clone(),
            env: agent_env,
            scrub_env: env::AGENT_SESSION_VARS,
            cols,
            rows,
        };
        let sref = SessionRef::Agent(agent.id.clone());
        let session = PtySession::spawn(sref, spec)?;
        self.install_session(session.clone());
        self.leave_ended(&agent.id);
        if unsaved_conversation {
            session.print_notice(resume_check::FRESH_START_NOTICE);
        }
        if terminal.suggestion_scan {
            session.arm_suggestion_scan();
        }
        if resumed {
            self.arm_resume_fallback(agent.clone(), worktree.clone(), session.clone(), cols, rows);
        }
        Ok(session)
    }

    /// A resumed session (`claude --resume` / `codex resume`) dies fast when
    /// it is stale/deleted — fall back to a fresh session instead of leaving
    /// a dead pane.
    fn arm_resume_fallback(
        self: &Arc<Self>,
        agent: Agent,
        worktree: Worktree,
        session: Arc<PtySession>,
        cols: u16,
        rows: u16,
    ) {
        let daemon = self.clone();
        let mut rx = session.events.subscribe();
        tokio::spawn(async move {
            let early_exit = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    match rx.recv().await {
                        Ok(PtyEvent::Exited { exit_code }) => return exit_code.unwrap_or(1) != 0,
                        Ok(_) => continue,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => return false,
                    }
                }
            })
            .await;
            if early_exit != Ok(true) {
                return;
            }
            // A deliberate kill looks identical to a failed resume from here:
            // the agent may have been archived or deleted inside the window —
            // never resurrect those.
            match daemon.store.get_agent(&agent.id) {
                Ok(Some(current)) if !current.archived && !current.is_ended() => {}
                _ => return,
            }
            tracing::info!(agent = %agent.id, "resume failed fast — respawning fresh");
            let _ = daemon.store.set_agent_session_id(&agent.id, None);
            let mut fresh = agent.clone();
            fresh.session_id = None;
            if let Ok(_session) = daemon.spawn_agent_session(&fresh, &worktree, cols, rows) {
                let mut broadcast_agent = fresh;
                broadcast_agent.alive = true;
                broadcast_agent.ended_at = 0;
                daemon.broadcast(ServerEvent::EntityUpserted {
                    entity: Entity::Agent(Box::new(broadcast_agent)),
                });
            }
        });
    }

    fn spawn_terminal_session(
        self: &Arc<Self>,
        terminal: &TerminalTab,
        worktree: &Worktree,
        cols: u16,
        rows: u16,
    ) -> Result<Arc<PtySession>> {
        let (program, args) = launch::interactive_shell();
        let spec = SpawnSpec {
            program,
            args,
            cwd: worktree.path.clone(),
            env: vec![],
            scrub_env: env::AGENT_SESSION_VARS,
            cols,
            rows,
        };
        let sref = SessionRef::Terminal(terminal.id.clone());
        let session = PtySession::spawn(sref, spec)?;
        self.install_session(session.clone());
        Ok(session)
    }

    fn install_session(self: &Arc<Self>, session: Arc<PtySession>) {
        self.touch_session(&session.sref);
        self.sessions
            .lock()
            .unwrap()
            .insert(session.sref.clone(), session.clone());
        let _ = self.installed_sessions.send(session.clone());
        self.watch_for_exit(session);
    }

    /// Once the child dies: drop it from the registry, feed the status
    /// machine (agents), and tell subscribers the entity is no longer alive.
    fn watch_for_exit(self: &Arc<Self>, session: Arc<PtySession>) {
        let daemon = self.clone();
        let mut rx = session.events.subscribe();
        let sref = session.sref.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(PtyEvent::Exited { exit_code }) => {
                        // Deliberate kills (archive/restart/delete) remove the
                        // entry first — only a *natural* death of the still-
                        // registered session drives status, so a restart never
                        // flags the fresh PTY's agent as terminated.
                        let was_registered = {
                            let mut sessions = daemon.sessions.lock().unwrap();
                            match sessions.get(&sref) {
                                Some(current) if Arc::ptr_eq(current, &session) => {
                                    sessions.remove(&sref);
                                    true
                                }
                                _ => false,
                            }
                        };
                        if was_registered {
                            daemon.session_interest.lock().unwrap().remove(&sref);
                        }
                        if !was_registered {
                            break;
                        }
                        tracing::info!(session = ?sref, exit_code, "session exited");
                        if let SessionRef::Agent(id) = &sref {
                            daemon.apply_hook_event(
                                id,
                                HookEvent::SessionEnded { exit_code },
                                None,
                            );
                        }
                        let upsert = match &sref {
                            SessionRef::Agent(id) => daemon
                                .agent_entity(id)
                                .map(|agent| Entity::Agent(Box::new(agent))),
                            SessionRef::Terminal(id) => {
                                daemon.terminal_entity(id).map(Entity::Terminal)
                            }
                        };
                        if let Ok(entity) = upsert {
                            daemon.broadcast(ServerEvent::EntityUpserted { entity });
                        }
                        break;
                    }
                    // The CLI's own busy/idle bit, read off its output. It is
                    // the only end-of-turn news after a user cancel: Claude
                    // Code fires no Stop for an interrupted turn, and
                    // suppresses the idle notification because the user just
                    // pressed a key. See `pty::progress`.
                    Ok(PtyEvent::Progress { busy }) => {
                        if let SessionRef::Agent(id) = &sref {
                            daemon.apply_hook_event(id, HookEvent::Progress { busy }, None);
                        }
                    }
                    // The same news off the window title, for a CLI that no
                    // longer advertises a progress bar at all — Claude Code
                    // 2.1.270 emits no OSC 9;4, so without this a cancelled
                    // turn stays yellow forever. See `pty::title`.
                    Ok(PtyEvent::TitleIdle { idle }) => {
                        if let SessionRef::Agent(id) = &sref {
                            daemon.apply_hook_event(id, HookEvent::TitleIdle { idle }, None);
                        }
                    }
                    Ok(PtyEvent::Suggestion) => {
                        if let SessionRef::Agent(id) = &sref {
                            daemon.try_broadcast_agent(id);
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // A fire-hosing child can push progress edges off the
                        // broadcast queue. The scanner itself never lags, so
                        // reconcile from its current reading rather than
                        // leaving the status stuck on a dropped edge.
                        if let SessionRef::Agent(id) = &sref {
                            if let Some(busy) = session.progress_busy() {
                                daemon.apply_hook_event(id, HookEvent::Progress { busy }, None);
                            }
                            if let Some(idle) = session.title_idle() {
                                daemon.apply_hook_event(id, HookEvent::TitleIdle { idle }, None);
                            }
                        }
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

/// The prompt a relocated Claude session is resumed with: it names the
/// checkout the process now runs in and asks for the work to pick back up
/// there, so the user never has to type "continue".
pub(crate) fn relocation_prompt(worktree: &Worktree) -> String {
    format!(
        "[berth] This session now runs inside the checkout `{}` at {} — your working \
         directory is that checkout. Continue the user's most recent request there.",
        worktree.branch,
        worktree.path.display()
    )
}

/// Validate an AGENT PRESET's composed starting prompt before it becomes the
/// CLI's positional argument.
pub(crate) fn validate_starting_prompt(raw: &str) -> Result<String> {
    let text = raw.trim().to_string();
    if text.is_empty() {
        bail!("starting prompt is empty");
    }
    if text.contains('\0') {
        bail!("starting prompt cannot contain NUL bytes");
    }
    if text.len() > MAX_STARTING_PROMPT_BYTES {
        bail!(
            "starting prompt is too long (max {} KiB)",
            MAX_STARTING_PROMPT_BYTES / 1024
        );
    }
    Ok(text)
}

/// Normalize an agent-supplied title: control characters become spaces,
/// whitespace collapses, and over-long titles are cut — models occasionally
/// hand over a whole sentence no matter what the instruction says.
pub(crate) fn sanitize_title(raw: &str) -> String {
    const MAX_CHARS: usize = 60;
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut title = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.chars().count() > MAX_CHARS {
        title = title.chars().take(MAX_CHARS).collect();
        title.truncate(title.trim_end().len());
    }
    title
}

/// Does this terminal's shell have any child processes (a command or job
/// still running)? An unknown child pid or a failed probe counts as busy —
/// never kill what can't be inspected.
fn shell_has_children(session: &PtySession) -> bool {
    let Some(pid) = session.child_pid else {
        return true;
    };
    berth_core::platform::has_child_processes(pid)
}

/// Why a create was refused when the agent CLI isn't installed. One line —
/// the TUI shows it in the footer flash, which truncates. Unlike git (which
/// the daemon runs with its own inherited PATH), agent CLIs are spawned
/// through the user's login shell, so a fresh install is picked up on the
/// next try with no daemon restart.
pub(crate) fn cli_missing_message(kind: AgentKind) -> String {
    format!(
        "{} was not found on your PATH — install it, then try again.",
        kind.program().unwrap_or_default()
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn test_daemon() -> Arc<Daemon> {
        let store = Arc::new(Store::open_in_memory().unwrap());
        Daemon::new(
            store,
            HookEnv {
                port: 0,
                token: String::new(),
            },
        )
    }

    #[tokio::test]
    async fn starting_prompt_is_validated() {
        let daemon = test_daemon();
        let spec = |kind: AgentKind, starting: Option<&str>| CreateAgentSpec {
            worktree: WorktreeId("unused".into()),
            name: "preset".into(),
            kind,
            model: None,
            effort: None,
            auto_title: true,
            starting_prompt: starting.map(String::from),
        };
        // Validation runs before the worktree lookup, so an unknown
        // worktree is fine here and every failure is the prompt's own.
        for (kind, starting, needle) in [
            (AgentKind::Claude, " \n ", "is empty"),
            (AgentKind::Codex, "fix\0auth", "NUL"),
            (
                AgentKind::Grok,
                &*"x".repeat(MAX_STARTING_PROMPT_BYTES + 1),
                "too long",
            ),
        ] {
            let err = daemon
                .create_agent(spec(kind, Some(starting)))
                .await
                .unwrap_err();
            assert!(err.to_string().contains(needle), "{kind:?}: {err}");
        }
    }

    #[test]
    fn failed_agent_spawn_rolls_back_the_persisted_row() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "w", "/tmp", true);
        seed_agent(&daemon, "a1", "w", None);
        let id = AgentId("a1".into());

        let error = daemon
            .rollback_agent_on_spawn_error(&id, Err::<(), _>(anyhow::anyhow!("spawn failed")))
            .unwrap_err();

        assert!(error.to_string().contains("spawn failed"));
        assert!(daemon.store.get_agent(&id).unwrap().is_none());
    }

    pub(crate) fn seed_projects(daemon: &Daemon, names: &[&str]) {
        for (i, name) in names.iter().enumerate() {
            daemon
                .store
                .insert_project(&Project {
                    workspace_id: Default::default(),
                    id: ProjectId((*name).into()),
                    name: (*name).into(),
                    repo_path: format!("/tmp/{name}").into(),
                    sort_order: i as i64,
                    pin_order: None,
                })
                .unwrap();
        }
    }

    pub(crate) fn seed_worktree(
        daemon: &Daemon,
        project: &str,
        id: &str,
        path: &str,
        is_main: bool,
    ) {
        daemon
            .store
            .insert_worktree(&Worktree {
                id: WorktreeId(id.into()),
                project_id: ProjectId(project.into()),
                path: path.into(),
                branch: id.into(),
                is_main,
                sort_order: 0,
            })
            .unwrap();
    }

    pub(crate) fn seed_agent(daemon: &Daemon, id: &str, worktree: &str, session_id: Option<&str>) {
        daemon
            .store
            .insert_agent(&Agent {
                id: AgentId(id.into()),
                worktree_id: WorktreeId(worktree.into()),
                name: id.into(),
                status: AgentStatus::Running,
                archived: false,
                archived_at: 0,
                ended_at: 0,
                unseen: false,
                kind: AgentKind::Claude,
                model: None,
                effort: None,
                session_id: session_id.map(str::to_string),
                sort_order: 0,
                status_changed_at: 0,
                alive: false,
                suggested_prompt: None,
                provider_error: None,
                routed_from: None,
                route_suggestion: None,
                acp_provider: None,
                backend: Backend::Terminal,
            })
            .unwrap();
    }

    fn agent_worktree(daemon: &Daemon, id: &str) -> String {
        daemon
            .store
            .get_agent(&AgentId(id.into()))
            .unwrap()
            .unwrap()
            .worktree_id
            .to_string()
    }

    #[test]
    fn move_agent_rehomes_row_and_broadcasts() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_worktree(&daemon, "p", "feat", "/berth-test/p-feat", false);
        seed_agent(&daemon, "a1", "root", None);
        let mut rx = daemon.events.subscribe();

        daemon
            .move_agent(&AgentId("a1".into()), &WorktreeId("feat".into()))
            .unwrap();
        assert_eq!(agent_worktree(&daemon, "a1"), "feat");
        match rx.try_recv().unwrap() {
            ServerEvent::EntityUpserted {
                entity: Entity::Agent(a),
            } => assert_eq!(a.worktree_id.to_string(), "feat"),
            other => panic!("expected agent upsert, got {other:?}"),
        }

        // Moving to the worktree it already lives in is a silent no-op.
        daemon
            .move_agent(&AgentId("a1".into()), &WorktreeId("feat".into()))
            .unwrap();
        assert!(rx.try_recv().is_err(), "no broadcast for a no-op move");
    }

    #[tokio::test]
    async fn enter_worktree_takes_an_existing_branch_and_moves_the_row_now() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_worktree(&daemon, "p", "feat", "/berth-test/p-feat", false);
        seed_agent(&daemon, "a1", "root", Some("s1"));
        let a1 = AgentId("a1".into());
        let mut rx = daemon.events.subscribe();

        let (target, outcome, _) = daemon.enter_worktree(&a1, "feat", None).await.unwrap();
        assert_eq!(target.id.to_string(), "feat");
        // No PTY runs here, so nothing waits on a turn end.
        assert_eq!(outcome, EnterOutcome::NextLaunch);
        assert!(!daemon.relocation_pending(&a1));
        assert_eq!(agent_worktree(&daemon, "a1"), "feat");
        match rx.try_recv().unwrap() {
            ServerEvent::EntityUpserted {
                entity: Entity::Agent(a),
            } => assert_eq!(a.worktree_id.to_string(), "feat"),
            other => panic!("expected agent upsert, got {other:?}"),
        }

        // Already there: a settled answer, no broadcast.
        let (again, outcome, _) = daemon.enter_worktree(&a1, "feat", None).await.unwrap();
        assert_eq!(again.id, target.id);
        assert_eq!(outcome, EnterOutcome::AlreadyThere);
        assert!(rx.try_recv().is_err(), "no broadcast for a no-op enter");

        // Blank names are refused before anything is touched.
        assert!(daemon.enter_worktree(&a1, "  ", None).await.is_err());
    }

    /// Between `berth worktree` and the turn's Stop the row already sits
    /// under the target while the process still reports the old checkout:
    /// that cwd must not drag it back, and only a turn-end hook drains the
    /// pending relocation.
    #[test]
    fn pending_relocation_ignores_the_old_cwd_until_the_turn_ends() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_worktree(&daemon, "p", "feat", "/berth-test/p-feat", false);
        seed_agent(&daemon, "a1", "feat", Some("s1"));
        let a1 = AgentId("a1".into());
        let feat = daemon
            .store
            .get_worktree(&WorktreeId("feat".into()))
            .unwrap()
            .unwrap();
        daemon
            .pending_moves
            .lock()
            .unwrap()
            .insert(a1.clone(), feat);

        daemon.reparent_agent_by_cwd(&a1, "/berth-test/p", Some("s1"), false);
        assert_eq!(
            agent_worktree(&daemon, "a1"),
            "feat",
            "the old checkout's cwd is ignored mid-relocation"
        );

        daemon.complete_pending_move(
            &a1,
            &HookEvent::PostToolUse {
                tool_name: Some("Bash".into()),
                subagent_id: None,
                detail: None,
            },
        );
        assert!(
            daemon.relocation_pending(&a1),
            "a tool hook is not a turn end"
        );
        daemon.complete_pending_move(&a1, &HookEvent::Stop);
        assert!(!daemon.relocation_pending(&a1));

        // Drained, the reparent is live again.
        daemon.reparent_agent_by_cwd(&a1, "/berth-test/p", Some("s1"), false);
        assert_eq!(agent_worktree(&daemon, "a1"), "root");
    }

    #[tokio::test]
    async fn enter_worktree_creates_the_checkout_in_berths_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);
        let daemon = test_daemon();
        daemon
            .store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: ProjectId("p".into()),
                name: "p".into(),
                repo_path: repo.clone(),
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        seed_worktree(&daemon, "p", "root", &repo.to_string_lossy(), true);
        seed_agent(&daemon, "a1", "root", Some("s1"));
        let a1 = AgentId("a1".into());
        let mut rx = daemon.events.subscribe();

        let (target, _, _) = daemon.enter_worktree(&a1, "feat", None).await.unwrap();
        assert_eq!(target.branch, "feat");
        assert_eq!(target.path, root.join("repo-worktrees").join("feat"));
        assert!(target.path.join(".git").exists(), "a real checkout");
        assert_eq!(agent_worktree(&daemon, "a1"), target.id.to_string());
        // The worktree's upsert lands first, then the agent's.
        assert!(matches!(
            rx.try_recv().unwrap(),
            ServerEvent::EntityUpserted { entity: Entity::Worktree(w) } if w.id == target.id
        ));
        assert!(matches!(
            rx.try_recv().unwrap(),
            ServerEvent::EntityUpserted { entity: Entity::Agent(a) } if a.worktree_id == target.id
        ));
    }

    fn seed_pending_agent(daemon: &Daemon, id: &str, worktree: &str) {
        daemon
            .store
            .insert_agent_with_auto_title(
                &Agent {
                    id: AgentId(id.into()),
                    worktree_id: WorktreeId(worktree.into()),
                    name: format!("{id}-default"),
                    status: AgentStatus::Fresh,
                    archived: false,
                    archived_at: 0,
                    ended_at: 0,
                    unseen: false,
                    kind: AgentKind::Claude,
                    model: None,
                    effort: None,
                    session_id: None,
                    sort_order: 0,
                    status_changed_at: 0,
                    alive: false,
                    suggested_prompt: None,
                    provider_error: None,
                    routed_from: None,
                    route_suggestion: None,
                    acp_provider: None,
                    backend: Backend::Terminal,
                },
                true,
            )
            .unwrap();
    }

    #[test]
    fn auto_rename_applies_once_and_defers_to_user_titles() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_pending_agent(&daemon, "a1", "root");
        let mut rx = daemon.events.subscribe();

        // First agent attempt lands, sanitized, and is broadcast.
        daemon
            .auto_rename_agent(&AgentId("a1".into()), "  Fix   Login\tRedirect  ")
            .unwrap();
        match rx.try_recv().unwrap() {
            ServerEvent::EntityUpserted {
                entity: Entity::Agent(a),
            } => assert_eq!(a.name, "Fix Login Redirect"),
            other => panic!("expected agent upsert, got {other:?}"),
        }

        // A second attempt is declined with a settled, informative error.
        let err = daemon
            .auto_rename_agent(&AgentId("a1".into()), "Another Title")
            .unwrap_err();
        assert!(err.to_string().contains("already has a title"), "{err}");
        assert_eq!(
            daemon
                .store
                .get_agent(&AgentId("a1".into()))
                .unwrap()
                .unwrap()
                .name,
            "Fix Login Redirect"
        );

        // A user rename beats a pending auto-title: the CLI's later attempt
        // must not clobber it.
        seed_pending_agent(&daemon, "a2", "root");
        daemon
            .rename_agent(&AgentId("a2".into()), "my session")
            .unwrap();
        let err = daemon
            .auto_rename_agent(&AgentId("a2".into()), "Model Title")
            .unwrap_err();
        assert!(err.to_string().contains("already has a title"), "{err}");

        // Garbage titles are rejected outright.
        assert!(daemon
            .auto_rename_agent(&AgentId("a1".into()), " \u{7}\n ")
            .is_err());
        // Unknown agents report cleanly.
        let err = daemon
            .auto_rename_agent(&AgentId("ghost".into()), "Some Title")
            .unwrap_err();
        assert!(err.to_string().contains("agent not found"), "{err}");
    }

    #[test]
    fn sanitize_title_collapses_and_caps() {
        assert_eq!(
            sanitize_title(" Fix   Login\u{7}Redirect \n"),
            "Fix Login Redirect"
        );
        assert_eq!(sanitize_title("\u{1b}[31m"), "[31m");
        assert_eq!(sanitize_title("   "), "");
        let long = "word ".repeat(30);
        assert!(sanitize_title(&long).chars().count() <= 60);
        assert!(!sanitize_title(&long).ends_with(' '));
    }

    #[test]
    fn move_agent_rejects_cross_project_targets() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p", "q"]);
        seed_worktree(&daemon, "p", "p-root", "/berth-test/p", true);
        seed_worktree(&daemon, "q", "q-root", "/berth-test/q", true);
        seed_agent(&daemon, "a1", "p-root", None);

        let err = daemon
            .move_agent(&AgentId("a1".into()), &WorktreeId("q-root".into()))
            .unwrap_err();
        assert!(err.to_string().contains("different project"));
        assert_eq!(agent_worktree(&daemon, "a1"), "p-root");
    }

    #[test]
    fn reparent_by_cwd_picks_deepest_matching_worktree() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        // Nested layout: the linked checkout lives under the repo root, so
        // both paths are prefixes of a cwd inside it — deepest must win.
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_worktree(&daemon, "p", "feat", "/berth-test/p/.wt/feat", false);
        seed_agent(&daemon, "a1", "root", None);

        // cwd inside the root checkout (but outside the nested worktree)
        // keeps the agent where it is.
        daemon.reparent_agent_by_cwd(&AgentId("a1".into()), "/berth-test/p/src", None, false);
        assert_eq!(agent_worktree(&daemon, "a1"), "root");

        // cwd inside the nested worktree re-homes it there.
        daemon.reparent_agent_by_cwd(
            &AgentId("a1".into()),
            "/berth-test/p/.wt/feat/src",
            None,
            false,
        );
        assert_eq!(agent_worktree(&daemon, "a1"), "feat");

        // cwd outside every worktree is ignored.
        daemon.reparent_agent_by_cwd(&AgentId("a1".into()), "/elsewhere", None, false);
        assert_eq!(agent_worktree(&daemon, "a1"), "feat");
    }

    /// Regression: a session that creates a worktree and steps into it
    /// reports the new cwd *before* the sync has adopted a row for it (the
    /// `Stop` hook fires long before the next 2s sync tick). The cwd must be
    /// remembered and replayed on adoption, or the row sits under the old
    /// checkout until the user's next prompt.
    #[tokio::test]
    async fn worktree_sync_replays_a_cwd_reported_before_adoption() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);

        let daemon = test_daemon();
        let project = Project {
            workspace_id: Default::default(),
            id: ProjectId("p".into()),
            name: "p".into(),
            repo_path: repo.clone(),
            sort_order: 0,
            pin_order: None,
        };
        daemon.store.insert_project(&project).unwrap();
        seed_worktree(&daemon, "p", "root", &repo.to_string_lossy(), true);
        seed_agent(&daemon, "a1", "root", Some("s1"));

        // The agent creates a sibling worktree and walks into it. The hook
        // lands first: no row exists yet, so nothing moves.
        let feat = root.join("repo-worktrees").join("feat");
        git_in(
            &repo,
            &["worktree", "add", &feat.to_string_lossy(), "-b", "feat"],
        );
        daemon.reparent_agent_by_cwd(
            &AgentId("a1".into()),
            &feat.to_string_lossy(),
            Some("s1"),
            false,
        );
        assert_eq!(agent_worktree(&daemon, "a1"), "root");

        // The sync adopts the checkout and replays the remembered cwd.
        daemon.sync_project_worktrees(&project).await.unwrap();
        let (_, worktrees, _, _) = daemon.store.load_tree().unwrap();
        let adopted = worktrees
            .iter()
            .find(|w| w.branch == "feat")
            .expect("feat worktree adopted");
        assert_eq!(agent_worktree(&daemon, "a1"), adopted.id.to_string());

        // A deliberate move back must survive the next adoption: the move
        // drops the remembered cwd, so replaying it can't overrule the user.
        daemon
            .move_agent(&AgentId("a1".into()), &WorktreeId("root".into()))
            .unwrap();
        let other = root.join("repo-worktrees").join("other");
        git_in(
            &repo,
            &["worktree", "add", &other.to_string_lossy(), "-b", "other"],
        );
        daemon.sync_project_worktrees(&project).await.unwrap();
        assert_eq!(agent_worktree(&daemon, "a1"), "root");
    }

    /// The replay is scoped to the synced project and skips archived rows.
    #[test]
    fn cwd_replay_skips_other_projects_and_archived_agents() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p", "q"]);
        seed_worktree(&daemon, "p", "p-root", "/berth-test/p", true);
        seed_worktree(&daemon, "q", "q-root", "/berth-test/q", true);
        seed_worktree(&daemon, "q", "q-feat", "/berth-test/q-feat", false);
        seed_agent(&daemon, "a1", "q-root", None);
        seed_agent(&daemon, "a2", "q-root", None);

        // Both agents report a cwd inside q-feat before it exists...
        daemon
            .store
            .delete_worktree(&WorktreeId("q-feat".into()))
            .unwrap();
        daemon.reparent_agent_by_cwd(&AgentId("a1".into()), "/berth-test/q-feat", None, false);
        daemon.reparent_agent_by_cwd(&AgentId("a2".into()), "/berth-test/q-feat", None, false);
        seed_worktree(&daemon, "q", "q-feat", "/berth-test/q-feat", false);

        // ...but a replay for project p touches neither.
        let p = daemon
            .store
            .get_project(&ProjectId("p".into()))
            .unwrap()
            .unwrap();
        daemon.reparent_agents_by_last_cwd(&p);
        assert_eq!(agent_worktree(&daemon, "a1"), "q-root");

        // Archived agents stay put; live ones re-home.
        daemon
            .store
            .set_agent_archived(&AgentId("a2".into()), true)
            .unwrap();
        let q = daemon
            .store
            .get_project(&ProjectId("q".into()))
            .unwrap()
            .unwrap();
        daemon.reparent_agents_by_last_cwd(&q);
        assert_eq!(agent_worktree(&daemon, "a1"), "q-feat");
        assert_eq!(agent_worktree(&daemon, "a2"), "q-root");
    }

    /// Renaming a project relabels its row and nothing else: the checkout on
    /// disk keeps its name, `repo_path` keeps pointing at it, and an empty
    /// name puts the row back on the folder's own name — the only way back
    /// once a project has been renamed.
    #[tokio::test]
    async fn rename_project_relabels_the_row_and_leaves_the_folder_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("acme-api");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);

        let daemon = test_daemon();
        let id = match daemon.add_project(&repo, None, false, None).await.unwrap() {
            EntityId::Project(id) => id,
            other => panic!("expected a project id, got {other:?}"),
        };
        let named = |daemon: &Arc<Daemon>| daemon.store.get_project(&id).unwrap().unwrap();
        assert_eq!(named(&daemon).name, "acme-api", "named after the folder");

        daemon.rename_project(&id, "  Acme API  ").unwrap();
        let project = named(&daemon);
        assert_eq!(project.name, "Acme API", "trimmed and stored");
        assert_eq!(project.repo_path, repo, "the folder is untouched");
        assert!(repo.exists(), "and still on disk under its own name");
        assert_eq!(
            project.folder_subtitle().as_deref(),
            Some("acme-api"),
            "a renamed row still shows where it lives"
        );

        daemon.rename_project(&id, "   ").unwrap();
        let project = named(&daemon);
        assert_eq!(project.name, "acme-api", "empty resets to the folder name");
        assert_eq!(project.folder_subtitle(), None, "nothing left to show");
    }

    /// `git rev-parse --show-toplevel` answers with the checkout it ran in, so
    /// `berth add .` from inside a linked worktree used to make the worktree
    /// the project: named after the branch directory, `repo_path` pointing at
    /// it, and a ⌂ root row for a directory the project did not own. The repo
    /// is the project no matter which of its checkouts you add it from.
    #[tokio::test]
    async fn add_project_from_inside_a_worktree_roots_at_the_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);
        let feat = root.join("repo-worktrees").join("gentle-narwhal-files");
        git_in(
            &repo,
            &[
                "worktree",
                "add",
                &feat.to_string_lossy(),
                "-b",
                "gentle-narwhal-files",
            ],
        );

        let daemon = test_daemon();
        daemon.add_project(&feat, None, false, None).await.unwrap();

        let (projects, worktrees, _, _) = daemon.store.load_tree().unwrap();
        let project = projects.first().expect("project added");
        assert_eq!(project.repo_path, repo, "project is rooted at the repo");
        assert_eq!(project.name, "repo", "named after the repo, not the branch");

        let main: Vec<&Worktree> = worktrees.iter().filter(|w| w.is_main).collect();
        assert_eq!(main.len(), 1, "exactly one root row: {worktrees:#?}");
        assert_eq!(
            main[0].path, repo,
            "the ⌂ root row is the project's own dir"
        );
        assert_eq!(main[0].branch, "main");
        let linked = worktrees
            .iter()
            .find(|w| !w.is_main)
            .expect("the worktree we added from is a plain row");
        assert_eq!(linked.path, feat);
        assert_eq!(linked.branch, "gentle-narwhal-files");
    }

    /// Adding the repo from a worktree of one already in the workspace is the
    /// same repo, so it collides instead of arriving as a second project.
    #[tokio::test]
    async fn adding_a_worktree_of_a_known_repo_is_a_duplicate() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);
        let feat = root.join("repo-worktrees").join("feat");
        git_in(
            &repo,
            &["worktree", "add", &feat.to_string_lossy(), "-b", "feat"],
        );

        let daemon = test_daemon();
        daemon.add_project(&repo, None, false, None).await.unwrap();
        let err = daemon
            .add_project(&feat, None, false, None)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("already added"),
            "expected a duplicate error, got: {err}"
        );
    }

    /// Root-ness is derived from git's checkout list on every pass, not frozen
    /// at insert time: a project whose rows were seeded before the root was
    /// known (or seeded wrong) has its ⌂ root row repaired in place, and the
    /// stale one loses the reprieve that kept it undeletable.
    #[tokio::test]
    async fn reconcile_moves_root_ness_onto_the_checkout_git_lists_first() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);
        let feat = root.join("repo-worktrees").join("feat");
        git_in(
            &repo,
            &["worktree", "add", &feat.to_string_lossy(), "-b", "feat"],
        );

        let daemon = test_daemon();
        let project = Project {
            workspace_id: Default::default(),
            id: ProjectId("p".into()),
            name: "p".into(),
            repo_path: repo.clone(),
            sort_order: 0,
            pin_order: None,
        };
        daemon.store.insert_project(&project).unwrap();
        // The wrong way round: the linked checkout wears the root badge and
        // the repo's own checkout is a plain row.
        seed_worktree(&daemon, "p", "wt", &feat.to_string_lossy(), true);
        seed_worktree(&daemon, "p", "rt", &repo.to_string_lossy(), false);

        daemon.sync_project_worktrees(&project).await.unwrap();

        let (_, worktrees, _, _) = daemon.store.load_tree().unwrap();
        let by = |id: &str| worktrees.iter().find(|w| w.id.as_str() == id).unwrap();
        assert!(by("rt").is_main, "the repo's checkout is the root row");
        assert!(!by("wt").is_main, "the linked checkout gave the badge back");
        assert_eq!(by("rt").branch, "main");
        assert_eq!(by("wt").branch, "feat");
    }

    /// A row still carrying a stale `is_main` no longer survives its checkout
    /// going away — the real root is always in git's list, so anything missing
    /// from it is a linked checkout, whatever flag it happens to hold.
    #[tokio::test]
    async fn reconcile_drops_a_vanished_row_that_still_claims_to_be_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);

        let daemon = test_daemon();
        let project = Project {
            workspace_id: Default::default(),
            id: ProjectId("p".into()),
            name: "p".into(),
            repo_path: repo.clone(),
            sort_order: 0,
            pin_order: None,
        };
        daemon.store.insert_project(&project).unwrap();
        seed_worktree(&daemon, "p", "rt", &repo.to_string_lossy(), true);
        seed_worktree(
            &daemon,
            "p",
            "ghost",
            &root.join("repo-worktrees").join("gone").to_string_lossy(),
            true,
        );

        daemon.sync_project_worktrees(&project).await.unwrap();

        let (_, worktrees, _, _) = daemon.store.load_tree().unwrap();
        assert!(
            worktrees.iter().all(|w| w.id.as_str() != "ghost"),
            "the ghost row is gone: {worktrees:#?}"
        );
        let rt = worktrees.iter().find(|w| w.id.as_str() == "rt").unwrap();
        assert!(rt.is_main, "the surviving root row keeps the badge");
    }

    fn git_in(repo: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn reparent_by_cwd_ignores_foreign_sessions_unless_capturing() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_worktree(&daemon, "p", "feat", "/berth-test/p-feat", false);
        seed_agent(&daemon, "a1", "root", Some("s1"));

        // A different session id on a non-capturing event (a nested claude
        // launched inside the agent's PTY) must not move the row.
        daemon.reparent_agent_by_cwd(
            &AgentId("a1".into()),
            "/berth-test/p-feat",
            Some("s2"),
            false,
        );
        assert_eq!(agent_worktree(&daemon, "a1"), "root");

        // A capturing event (re)establishes ownership, so it may move it.
        daemon.reparent_agent_by_cwd(
            &AgentId("a1".into()),
            "/berth-test/p-feat",
            Some("s2"),
            true,
        );
        assert_eq!(agent_worktree(&daemon, "a1"), "feat");
    }

    #[test]
    fn cli_missing_message_names_the_binary_not_the_kind() {
        assert!(cli_missing_message(AgentKind::Claude).starts_with("claude was not found"));
        assert!(cli_missing_message(AgentKind::Codex).starts_with("codex was not found"));
        // No "restart berth": agent CLIs are spawned through the user's
        // login shell, so a fresh install is picked up on the next try.
        for kind in AgentKind::NATIVE {
            let msg = cli_missing_message(kind);
            assert!(msg.contains("try again"), "{msg}");
            assert!(!msg.contains("restart"), "{msg}");
        }
    }

    /// The three-way wiring behind a killed subagent: the machine reports
    /// which ids a hold waits on, `subagents` reads the kill off disk, and
    /// the tick drops it. Fails if `held_subagents` stops reporting a hold,
    /// if the sweep moves after the tick instead of before it, or if the
    /// meta path stops matching the `transcript_path` the hook reported.
    #[test]
    fn a_killed_subagent_drops_out_of_a_held_stop() {
        let daemon = test_daemon();
        let id = AgentId("a1".into());
        daemon.status_machines.lock().unwrap().insert(
            id.clone(),
            AgentStatusMachine::new(AgentStatus::Fresh, Some("s1".into())),
        );

        let tmp = tempfile::tempdir().unwrap();
        let transcript = tmp.path().join("s1.jsonl");
        let dir = transcript.with_extension("").join("subagents");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("agent-dead.meta.json"),
            r#"{"stoppedByUser":true}"#,
        )
        .unwrap();
        daemon.note_transcript_path(&id, transcript.to_str().unwrap());

        let sid = Some("s1".to_string());
        daemon.apply_hook_event(&id, HookEvent::UserPromptSubmit, sid.clone());
        daemon.apply_hook_event(
            &id,
            HookEvent::SubagentStart {
                subagent_id: Some("dead".into()),
            },
            sid.clone(),
        );
        daemon.apply_hook_event(&id, HookEvent::Stop, sid);

        // The Stop is gated: as far as the hooks went, the subagent is
        // still working.
        {
            let machines = daemon.status_machines.lock().unwrap();
            let machine = machines.get(&id).unwrap();
            assert_eq!(machine.status(), AgentStatus::Running);
            assert_eq!(machine.held_subagents(), Some(vec!["dead".to_string()]));
        }

        daemon.tick_status_machines();

        // The kill is off the books, so the hold now drains on DRAIN_GRACE
        // instead of waiting out SUBAGENT_QUIET_GRACE.
        assert_eq!(
            daemon
                .status_machines
                .lock()
                .unwrap()
                .get(&id)
                .unwrap()
                .held_subagents(),
            None
        );
    }

    #[test]
    fn reparent_by_cwd_skips_archived_agents() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_worktree(&daemon, "p", "feat", "/berth-test/p-feat", false);
        seed_agent(&daemon, "a1", "root", None);
        daemon
            .store
            .set_agent_archived(&AgentId("a1".into()), true)
            .unwrap();

        daemon.reparent_agent_by_cwd(&AgentId("a1".into()), "/berth-test/p-feat", None, false);
        assert_eq!(agent_worktree(&daemon, "a1"), "root");
    }

    // ---- workspaces ----

    #[test]
    fn workspace_lifecycle_add_open_rename_delete() {
        let daemon = test_daemon();
        let EntityId::Workspace(id) = daemon.add_workspace(" client ").unwrap() else {
            panic!("add returns the workspace id");
        };
        // Name is trimmed; duplicates (trimmed) and blanks are refused.
        assert_eq!(
            daemon.store.get_workspace(&id).unwrap().unwrap().name,
            "client"
        );
        assert!(daemon.add_workspace("client").is_err());
        assert!(daemon.add_workspace("   ").is_err());

        // Adding never opens; opening one moves the remembered default
        // (and re-opening is a quiet no-op).
        assert_eq!(
            daemon.store.active_workspace_id().unwrap().as_str(),
            "default"
        );
        daemon.set_default_workspace(&id).unwrap();
        assert_eq!(daemon.store.active_workspace_id().unwrap(), id);
        daemon.set_default_workspace(&id).unwrap();
        assert!(daemon
            .set_default_workspace(&WorkspaceId("ghost".into()))
            .is_err());

        // Rename keeps names unique (a rename to itself is fine).
        daemon.rename_workspace(&id, "acme").unwrap();
        daemon.rename_workspace(&id, "acme").unwrap();
        assert!(daemon.rename_workspace(&id, "default").is_err());

        // Deleting the default workspace moves the default to a survivor.
        daemon.remove_workspace(&id).unwrap();
        assert_eq!(
            daemon.store.active_workspace_id().unwrap().as_str(),
            "default"
        );
        assert!(daemon.store.get_workspace(&id).unwrap().is_none());

        // The last workspace can't go.
        assert!(daemon
            .remove_workspace(&WorkspaceId("default".into()))
            .is_err());
    }

    #[test]
    fn workspace_with_projects_refuses_deletion() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]); // lands in 'default'
        let EntityId::Workspace(empty) = daemon.add_workspace("empty").unwrap() else {
            panic!("add returns the workspace id");
        };
        let err = daemon
            .remove_workspace(&WorkspaceId("default".into()))
            .unwrap_err();
        assert!(
            err.to_string().contains("1 project"),
            "helpful refusal: {err}"
        );
        // An empty, closed workspace deletes cleanly.
        daemon.remove_workspace(&empty).unwrap();
    }

    /// The status broadcast carries the flag it persisted: a live turn
    /// landing on finished says `unseen`, the next prompt says not.
    #[test]
    fn status_broadcast_carries_the_unseen_flag() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_agent(&daemon, "a1", "root", None); // running
        let id = AgentId("a1".into());
        let mut rx = daemon.events.subscribe();

        let mut step = |to: AgentStatus| {
            daemon.apply_status_effects(&id, vec![Effect::SetStatus(to)]);
            match rx.try_recv().unwrap() {
                ServerEvent::StatusChanged { unseen, raised, .. } => (unseen, raised),
                other => panic!("expected a status change, got {other:?}"),
            }
        };
        let waiting = Some(berth_core::Attention::NeedsFeedback);
        assert_eq!(step(AgentStatus::NeedsFeedback), (false, waiting));
        let unread = Some(berth_core::Attention::Unseen);
        assert_eq!(step(AgentStatus::Finished), (true, unread), "red → green");
        assert_eq!(step(AgentStatus::Running), (false, None), "a new turn");
    }

    #[test]
    fn saved_session_id_is_broadcast_with_the_row() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_agent(&daemon, "a1", "root", None);
        let id = AgentId("a1".into());
        let mut rx = daemon.events.subscribe();

        daemon.apply_status_effects(&id, vec![Effect::SaveSessionId("s9".into())]);
        match rx.try_recv().unwrap() {
            ServerEvent::EntityUpserted {
                entity: Entity::Agent(a),
            } => assert_eq!(a.session_id.as_deref(), Some("s9")),
            other => panic!("expected agent upsert, got {other:?}"),
        }
    }

    /// `mark_agent_seen` clears the flag and hands every subscriber the row
    /// — once. Marking a row already read sends nothing.
    #[test]
    fn mark_agent_seen_broadcasts_only_a_flip() {
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "root", "/berth-test/p", true);
        seed_agent(&daemon, "a1", "root", None);
        let id = AgentId("a1".into());
        daemon
            .store
            .set_agent_status(&id, AgentStatus::Finished)
            .unwrap();
        let mut rx = daemon.events.subscribe();

        daemon.mark_agent_seen(&id).unwrap();
        match rx.try_recv().unwrap() {
            ServerEvent::EntityUpserted {
                entity: Entity::Agent(a),
            } => assert!(!a.unseen),
            other => panic!("expected agent upsert, got {other:?}"),
        }
        daemon.mark_agent_seen(&id).unwrap();
        assert!(rx.try_recv().is_err(), "nothing to say twice");
    }
}
