use crate::acp::{
    AcpItem, AcpItemAt, AcpSessionState, AttachmentUpload, FeedbackAnswer, QuestionForm, QueueId,
};
use crate::agent::AgentKind;
use crate::attention::Attention;
use crate::entities::{
    Agent, AgentStatus, Entity, EntityId, OrphanedSession, Project, TerminalTab, Workspace,
    Worktree,
};
use crate::ids::{AgentId, ProjectId, TerminalId, WorkspaceId, WorktreeId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Bump on any breaking change to these enums. The daemon refuses mismatched
/// clients; the client then offers a kill-and-restart of the old daemon.
pub const PROTOCOL_VERSION: u32 = 68;

/// Max IPC frame size (length prefix sanity bound).
pub const MAX_FRAME_LEN: u32 = 4 * 1024 * 1024;

pub const MAX_STARTING_PROMPT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionRef {
    Agent(AgentId),
    Terminal(TerminalId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnWorktree {
    pub branch: String,
    pub base: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientRequest {
    Hello {
        protocol_version: u32,
    },
    /// Reply is one Snapshot, then deltas stream on this connection forever.
    Subscribe,

    // -- PTY plane --
    Attach {
        session: SessionRef,
        /// Resume point for gap-free re-attach; None = replay whole ring.
        from_seq: Option<u64>,
        cols: u16,
        rows: u16,
    },
    Detach {
        session: SessionRef,
    },
    Input {
        session: SessionRef,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    Resize {
        session: SessionRef,
        cols: u16,
        rows: u16,
    },

    // -- entity CRUD (RPC-style; answered by Ack/Error with matching req_id) --
    /// Create a workspace. Does not open it — that stays a separate step.
    AddWorkspace {
        req_id: u64,
        name: String,
    },
    /// Delete a workspace. Refused while it still holds projects, or when it
    /// is the last workspace. Clients still scoped to it fall back to a
    /// surviving one when its EntityRemoved lands.
    RemoveWorkspace {
        req_id: u64,
        id: WorkspaceId,
    },
    RenameWorkspace {
        req_id: u64,
        id: WorkspaceId,
        name: String,
    },
    /// Scope THIS connection to `id`, and remember it as the workspace a
    /// fresh client opens into. Deliberately not broadcast: two berth
    /// instances are two views, and switching in one must not drag the
    /// other along with it.
    OpenWorkspace {
        req_id: u64,
        id: WorkspaceId,
    },
    /// Add a project to the workspace this connection is scoped to (see
    /// OpenWorkspace) — the remembered default for a connection that never
    /// scoped itself, which is every one-shot `berth add`.
    AddProject {
        req_id: u64,
        path: PathBuf,
        name: Option<String>,
        /// Create `path` (and `git init` it, per config) when it doesn't
        /// exist on disk. Set only after the user confirmed in the client.
        create_missing: bool,
    },
    RemoveProject {
        req_id: u64,
        id: ProjectId,
    },
    /// Retitle a project's row. Purely cosmetic: `repo_path` — the folder on
    /// disk — is never touched. An empty name resets the row to the folder's
    /// own name, which is the only way back from a rename.
    RenameProject {
        req_id: u64,
        id: ProjectId,
        name: String,
    },
    /// The full ordered list of pinned projects, highest priority first.
    /// Every project not in `order` is unpinned, so one request pins,
    /// unpins and reorders. Ids the daemon doesn't know are ignored.
    SetProjectPins {
        req_id: u64,
        order: Vec<ProjectId>,
    },
    CreateWorktree {
        req_id: u64,
        project: ProjectId,
        branch: String,
        base: Option<String>,
    },
    DeleteWorktree {
        req_id: u64,
        id: WorktreeId,
        force: bool,
    },
    /// The local branches of `project`, most recently committed first, each
    /// with the worktree row that has it checked out. Answered by one
    /// `Branches`, not by deltas: the list is read when a client opens a
    /// branch picker. The daemon reconciles the project's worktree rows with
    /// `git worktree list` first, so a named worktree is a row the client
    /// already holds.
    ListBranches {
        req_id: u64,
        project: ProjectId,
    },
    /// The resumable conversations of `project`: every ORPHANED SESSION
    /// Answered by one
    /// `OrphanedSessions`, not by deltas — the list is read when the user
    /// opens it and is half derived from the agent CLI's own transcript
    /// store, which berth does not watch.
    ListOrphanedSessions {
        req_id: u64,
        project: ProjectId,
        include_live: bool,
    },
    /// Bring an ORPHANED SESSION back as a live AGENT in `worktree`. Creates
    /// an ordinary row there carrying the old CLI session id, so the next
    /// spawn resumes the conversation. Answered by an Ack naming the row.
    ResumeOrphanedSession {
        req_id: u64,
        session_id: String,
        worktree: WorktreeId,
    },
    ResumeConversation {
        req_id: u64,
        worktree: WorktreeId,
        kind: AgentKind,
        session_id: String,
        name: String,
    },
    CreateAgent {
        req_id: u64,
        worktree: WorktreeId,
        name: String,
        kind: AgentKind,
        /// Model the CLI launches with; None = the CLI's own default.
        model: Option<String>,
        /// Reasoning effort the CLI launches with; None = the CLI's own default.
        effort: Option<String>,
        /// True when the user accepted the generated default name, marking
        /// the session eligible for one agent-driven auto-title (the CLI
        /// runs `berth rename` on its first prompt).
        auto_title: bool,
        /// The first turn handed to the CLI as its positional prompt
        /// (`claude "<text>"`, `codex "<text>"`) —
        /// what an AGENT PRESET launch composes from its prefix, the task
        /// and its postfix. Request-only: never
        /// persisted, so a RESUME can never replay it. Skips PREWARM POOL
        /// adoption, since a spare booted bare cannot be handed one.
        #[serde(default)]
        starting_prompt: Option<String>,
    },
    /// Fire-and-forget: pre-spawn an agent CLI for this (worktree, kind) so
    /// the next CreateAgent adopts an already-booted session.
    /// No reply;
    /// a missing CLI or failed spawn silently degrades to a cold spawn.
    PrewarmAgent {
        worktree: WorktreeId,
        kind: AgentKind,
        /// Must match the CreateAgent that follows or the warm session is
        /// discarded (a CLI booted with the wrong model can't be adopted).
        model: Option<String>,
        effort: Option<String>,
    },
    RenameAgent {
        req_id: u64,
        id: AgentId,
        name: String,
    },
    /// Agent-initiated one-shot title (`berth rename` inside the session's
    /// CLI). Applies only while the session still awaits its auto-title;
    /// answered with Error (informational, not a fault) once a title —
    /// user- or agent-set — already sticks, so a user rename is never
    /// clobbered by a late or repeated agent attempt.
    AutoRenameAgent {
        req_id: u64,
        id: AgentId,
        name: String,
    },
    /// Re-home the agent row under another worktree of the same project,
    /// right now. A live PTY is killed and respawned (resumed) in the new
    /// path — left running, its hooks would keep reporting the old
    /// checkout's cwd and the daemon would re-home the row right back.
    /// The daemon-side primitive behind `EnterWorktree`; no TUI verb sends
    /// it any more.
    MoveAgent {
        req_id: u64,
        id: AgentId,
        worktree: WorktreeId,
    },
    /// `berth worktree <name>`, run by the agent from inside its own
    /// session: create the worktree `branch` under the agent's project (or
    /// take the existing one with that branch), re-home the agent row under
    /// it at once, and relocate the live session into it when its current
    /// turn ends — killed and respawned resumed there, with a prompt that
    /// tells the CLI where it now is. Replies with `WorktreeEntered`.
    EnterWorktree {
        req_id: u64,
        id: AgentId,
        branch: String,
        base: Option<String>,
    },
    /// `berth enter <dir>`, run by the agent from inside its own session:
    /// the same relocation as `EnterWorktree`, into the checkout at `path`
    /// of any project — added to the agent's workspace first when it is not
    /// there yet. Replies with `WorktreeEntered`.
    EnterProject {
        req_id: u64,
        id: AgentId,
        path: PathBuf,
    },
    /// `berth spawn "<task>"`, run by the agent from inside its own
    /// session: start a new AGENT beside it — the same
    /// AGENT KIND / MODEL / EFFORT unless `kind` names another harness —
    /// with `starting_prompt` as the new CLI's first prompt, so it begins
    /// the task at once. The caller's own process is untouched. Answered
    /// with `Ack { created: Some(EntityId::Agent(..)) }`; the row reaches
    /// every TUI as an ordinary `EntityUpserted`.
    SpawnSiblingAgent {
        req_id: u64,
        id: AgentId,
        kind: Option<AgentKind>,
        starting_prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktree: Option<SpawnWorktree>,
    },
    EndSelf {
        req_id: u64,
        id: AgentId,
    },
    EndAgent {
        req_id: u64,
        id: AgentId,
    },
    /// Kills the PTY, sets archived=1.
    ArchiveAgent {
        req_id: u64,
        id: AgentId,
    },
    UnarchiveAgent {
        req_id: u64,
        id: AgentId,
    },
    StopAgent {
        req_id: u64,
        id: AgentId,
    },
    /// Archive an EXTERNAL SESSION — a conversation berth did not start,
    /// found on disk by a client. Only the mark is stored: the transcript
    /// and any process running it are left alone. Re-archiving moves
    /// `archived_at` forward. Broadcast as `ExternalSessionArchived`.
    ArchiveExternalSession {
        req_id: u64,
        kind: AgentKind,
        session_id: String,
    },
    /// Drop the mark again. Broadcast as `ExternalSessionUnarchived`, also
    /// when there was none.
    UnarchiveExternalSession {
        req_id: u64,
        kind: AgentKind,
        session_id: String,
    },
    DeleteAgent {
        req_id: u64,
        id: AgentId,
    },
    /// Respawn; resumes the stored session id (`claude --resume` /
    /// `codex resume`) when one is stored.
    RestartAgent {
        req_id: u64,
        id: AgentId,
    },
    SwitchAgentModel {
        req_id: u64,
        id: AgentId,
        model: Option<String>,
    },
    ContinueAgentIn {
        req_id: u64,
        id: AgentId,
        kind: AgentKind,
        model: Option<String>,
    },
    /// `router.toml` as JSON. Reply `RouterConfig`.
    GetRouterConfig {
        req_id: u64,
    },
    /// Validate, format and write `router.toml` from JSON. Reply
    /// `RouterConfig` with the file as written.
    SaveRouterConfig {
        req_id: u64,
        json: String,
    },
    CreateTerminal {
        req_id: u64,
        worktree: WorktreeId,
        name: Option<String>,
    },
    RenameTerminal {
        req_id: u64,
        id: TerminalId,
        name: String,
    },
    CloseTerminal {
        req_id: u64,
        id: TerminalId,
    },

    /// Fire-and-forget: this agent's session is on screen, so a turn it
    /// finished unwatched (`Agent::unseen`) has now been looked at. The
    /// daemon answers with the agent's upsert when the flag actually flips.
    MarkAgentSeen {
        id: AgentId,
    },

    /// The last reading the daemon's sampler published — the daemon plus
    /// every live session's process subtree, its CPU share and its Claude
    /// token spend. Answered by `ServerEvent::Metrics` with the same req_id
    /// (not an Ack).
    ///
    /// This does not trigger a sweep. CPU is a delta and the token tally is
    /// a file offset, so both are stateful over time and only one owner may
    /// measure them; asking here would date every delta from whichever
    /// client swept last. The answer is therefore up to one sampler tick
    /// old, which `MetricsSnapshot::sampled_at_ms` reports.
    GetMetrics {
        req_id: u64,
    },

    GetUsageLimits {
        req_id: u64,
    },

    GetTaskNote {
        req_id: u64,
        worktree: WorktreeId,
    },

    SetTaskNote {
        req_id: u64,
        worktree: WorktreeId,
        text: String,
    },

    GetProjectSetup {
        req_id: u64,
        project: ProjectId,
    },

    SetProjectSetup {
        req_id: u64,
        project: ProjectId,
        command: String,
    },

    ListTriggers {
        req_id: u64,
    },

    /// Post a short test text through every configured chat bridge route.
    /// Answered by an Ack when each route took it, else an Error naming the
    /// routes that failed, or saying none is configured.
    TestChatBridge {
        req_id: u64,
    },

    CreateTrigger {
        req_id: u64,
        trigger: NewTrigger,
    },

    DeleteTrigger {
        req_id: u64,
        id: TriggerId,
    },

    ListCheckpoints {
        req_id: u64,
        agent: AgentId,
    },

    RestoreCheckpoint {
        req_id: u64,
        agent: AgentId,
        number: u32,
    },

    ListChannels {
        req_id: u64,
    },

    ReadChannel {
        req_id: u64,
        channel: String,
        after: i64,
        agent: Option<AgentId>,
    },

    PostToChannel {
        req_id: u64,
        channel: String,
        content: String,
        agent: Option<AgentId>,
    },

    GetChannelPresence {
        req_id: u64,
        channel: String,
    },

    OpenChannel {
        req_id: u64,
        key: Option<String>,
        agent: Option<AgentId>,
        #[serde(default)]
        shared: bool,
    },

    LeaveChannel {
        req_id: u64,
        key: String,
    },

    DeleteChannel {
        req_id: u64,
        key: String,
    },

    PostChannelNote {
        req_id: u64,
        channel: String,
        content: String,
        agent: Option<AgentId>,
    },

    GetChannelTouched {
        req_id: u64,
        channel: String,
    },

    PromoteChannelMessage {
        req_id: u64,
        channel: String,
        seq: i64,
        agent: Option<AgentId>,
    },

    SetChannelGrant {
        req_id: u64,
        channel: String,
        agent: AgentId,
        member: String,
        granted: bool,
    },

    ListChannelGrants {
        req_id: u64,
        channel: Option<String>,
    },

    CreateAcpAgent {
        req_id: u64,
        worktree: WorktreeId,
        name: String,
        provider: String,
        auto_title: bool,
    },

    AcpPrompt {
        req_id: u64,
        agent: AgentId,
        text: String,
        #[serde(default)]
        attachments: Vec<AttachmentUpload>,
    },

    AcpEditQueued {
        req_id: u64,
        agent: AgentId,
        id: QueueId,
        text: String,
    },

    AcpRemoveQueued {
        req_id: u64,
        agent: AgentId,
        id: QueueId,
    },

    AcpSendQueuedNow {
        req_id: u64,
        agent: AgentId,
        id: QueueId,
    },

    AcpRewind {
        req_id: u64,
        agent: AgentId,
        seq: u64,
    },

    AcpMcpStatus {
        req_id: u64,
        agent: AgentId,
    },

    SetConfigOption {
        req_id: u64,
        agent: AgentId,
        id: String,
        value: String,
    },

    AcpAttachment {
        req_id: u64,
        blob: String,
    },

    ListWorktreeFiles {
        req_id: u64,
        worktree: WorktreeId,
        query: String,
        limit: u32,
    },

    AcpTranscript {
        req_id: u64,
        agent: AgentId,
        after: u64,
    },

    AnswerFeedback {
        req_id: u64,
        agent: AgentId,
        answer: FeedbackAnswer,
    },

    AnswerInChannel {
        req_id: u64,
        channel: String,
        request: i64,
        answer: String,
        agent: Option<AgentId>,
    },

    Shutdown,

    OfferTranscription,

    VoiceTranscribed {
        request: u64,
        text: Option<String>,
        error: Option<String>,
    },

    AcpSessionState {
        req_id: u64,
        agent: AgentId,
    },

    GetHub {
        req_id: u64,
    },

    ChangeHub {
        req_id: u64,
        change: String,
    },
}

/// The archive mark on an EXTERNAL SESSION, keyed by its agent CLI and that
/// CLI's own session id. `archived_at` is the daemon's epoch ms at the
/// moment of archiving: a client that sees the transcript written after it
/// treats the conversation as active again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivedExternalSession {
    pub kind: AgentKind,
    pub session_id: String,
    pub archived_at: i64,
}

/// Memory usage of one live session: the PTY child plus every descendant
/// (an agent CLI typically fans out into node workers, shells, MCP servers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub session: SessionRef,
    /// OS pid of the PTY child (the subtree's root).
    pub pid: u32,
    /// Resident set size summed over the whole subtree, bytes.
    pub rss_bytes: u64,
    /// Live processes in the subtree, the root included.
    pub procs: u32,
    /// Set when the session is a prewarm-pool spare: an agent CLI the
    /// daemon booted ahead of time for this worktree, waiting for the next
    /// new-agent request there to adopt it. It has no agent row yet, so
    /// this is the only handle a client has for naming and placing it.
    #[serde(default)]
    pub prewarm: Option<PrewarmInfo>,
    /// CPU share of the whole subtree over the interval between the last
    /// two samples, as a percentage of **one core** (`top`'s convention: a
    /// subtree busy on four cores reads up to 400).
    ///
    /// `None` means *not measured* — the sampler's first sweep, or a
    /// previous sample too stale to divide by. Never `0.0` as a stand-in:
    /// "idle" and "unknown" are different facts and must render apart.
    #[serde(default)]
    pub cpu_percent: Option<f32>,
    /// Cumulative Claude token spend of this session's conversation.
    ///
    /// `None` means *not measured*: a Codex or Cursor session (their
    /// transcripts are a different format), one with no CLI session id
    /// yet, or one whose transcript could not be located. `Some` with an
    /// empty `by_model` means measured and nothing spent.
    #[serde(default)]
    pub tokens: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub kind: AgentKind,
    pub window_minutes: u32,
    pub used_percent: f32,
    pub resets_at_ms: i64,
}

/// Tokens one model spent in one conversation. Rates differ per bucket — a
/// cache read is a tenth of an input token, a 1h cache write twice one — so
/// the buckets stay separate all the way to the price table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTokens {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
}

/// One Claude conversation's cumulative spend, split by model.
///
/// `by_model` is a `Vec` and not a map on purpose: `HashMap` iteration
/// order is nondeterministic, so a Sonnet/Opus breakdown would jitter
/// between frames. First-seen order is a fact the daemon has and the client
/// wants, and a conversation runs one to three models, so a map buys
/// nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// One entry per model that took a turn, in first-seen order.
    pub by_model: Vec<ModelTokens>,
}

impl TokenUsage {
    /// Fold `add` into this model's entry, appending it if new.
    pub fn add(&mut self, model: &str, add: &ModelTokens) {
        let slot = match self.by_model.iter_mut().find(|m| m.model == model) {
            Some(slot) => slot,
            None => {
                self.by_model.push(ModelTokens {
                    model: model.to_string(),
                    ..Default::default()
                });
                self.by_model.last_mut().expect("just pushed")
            }
        };
        slot.input += add.input;
        slot.output += add.output;
        slot.cache_read += add.cache_read;
        slot.cache_write_5m += add.cache_write_5m;
        slot.cache_write_1h += add.cache_write_1h;
    }
}

/// Where a prewarm-pool spare is homed and what it booted as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrewarmInfo {
    pub worktree: WorktreeId,
    pub kind: AgentKind,
    pub model: Option<String>,
}

/// Daemon-side half of the metrics modal's data; the client stacks its own
/// RSS on top. Session subtrees are daemon descendants, so `daemon_rss_bytes`
/// counts the daemon process alone — the total stays double-count-free.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub daemon_pid: u32,
    pub daemon_rss_bytes: u64,
    /// Physical memory installed on the machine, bytes; 0 = unknown.
    pub system_total_bytes: u64,
    pub sessions: Vec<SessionMetrics>,
    /// When the sampler took this reading, ms since the unix epoch.
    /// `GetMetrics` answers from a cache, so without this a wedged sampler
    /// is indistinguishable from a quiet machine.
    ///
    /// **Zero means nothing was measured** — see [`Self::unsampled`].
    #[serde(default)]
    pub sampled_at_ms: i64,
}

impl MetricsSnapshot {
    /// The answer to a `GetMetrics` that arrived before the sampler's
    /// first tick.
    ///
    /// The request is answered anyway, and with an unmistakable value,
    /// because the client allocates a pending slot per request and only a
    /// reply frees it: staying silent would leak one slot per poll for as
    /// long as the sampler had nothing to say. A client must test
    /// `sampled_at_ms != 0` before showing any of this — every other field
    /// is zero, which would read as a machine using no memory at all.
    pub fn unsampled() -> Self {
        Self {
            daemon_pid: 0,
            daemon_rss_bytes: 0,
            system_total_bytes: 0,
            sessions: Vec::new(),
            sampled_at_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackKind {
    Permission,
    Question,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub kind: FeedbackKind,
    pub title: String,
    pub detail: Option<String>,
    pub options: Vec<String>,
    #[serde(default)]
    pub always_allow: bool,
    #[serde(default)]
    pub form: Option<QuestionForm>,
}

/// One local branch of a project, as `ListBranches` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// The name under `refs/heads/`, slashes kept (`feat/x`).
    pub name: String,
    /// The project's worktree row that has this branch checked out, the
    /// main checkout included; None when no checkout holds it.
    pub worktree: Option<WorktreeId>,
}

/// What `EnterWorktree` / `EnterProject` did to the agent's live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnterOutcome {
    /// The agent already lived in that worktree; nothing changed.
    AlreadyThere,
    /// The row moved; the live session respawns inside the worktree, resumed,
    /// once its current turn ends.
    Relocating,
    /// The row moved and nothing was running: the next launch lands there.
    NextLaunch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerEvent {
    HelloOk {
        protocol_version: u32,
        daemon_pid: u32,
    },
    Incompatible {
        daemon_protocol_version: u32,
    },
    Snapshot {
        workspaces: Vec<Workspace>,
        /// The workspace to scope this client's project lists to: the
        /// last one opened anywhere, which is only ever a starting point —
        /// each client owns its scope from here (see OpenWorkspace).
        active_workspace: WorkspaceId,
        projects: Vec<Project>,
        worktrees: Vec<Worktree>,
        agents: Vec<Agent>,
        terminals: Vec<TerminalTab>,
        /// Every EXTERNAL SESSION the user archived.
        archived_external_sessions: Vec<ArchivedExternalSession>,
    },

    Ack {
        req_id: u64,
        created: Option<EntityId>,
    },
    /// Reply to `ListOrphanedSessions`, newest first.
    OrphanedSessions {
        req_id: u64,
        sessions: Vec<OrphanedSession>,
    },
    /// Reply to `ListBranches`.
    Branches {
        req_id: u64,
        branches: Vec<Branch>,
    },
    /// Reply to `EnterWorktree`: the worktree the agent now belongs to, and
    /// what that meant for its process.
    WorktreeEntered {
        req_id: u64,
        worktree: Worktree,
        outcome: EnterOutcome,
        #[serde(default)]
        warning: Option<String>,
    },
    Error {
        req_id: Option<u64>,
        message: String,
    },

    // -- deltas (pushed to all subscribers) --
    EntityUpserted {
        entity: Entity,
    },
    EntityRemoved {
        id: EntityId,
    },
    AgentSpawned {
        agent: AgentId,
        spawned_by: AgentId,
    },
    ExternalSessionArchived {
        session: ArchivedExternalSession,
    },
    ExternalSessionUnarchived {
        kind: AgentKind,
        session_id: String,
    },
    StatusChanged {
        agent: AgentId,
        status: AgentStatus,
        /// Epoch ms the change was stamped with (matches the persisted
        /// `status_changed_at`, so clients regroup consistently).
        changed_at: i64,
        /// The agent's `unseen` flag after this change: set when a live
        /// turn just finished, cleared when it left `finished`.
        #[serde(default)]
        unseen: bool,
        #[serde(default)]
        raised: Option<Attention>,
    },

    // -- PTY plane (only to clients attached to that session) --
    /// Ring replay on attach; client resets its parser before applying.
    Scrollback {
        session: SessionRef,
        base_seq: u64,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    /// Live coalesced output. `seq` = byte offset of the first byte.
    Output {
        session: SessionRef,
        seq: u64,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    SessionExited {
        session: SessionRef,
        exit_code: Option<i32>,
    },

    UsageLimits {
        req_id: u64,
        windows: Vec<UsageWindow>,
    },

    /// Reply to `ClientRequest::GetMetrics`.
    Metrics {
        req_id: u64,
        snapshot: MetricsSnapshot,
    },

    FeedbackRequestChanged {
        agent: AgentId,
        request: Option<FeedbackRequest>,
    },

    RouterConfig {
        req_id: u64,
        json: String,
        path: String,
    },

    TaskNote {
        req_id: u64,
        worktree: WorktreeId,
        text: String,
    },

    TaskNoteChanged {
        worktree: WorktreeId,
        text: String,
    },

    ProjectSetup {
        req_id: u64,
        project: ProjectId,
        command: String,
    },

    Checkpoints {
        req_id: u64,
        agent: AgentId,
        checkpoints: Vec<Checkpoint>,
    },

    Triggers {
        req_id: u64,
        triggers: Vec<PromptTrigger>,
    },

    TriggerCreated {
        req_id: u64,
        trigger: PromptTrigger,
        secret: Option<String>,
    },

    Channels {
        req_id: u64,
        channels: Vec<ChannelSummary>,
    },

    ChannelEvents {
        req_id: u64,
        events: Vec<ChannelEvent>,
    },

    ChannelPresence {
        req_id: u64,
        here: Vec<ChannelPresent>,
    },

    ChannelOpened {
        req_id: u64,
        key: String,
    },

    ChannelTouched {
        req_id: u64,
        touched: Vec<ChannelTouching>,
    },

    ChannelGrants {
        req_id: u64,
        grants: Vec<ChannelGrant>,
    },

    AcpTranscript {
        req_id: u64,
        agent: AgentId,
        items: Vec<AcpItemAt>,
    },

    AcpItem {
        agent: AgentId,
        seq: u64,
        item: Box<AcpItem>,
    },

    ChannelAppended {
        channel: String,
        seq: i64,
    },

    TranscribeVoice {
        request: u64,
        pcm_path: String,
    },

    AcpSessionState {
        req_id: u64,
        agent: AgentId,
        state: Box<AcpSessionState>,
    },

    AcpSessionChanged {
        agent: AgentId,
        state: Box<AcpSessionState>,
    },

    AcpTruncated {
        agent: AgentId,
        from_seq: u64,
    },

    AcpQueueCleared {
        agent: AgentId,
        texts: Vec<String>,
    },

    AcpAttachment {
        req_id: u64,
        data: String,
    },

    WorktreeFiles {
        req_id: u64,
        files: Vec<String>,
    },

    Hub {
        req_id: u64,
        json: String,
    },

    HubChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelPresent {
    pub member: String,
    pub name: String,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelTouching {
    pub member: String,
    pub name: String,
    pub agent: String,
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelGrant {
    pub channel: String,
    pub agent: AgentId,
    pub member: String,
    pub member_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSummary {
    pub key: String,
    pub head: i64,
    pub unread: i64,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub mine: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelEvent {
    pub seq: i64,
    pub kind: u32,
    pub author: String,
    pub author_name: String,
    pub author_kind: String,
    pub channel: String,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub created_at: i64,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub author_agent: Option<AgentId>,
    #[serde(default)]
    pub mentioned_agents: Vec<AgentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TriggerId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerWhen {
    At {
        run_at: i64,
        repeat_secs: Option<u64>,
    },
    Webhook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerTarget {
    Agent(AgentId),
    NewAgent {
        worktree: WorktreeId,
        kind: AgentKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTrigger {
    pub name: String,
    pub when: TriggerWhen,
    pub target: TriggerTarget,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTrigger {
    pub id: TriggerId,
    pub name: String,
    pub when: TriggerWhen,
    pub target: TriggerTarget,
    pub prompt: String,
    pub last_fired_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub number: u32,
    pub created_at: i64,
    pub before_restore: bool,
}
