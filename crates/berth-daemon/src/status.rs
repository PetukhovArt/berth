//! The agent status state machine. Pure logic — no I/O, injected clock — so
//! the tricky interleavings (Stop racing SubagentStart, post-turn helpers,
//! idle notifications, foreign sessions) are unit-testable.
//!
//! Semantics, in the order they are evaluated:
//! - `Stop` does NOT mean done while Task-tool subagents are still active;
//!   hold `running` and promote to `finished` only after a drain grace.
//! - A `SubagentStart` shortly *after* a finish heals back to `running`
//!   (the Stop raced the subagent's own hook POST) — but only within a
//!   window, because Claude runs post-turn helpers that fire subagent events
//!   with no following Stop.
//! - Hooks from a different Claude session in the same cwd are foreign and
//!   must not drive status.
//! - An `idle_prompt` notification is Claude reporting it is parked at the
//!   input box; it is what un-sticks a turn that ended without a `Stop`
//!   (rejected prompt, escape mid-turn). But since Claude Code 2.1 the Agent
//!   tool runs subagents in the *background*: the foreground turn ends, the
//!   input box comes back, and `idle_prompt` fires ~60 s later while the
//!   workers are still going. So with subagents tracked it is a hold, not a
//!   finish — the same hold a gated `Stop` gets — and only a quiet set
//!   (no subagent hook traffic for `SUBAGENT_QUIET_GRACE`) is presumed
//!   orphaned and finished anyway.
//! - `Progress` is the same end-of-turn news read straight off the PTY
//!   (OSC 9;4, see `pty::progress`). It is the only signal that survives a
//!   user cancel — no hook fires at all there, and Claude suppresses
//!   `idle_prompt` precisely because the user just touched the keyboard.
//! - That event is an *edge*, though, so a turn whose status moved after the
//!   CLI had already parked has no second edge coming and wedges — red
//!   especially, since every other exit from `needs_feedback` is an event
//!   that may never arrive. `tick` therefore also reads the CLI's current
//!   progress *level* and closes such a turn out.
//! - `TitleIdle` is the same news again for a CLI that has stopped emitting
//!   OSC 9;4 (Claude Code 2.1.270 does not emit it at all, so `Progress`
//!   never fires there). It is the weakest of the three and is gated
//!   hardest: the window title also reads idle while a permission prompt
//!   waits, so it may only close out a turn that is *already* `running`,
//!   never one on `needs_feedback`, and it may never start one.

use berth_core::AgentStatus;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const RECENT_FINISH_WINDOW: Duration = Duration::from_secs(30);
pub const DRAIN_GRACE: Duration = Duration::from_secs(180);
pub const SUBAGENT_TTL: Duration = Duration::from_secs(2 * 60 * 60);
/// How long a held Stop tolerates tracked subagents that show no sign of
/// life — no SubagentStart/SubagentStop, no subagent tool traffic — before
/// presuming they were killed without a SubagentStop and finishing anyway.
/// Generous on purpose: a fleet implementer's single `cargo test` can be
/// silent for many minutes, and a wrong green here is the bug this guards
/// against.
pub const SUBAGENT_QUIET_GRACE: Duration = Duration::from_secs(30 * 60);
pub const MAX_TRACKED_SUBAGENTS: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub enum HookEvent {
    UserPromptSubmit,
    Stop,
    SessionStart {
        source: Option<String>,
    },
    PermissionRequest {
        tool_name: Option<String>,
        subagent_id: Option<String>,
        detail: Option<String>,
    },
    Notification {
        notification_type: Option<String>,
    },
    PreToolUse {
        tool_name: Option<String>,
        /// Set when the call came from a subagent (claude stamps
        /// `agent_id` on subagent tool traffic): a sign of life for the
        /// STOP GATE's quiet clock, nothing more.
        subagent_id: Option<String>,
    },
    PostToolUse {
        tool_name: Option<String>,
        subagent_id: Option<String>,
        detail: Option<String>,
    },
    SubagentStart {
        subagent_id: Option<String>,
    },
    SubagentStop {
        subagent_id: Option<String>,
    },
    /// Synthetic: the agent's PTY died.
    SessionEnded {
        exit_code: Option<i32>,
    },
    /// Synthetic: the CLI's OSC 9;4 progress state flipped. `busy: false` is
    /// end-of-turn — including the cancel that fires no hook.
    Progress {
        busy: bool,
    },
    /// Synthetic: the CLI's window-title glyph flipped (`pty::title`). The
    /// fallback for a CLI with no progress bar left; `idle: true` ends a
    /// turn, but only one already believed to be running.
    TitleIdle {
        idle: bool,
    },
    Tick {
        progress_busy: Option<bool>,
    },
    FeedbackAnswered {
        declined: bool,
    },
    Killed,
    DaemonBooted,
}

impl HookEvent {
    /// Events that (re)establish which Claude session id owns this agent.
    pub fn captures_session(&self) -> bool {
        matches!(
            self,
            HookEvent::UserPromptSubmit | HookEvent::SessionStart { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    SetStatus(AgentStatus),
    SaveSessionId(String),
    ArchiveClearedSession(String),
}

#[derive(Default)]
struct SubagentSet {
    keyed: HashMap<String, Instant>,
    anon: usize,
}

impl SubagentSet {
    fn start(&mut self, id: Option<String>, now: Instant) {
        match id {
            Some(id) if self.keyed.len() < MAX_TRACKED_SUBAGENTS => {
                self.keyed.insert(id, now);
            }
            Some(_) | None => self.anon = self.anon.saturating_add(1).min(MAX_TRACKED_SUBAGENTS),
        }
    }

    /// Cross-cancel bias toward finishing: a keyed stop with no matching start
    /// cancels an anon start; an anon stop cancels the oldest keyed start.
    fn stop(&mut self, id: Option<String>) {
        match id {
            Some(id) => {
                if self.keyed.remove(&id).is_none() {
                    self.anon = self.anon.saturating_sub(1);
                }
            }
            None => {
                if self.anon > 0 {
                    self.anon -= 1;
                } else if let Some(oldest) = self
                    .keyed
                    .iter()
                    .min_by_key(|(_, t)| **t)
                    .map(|(k, _)| k.clone())
                {
                    self.keyed.remove(&oldest);
                }
            }
        }
    }

    fn prune_expired(&mut self, now: Instant) {
        self.keyed
            .retain(|_, started| now.duration_since(*started) < SUBAGENT_TTL);
        // Anon starts can't be aged individually; they are cleared wholesale
        // on session change, `/clear` and session end. Deliberately not on
        // `UserPromptSubmit`: background workers outlive a prompt.
    }

    fn is_empty(&self) -> bool {
        self.keyed.is_empty() && self.anon == 0
    }

    fn clear(&mut self) {
        self.keyed.clear();
        self.anon = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StopGate {
    Open,
    HeldForSubagents,
    DrainingSince(Instant),
}

impl StopGate {
    fn is_held(self) -> bool {
        self != StopGate::Open
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AskedToolCall {
    tool_name: String,
    subagent_id: Option<String>,
    detail: Option<String>,
}

impl AskedToolCall {
    fn of(
        tool_name: &Option<String>,
        subagent_id: &Option<String>,
        detail: &Option<String>,
    ) -> Option<Self> {
        Some(AskedToolCall {
            tool_name: tool_name.clone()?,
            subagent_id: subagent_id.clone(),
            detail: detail.clone(),
        })
    }

    fn ran_as(
        &self,
        tool_name: &Option<String>,
        subagent_id: &Option<String>,
        detail: &Option<String>,
    ) -> bool {
        Some(self) == Self::of(tool_name, subagent_id, detail).as_ref()
    }

    fn passed_by(&self, tool_name: &Option<String>, subagent_id: &Option<String>) -> bool {
        tool_name.is_some()
            && *subagent_id == self.subagent_id
            && NEVER_RUN_IN_PARALLEL.contains(&self.tool_name.as_str())
    }
}

const NEVER_RUN_IN_PARALLEL: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

#[derive(Debug, Clone, PartialEq)]
enum State {
    Fresh,
    Running(StopGate),
    NeedsFeedback {
        gate: StopGate,
        asked: Option<AskedToolCall>,
    },
    Finished {
        finished_at: Option<Instant>,
    },
    Terminated,
    Disconnected,
}

impl State {
    fn restored(status: AgentStatus) -> Self {
        match status {
            AgentStatus::Fresh => State::Fresh,
            AgentStatus::Running => State::Running(StopGate::Open),
            AgentStatus::NeedsFeedback => State::NeedsFeedback {
                gate: StopGate::Open,
                asked: None,
            },
            AgentStatus::Finished => State::Finished { finished_at: None },
            AgentStatus::Terminated => State::Terminated,
            AgentStatus::Disconnected => State::Disconnected,
        }
    }

    fn status(&self) -> AgentStatus {
        match self {
            State::Fresh => AgentStatus::Fresh,
            State::Running(_) => AgentStatus::Running,
            State::NeedsFeedback { .. } => AgentStatus::NeedsFeedback,
            State::Finished { .. } => AgentStatus::Finished,
            State::Terminated => AgentStatus::Terminated,
            State::Disconnected => AgentStatus::Disconnected,
        }
    }

    fn gate(&self) -> StopGate {
        match self {
            State::Running(gate) | State::NeedsFeedback { gate, .. } => *gate,
            _ => StopGate::Open,
        }
    }

    fn asked(&self) -> Option<AskedToolCall> {
        match self {
            State::NeedsFeedback { asked, .. } => asked.clone(),
            _ => None,
        }
    }

    fn holds_stop(&self) -> bool {
        matches!(self, State::Running(gate) if gate.is_held())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Delivery {
    SameSession,
    AdoptedSession,
}

pub struct AgentStatusMachine {
    state: State,
    session_id: Option<String>,
    subagents: SubagentSet,
    /// The last moment the tracked subagents proved they are alive — a
    /// SubagentStart/Stop or subagent tool traffic — or, failing that, when
    /// the hold began. `tick` gives up on the hold once this is
    /// `SUBAGENT_QUIET_GRACE` old.
    subagent_alive_at: Option<Instant>,
    /// Set by a `tick` that found the CLI parked while the status still said
    /// otherwise. Acting on it takes a second sighting, so a prompt whose
    /// `UserPromptSubmit` arrived ahead of the CLI's own busy byte cannot
    /// read as a finished turn for the width of one tick.
    idle_level_seen: bool,
}

impl AgentStatusMachine {
    pub fn new(status: AgentStatus, session_id: Option<String>) -> Self {
        Self {
            state: State::restored(status),
            session_id,
            subagents: SubagentSet::default(),
            subagent_alive_at: None,
            idle_level_seen: false,
        }
    }

    pub fn status(&self) -> AgentStatus {
        self.state.status()
    }

    pub fn handle(
        &mut self,
        event: HookEvent,
        payload_session_id: Option<&str>,
        now: Instant,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        let Some(delivery) = self.reconcile_session(&event, payload_session_id, &mut effects)
        else {
            return effects; // foreign session — ignore entirely
        };
        self.track_subagents(&event, now);
        if let Some(next) = self.transition(&event, delivery, now) {
            let before = self.state.status();
            self.state = next;
            let after = self.state.status();
            if after != before {
                effects.push(Effect::SetStatus(after));
            }
        }
        effects
    }

    fn reconcile_session(
        &mut self,
        event: &HookEvent,
        payload_session_id: Option<&str>,
        effects: &mut Vec<Effect>,
    ) -> Option<Delivery> {
        if event.captures_session() {
            if let Some(sid) = payload_session_id {
                if self.session_id.as_deref() != Some(sid) {
                    // New claude process (restart / manual relaunch): adopt it
                    // and forget the old session's subagents.
                    let cleared =
                        matches!(event, HookEvent::SessionStart { source } if is_clear(source));
                    if let Some(old) = self.session_id.take().filter(|_| cleared) {
                        effects.push(Effect::ArchiveClearedSession(old));
                    }
                    self.session_id = Some(sid.to_string());
                    self.subagents.clear();
                    self.subagent_alive_at = None;
                    effects.push(Effect::SaveSessionId(sid.to_string()));
                    return Some(Delivery::AdoptedSession);
                }
            }
        } else if !matches!(event, HookEvent::SessionEnded { .. }) {
            if let (Some(mine), Some(theirs)) = (self.session_id.as_deref(), payload_session_id) {
                if mine != theirs {
                    return None;
                }
            }
        }
        Some(Delivery::SameSession)
    }

    fn track_subagents(&mut self, event: &HookEvent, now: Instant) {
        match event {
            HookEvent::SessionStart { source } if is_clear(source) => {
                // /clear killed any live subagents.
                self.subagents.clear();
                self.subagent_alive_at = None;
            }
            HookEvent::PreToolUse {
                subagent_id: Some(_),
                ..
            }
            | HookEvent::PostToolUse {
                subagent_id: Some(_),
                ..
            } => self.note_subagent_alive(now),
            HookEvent::SubagentStart { subagent_id } => {
                self.subagents.start(subagent_id.clone(), now);
                self.note_subagent_alive(now);
            }
            HookEvent::SubagentStop { subagent_id } => {
                self.subagents.stop(subagent_id.clone());
                self.note_subagent_alive(now);
            }
            HookEvent::SessionEnded { .. } => {
                // Dead process: laggard subagent POSTs must never resurrect it.
                self.subagents.clear();
                self.subagent_alive_at = None;
            }
            _ => {}
        }
    }

    fn transition(&mut self, event: &HookEvent, delivery: Delivery, now: Instant) -> Option<State> {
        use HookEvent as E;
        use State::*;
        let state = self.state.clone();
        let kept_gate = state.gate();
        let kept_ask = state.asked();
        let adopted = delivery == Delivery::AdoptedSession;
        match (&state, event) {
            (_, E::UserPromptSubmit) => Some(self.start_turn()),

            (_, E::Stop) => Some(self.end_turn(now)),

            (Running(_) | NeedsFeedback { .. }, E::SessionStart { source }) if is_clear(source) => {
                // `/clear` itself fired `UserPromptSubmit` but runs no
                // turn, so no `Stop` or title edge follows: without this
                // the fresh conversation sits on running forever.
                Some(Fresh)
            }
            (Running(gate), E::SessionStart { .. }) if adopted && gate.is_held() => {
                Some(Running(StopGate::Open))
            }
            (NeedsFeedback { gate, asked }, E::SessionStart { .. })
                if adopted && gate.is_held() =>
            {
                Some(NeedsFeedback {
                    gate: StopGate::Open,
                    asked: asked.clone(),
                })
            }
            (Finished { .. }, E::SessionStart { source }) if is_clear(source) => {
                Some(Finished { finished_at: None })
            }
            (Terminated | Disconnected, E::SessionStart { source }) => {
                Some(if starts_new_conversation(source) {
                    Fresh
                } else {
                    Finished { finished_at: None }
                })
            }
            (_, E::SessionStart { .. }) => None,

            (
                _,
                E::PermissionRequest {
                    tool_name,
                    subagent_id,
                    detail,
                },
            ) => Some(NeedsFeedback {
                gate: kept_gate,
                asked: AskedToolCall::of(tool_name, subagent_id, detail),
            }),

            (_, E::Notification { notification_type })
                if notified(notification_type, "permission_prompt") =>
            {
                Some(NeedsFeedback {
                    gate: kept_gate,
                    asked: kept_ask,
                })
            }
            // "Claude is waiting for your input". Claude fires this
            // only with nothing in flight and no dialog open, so it
            // means the turn really is over — see `mark_idle`.
            (Running(_) | NeedsFeedback { .. }, E::Notification { notification_type })
                if notified(notification_type, "idle_prompt") =>
            {
                Some(self.mark_idle(now))
            }
            // Every other notification type (auth, quota, nested
            // fleet sessions) is none of our business.
            (_, E::Notification { .. }) => None,

            (_, E::PreToolUse { tool_name, .. }) if asks_the_user(tool_name) => {
                Some(NeedsFeedback {
                    gate: kept_gate,
                    asked: None,
                })
            }
            (_, E::PreToolUse { .. }) => None,
            (_, E::PostToolUse { tool_name, .. }) if asks_the_user(tool_name) => {
                Some(Running(kept_gate))
            }
            (
                NeedsFeedback {
                    gate,
                    asked: Some(asked),
                },
                E::PostToolUse {
                    tool_name,
                    subagent_id,
                    detail,
                },
            ) if asked.ran_as(tool_name, subagent_id, detail)
                || asked.passed_by(tool_name, subagent_id) =>
            {
                Some(Running(*gate))
            }
            (_, E::PostToolUse { .. }) => None,

            (NeedsFeedback { gate, .. }, E::FeedbackAnswered { declined: false }) => {
                Some(Running(*gate))
            }
            (NeedsFeedback { .. }, E::FeedbackAnswered { declined: true }) => {
                Some(self.end_turn(now))
            }
            (_, E::FeedbackAnswered { .. }) => None,

            // The Stop raced this subagent's own POST — heal.
            (
                Finished {
                    finished_at: Some(finished),
                },
                E::SubagentStart { .. },
            ) if now.duration_since(*finished) < RECENT_FINISH_WINDOW => {
                Some(Running(StopGate::HeldForSubagents))
            }
            // Post-turn internal helper (away-summary etc.): a
            // start with no Stop coming. Track it, don't heal —
            // healing here wedges the agent on running forever.
            (_, E::SubagentStart { .. }) => None,
            (_, E::SubagentStop { .. }) => None,

            // A turn started. Normally `UserPromptSubmit` already
            // said so; this also catches turns berth never saw a
            // prompt for (a resumed session, a scheduled wake-up).
            // Deliberately narrow: it must not talk over a pending
            // permission prompt (which holds progress at *busy*
            // anyway, so no edge arrives) or revive a dead agent.
            (Fresh | Finished { .. }, E::Progress { busy: true }) => Some(self.start_turn()),
            // The CLI cleared its progress bar: the turn is over,
            // however it ended. Same bookkeeping as `Stop`, so a
            // real Stop arriving either side of this is a no-op and
            // the subagent drain hold still applies.
            (Running(_) | NeedsFeedback { .. }, E::Progress { busy: false }) => {
                Some(self.end_turn(now))
            }
            // Fresh / Terminated / Disconnected are left alone: a CLI
            // clears its progress bar on startup and on exit too, and
            // neither is a finished turn.
            (_, E::Progress { .. }) => None,

            // Deliberately narrower than `Progress`. The title reads idle
            // while a permission prompt waits as well as when a turn is
            // over, so `needs_feedback` is excluded — and it is never a
            // reason to *start* a turn, since a CLI titles itself idle at
            // startup and on exit too.
            (Running(_), E::TitleIdle { idle: true }) => Some(self.end_turn(now)),
            (_, E::TitleIdle { .. }) => None,

            (Running(_) | NeedsFeedback { .. } | Disconnected, E::SessionEnded { exit_code }) => {
                Some(if *exit_code == Some(0) {
                    Finished { finished_at: None }
                } else {
                    Terminated
                })
            }
            (Finished { .. }, E::SessionEnded { .. }) => Some(Finished { finished_at: None }),
            (_, E::SessionEnded { .. }) => None,

            (
                Running(StopGate::Open)
                | NeedsFeedback {
                    gate: StopGate::Open,
                    ..
                },
                E::Tick {
                    progress_busy: Some(false),
                },
            ) => self.parked_twice(now),
            (Running(gate), E::Tick { .. }) if gate.is_held() => {
                self.idle_level_seen = false;
                self.drain(*gate, now)
            }
            (_, E::Tick { .. }) => {
                self.idle_level_seen = false;
                None
            }

            (NeedsFeedback { gate, .. }, E::Killed) => Some(NeedsFeedback {
                gate: *gate,
                asked: None,
            }),
            (_, E::Killed) => None,

            (Running(_) | NeedsFeedback { .. }, E::DaemonBooted) => Some(Disconnected),
            (_, E::DaemonBooted) => None,
        }
    }

    /// Periodic tick, two jobs.
    ///
    /// First, `progress_busy` is the CLI's *current* advertised busy state
    /// (`None` when it advertises none: codex, a session with no live
    /// PTY). `Progress` events report edges only, so an end-of-turn that
    /// landed while the status said something else leaves no second edge to
    /// wait for and the agent wedges — `needs_feedback` especially, since
    /// every other way out of it is an event that may never come after an
    /// Esc cancel. Reading the level closes that turn out. It cannot green
    /// out an agent that genuinely waits on the user: an open permission
    /// prompt holds the progress state at busy (see `pty::progress`).
    ///
    /// Second, the deferred finish: while a Stop is held open, promote to
    /// finished once the subagent set has drained and stayed empty for the
    /// grace period — or once the set has gone quiet for
    /// `SUBAGENT_QUIET_GRACE`, which means its SubagentStops are never coming
    /// (killed tasks, a crashed worker) and holding longer only wedges the
    /// agent on yellow.
    pub fn tick(&mut self, now: Instant, progress_busy: Option<bool>) -> Vec<Effect> {
        self.handle(HookEvent::Tick { progress_busy }, None, now)
    }

    fn parked_twice(&mut self, now: Instant) -> Option<State> {
        // A held Stop is exempt: the turn is already over and the drain owns
        // the status, so re-running `end_turn` here would reset its clock
        // every tick and the hold would never drain.
        if std::mem::replace(&mut self.idle_level_seen, true) {
            self.idle_level_seen = false;
            return Some(self.end_turn(now));
        }
        None
    }

    fn drain(&mut self, gate: StopGate, now: Instant) -> Option<State> {
        self.subagents.prune_expired(now);
        if self.subagents.is_empty() {
            return match gate {
                StopGate::DrainingSince(idle_since)
                    if now.duration_since(idle_since) >= DRAIN_GRACE =>
                {
                    self.subagent_alive_at = None;
                    Some(State::Finished {
                        finished_at: Some(now),
                    })
                }
                StopGate::DrainingSince(_) => None,
                StopGate::Open | StopGate::HeldForSubagents => {
                    Some(State::Running(StopGate::DrainingSince(now)))
                }
            };
        }
        let quiet_since = *self.subagent_alive_at.get_or_insert(now);
        if now.duration_since(quiet_since) >= SUBAGENT_QUIET_GRACE {
            // Orphaned: same exit as an idle notification with nothing
            // tracked, and no `finished_at` for the same reason.
            self.subagents.clear();
            self.subagent_alive_at = None;
            return Some(State::Finished { finished_at: None });
        }
        Some(State::Running(StopGate::HeldForSubagents))
    }

    /// Ids of the subagents a held Stop is still waiting on, or `None`
    /// when nothing is being held. The caller reads this before touching
    /// the disk, so an agent that is simply running costs no I/O.
    pub fn held_subagents(&self) -> Option<Vec<String>> {
        if !self.state.holds_stop() {
            return None;
        }
        let ids: Vec<String> = self.subagents.keyed.keys().cloned().collect();
        (!ids.is_empty()).then_some(ids)
    }

    /// Drop a subagent whose `SubagentStop` is never coming because the
    /// user killed it (see `crate::subagents`). Exact keyed match only: an
    /// id this machine does not know is ignored rather than cancelling an
    /// anonymous start, so a mismatch degrades to the quiet grace instead
    /// of finishing a turn whose workers are still alive.
    pub fn forget_subagent(&mut self, id: &str) {
        self.subagents.keyed.remove(id);
    }

    /// A tracked subagent just proved it is alive (its own hook traffic):
    /// restart the quiet clock the hold is measured against.
    fn note_subagent_alive(&mut self, now: Instant) {
        self.subagent_alive_at = Some(now);
    }

    /// The foreground turn ended — a `Stop`, or the CLI clearing its
    /// progress bar. Finished outright when no subagent is still tracked;
    /// otherwise the stop is held at running and `tick` promotes it once
    /// the set has drained and stayed empty for the grace period.
    fn end_turn(&mut self, now: Instant) -> State {
        self.subagents.prune_expired(now);
        if self.subagents.is_empty() {
            State::Finished {
                finished_at: Some(now),
            }
        } else {
            // Foreground turn ended but subagents are still working.
            self.hold_for_subagents(now)
        }
    }

    fn start_turn(&mut self) -> State {
        self.subagent_alive_at = None;
        State::Running(StopGate::Open)
    }

    /// Keep (or put) the agent at running because subagents are still
    /// tracked. The quiet clock starts now unless a subagent has already
    /// shown life more recently than the hold.
    fn hold_for_subagents(&mut self, now: Instant) -> State {
        self.subagent_alive_at.get_or_insert(now);
        State::Running(StopGate::HeldForSubagents)
    }

    /// Claude reports itself idle at the input box. This is the only end-of-
    /// turn signal that survives the paths where no `Stop` ever fires: the
    /// user rejecting a permission prompt or an `AskUserQuestion`, or hitting
    /// escape mid-turn. Without it those leave the agent pinned on red (or
    /// yellow) until the next prompt, long after the CLI went quiet.
    ///
    /// Safe to trust as "no dialog is open" because Claude gates the
    /// notification on an idle main loop AND an empty dialog stack — a
    /// permission prompt still waiting on the user suppresses it, so this
    /// can't green out an agent that genuinely needs feedback.
    ///
    /// It is *not* proof that the turn's work is over: the Agent tool runs
    /// its subagents in the background, the foreground loop parks at the
    /// input box while they work, and this fires ~60 s in. With subagents
    /// still tracked the agent stays at running exactly as a gated `Stop`
    /// would — `tick` finishes it once they drain, or once they have been
    /// quiet for `SUBAGENT_QUIET_GRACE`.
    fn mark_idle(&mut self, now: Instant) -> State {
        self.subagents.prune_expired(now);
        if !self.subagents.is_empty() {
            return self.hold_for_subagents(now);
        }
        self.subagent_alive_at = None;
        // Deliberately no `finished_at`: after this much idle time a
        // SubagentStart is a post-turn helper, never a Stop that raced its
        // own POST, so it must not heal back to running.
        State::Finished { finished_at: None }
    }
}

fn is_clear(source: &Option<String>) -> bool {
    source.as_deref() == Some("clear")
}

fn starts_new_conversation(source: &Option<String>) -> bool {
    matches!(source.as_deref(), Some("startup" | "clear"))
}

fn notified(notification_type: &Option<String>, kind: &str) -> bool {
    notification_type.as_deref() == Some(kind)
}

fn asks_the_user(tool_name: &Option<String>) -> bool {
    tool_name.as_deref() == Some("AskUserQuestion")
}

pub fn after_daemon_boot(status: AgentStatus) -> Option<AgentStatus> {
    AgentStatusMachine::new(status, None)
        .handle(HookEvent::DaemonBooted, None, Instant::now())
        .into_iter()
        .find_map(|effect| match effect {
            Effect::SetStatus(status) => Some(status),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    fn status_of(effects: &[Effect]) -> Option<AgentStatus> {
        effects.iter().rev().find_map(|e| match e {
            Effect::SetStatus(s) => Some(*s),
            _ => None,
        })
    }

    #[test]
    fn normal_turn_lifecycle() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        let fx = m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
        assert!(fx.contains(&Effect::SaveSessionId("s1".into())));
        let fx = m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(10));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    /// The escape cancel, on a CLI that emits no OSC 9;4 at all: the title
    /// glyph is the only news the turn ended, and without it the agent sits
    /// on yellow forever.
    #[test]
    fn a_title_idle_edge_ends_a_cancelled_turn() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let fx = m.handle(
            HookEvent::TitleIdle { idle: true },
            Some("s1"),
            now + Duration::from_secs(10),
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    /// The gate that makes the title safe to read at all. Claude titles
    /// itself idle while a permission prompt waits, so widening this to
    /// `needs_feedback` would green out every agent that is genuinely
    /// waiting on the user.
    #[test]
    fn a_title_idle_edge_never_greens_out_a_waiting_agent() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        let fx = m.handle(
            HookEvent::TitleIdle { idle: true },
            Some("s1"),
            now + Duration::from_secs(1),
        );
        assert_eq!(status_of(&fx), None, "the prompt is still waiting");
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
    }

    /// A CLI titles itself idle at startup and on exit too, so the title is
    /// never a reason to believe a turn began.
    #[test]
    fn a_title_edge_never_starts_a_turn() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        let fx = m.handle(HookEvent::TitleIdle { idle: false }, None, now);
        assert_eq!(status_of(&fx), None);
        assert_eq!(m.status(), AgentStatus::Fresh);
    }

    #[test]
    fn permission_prompt_flow() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let fx = m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::NeedsFeedback));
        // Answering the prompt resumes the turn (next event is tool traffic /
        // eventually Stop; a PostToolUse AskUserQuestion also flips back).
        let fx = m.handle(
            HookEvent::PostToolUse {
                tool_name: Some("AskUserQuestion".into()),
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    fn idle(m: &mut AgentStatusMachine, now: Instant) -> Vec<Effect> {
        m.handle(
            HookEvent::Notification {
                notification_type: Some("idle_prompt".into()),
            },
            Some("s1"),
            now,
        )
    }

    #[test]
    fn idle_notification_is_a_noop_when_already_finished() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        let fx = idle(&mut m, now);
        assert!(fx.is_empty(), "already finished: {fx:?}");
        assert_eq!(m.status(), AgentStatus::Finished);
    }

    #[test]
    fn idle_notification_clears_a_rejected_question() {
        // The reported bug: AskUserQuestion goes red, the user rejects it,
        // and the interrupted turn fires neither PostToolUse nor Stop.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let fx = m.handle(
            HookEvent::PreToolUse {
                tool_name: Some("AskUserQuestion".into()),
                subagent_id: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::NeedsFeedback));
        let fx = idle(&mut m, now + Duration::from_secs(60));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn idle_notification_clears_a_stale_running_turn() {
        // Escape mid-turn: no Stop ever arrives, so running would stick.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let fx = idle(&mut m, now + Duration::from_secs(60));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    fn subagent(m: &mut AgentStatusMachine, start: bool, id: &str, now: Instant) -> Vec<Effect> {
        let ev = if start {
            HookEvent::SubagentStart {
                subagent_id: Some(id.into()),
            }
        } else {
            HookEvent::SubagentStop {
                subagent_id: Some(id.into()),
            }
        };
        m.handle(ev, Some("s1"), now)
    }

    #[test]
    fn idle_notification_holds_running_while_background_subagents_work() {
        // The reported bug: the Agent tool runs subagents in the background,
        // the foreground turn ends (Stop, held), and ~60 s later Claude's
        // idle_prompt fires with the workers still going — that must not
        // green the session out.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        assert_eq!(m.status(), AgentStatus::Running, "stop held open");
        let fx = idle(&mut m, now + Duration::from_secs(60));
        assert!(fx.is_empty(), "idle with live subagents is a hold: {fx:?}");
        assert_eq!(m.status(), AgentStatus::Running);
        assert!(m.tick(now + Duration::from_secs(90), None).is_empty());

        // The worker finishes and its completion re-invokes the foreground
        // turn (progress busy, then a real Stop): finished outright.
        subagent(&mut m, false, "sub1", now + Duration::from_secs(95));
        m.handle(
            HookEvent::Progress { busy: true },
            Some("s1"),
            now + Duration::from_secs(96),
        );
        assert_eq!(m.status(), AgentStatus::Running);
        let fx = m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(100));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn idle_hold_drains_through_the_grace_when_nothing_reinvokes_the_turn() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        idle(&mut m, now + Duration::from_secs(60));
        subagent(&mut m, false, "sub1", now + Duration::from_secs(120));
        let t = now + Duration::from_secs(121);
        assert!(m.tick(t, None).is_empty(), "drain grace starts");
        assert!(m
            .tick(t + DRAIN_GRACE - Duration::from_secs(1), None)
            .is_empty());
        let fx = m.tick(t + DRAIN_GRACE, None);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn idle_notification_with_a_rejected_prompt_still_holds_for_subagents() {
        // A permission prompt rejected while a background worker runs: the
        // dialog is gone (idle_prompt proves that) but the turn is not over.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        let fx = idle(&mut m, now + Duration::from_secs(60));
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    #[test]
    fn quiet_subagents_are_presumed_orphaned_after_the_quiet_grace() {
        // A worker killed without a SubagentStop: the hold must not wedge
        // the agent on yellow until SUBAGENT_TTL.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(5));
        idle(&mut m, now + Duration::from_secs(65));
        assert_eq!(m.status(), AgentStatus::Running);
        // The quiet clock runs from the last sign of life — the
        // SubagentStart at `now` — not from the Stop or the idle_prompt.
        assert!(m
            .tick(now + SUBAGENT_QUIET_GRACE - Duration::from_secs(1), None)
            .is_empty());
        let fx = m.tick(now + SUBAGENT_QUIET_GRACE, None);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
        // A helper subagent afterwards must not heal it back to running.
        let fx = subagent(
            &mut m,
            true,
            "sub2",
            now + SUBAGENT_QUIET_GRACE + Duration::from_secs(1),
        );
        assert!(fx.is_empty(), "post-orphan subagent must not heal: {fx:?}");
        assert_eq!(m.status(), AgentStatus::Finished);
    }

    #[test]
    fn subagent_traffic_resets_the_quiet_clock() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        idle(&mut m, now + Duration::from_secs(60));

        // Twenty minutes in, the worker runs a Bash call (agent_id stamped).
        let t1 = now + Duration::from_secs(20 * 60);
        m.handle(
            HookEvent::PostToolUse {
                tool_name: Some("Bash".into()),
                subagent_id: Some("sub1".into()),
                detail: None,
            },
            Some("s1"),
            t1,
        );
        assert!(m
            .tick(now + SUBAGENT_QUIET_GRACE + Duration::from_secs(1), None)
            .is_empty());
        // A sibling starting resets it again; the main agent's own tool
        // traffic (no agent_id) does not count.
        let t2 = t1 + Duration::from_secs(20 * 60);
        subagent(&mut m, true, "sub2", t2);
        m.handle(
            HookEvent::PostToolUse {
                tool_name: Some("Bash".into()),
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            t2 + Duration::from_secs(25 * 60),
        );
        assert!(m
            .tick(t1 + SUBAGENT_QUIET_GRACE + Duration::from_secs(1), None)
            .is_empty());
        let fx = m.tick(t2 + SUBAGENT_QUIET_GRACE, None);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn idle_notification_leaves_fresh_and_dead_agents_alone() {
        for start in [
            AgentStatus::Fresh,
            AgentStatus::Terminated,
            AgentStatus::Disconnected,
        ] {
            let mut m = AgentStatusMachine::new(start, Some("s1".into()));
            let fx = idle(&mut m, t0());
            assert!(fx.is_empty(), "{start:?} must not be touched: {fx:?}");
            assert_eq!(m.status(), start);
        }
    }

    #[test]
    fn foreign_idle_notification_is_ignored() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        let fx = m.handle(
            HookEvent::Notification {
                notification_type: Some("idle_prompt".into()),
            },
            Some("someone-elses-claude"),
            now,
        );
        assert!(fx.is_empty(), "foreign session: {fx:?}");
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
    }

    #[test]
    fn unknown_notification_types_are_ignored() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        for ty in [
            "auth_success",
            "agent_needs_input",
            "quota_auto_resume_fired",
        ] {
            let fx = m.handle(
                HookEvent::Notification {
                    notification_type: Some(ty.into()),
                },
                Some("s1"),
                now,
            );
            assert!(fx.is_empty(), "{ty} must not flip status: {fx:?}");
        }
        assert_eq!(m.status(), AgentStatus::Running);
    }

    #[test]
    fn stop_with_active_subagents_holds_running_until_drained() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now,
        );
        let fx = m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(5));
        assert_eq!(status_of(&fx), None, "stays running — no transition");
        assert_eq!(m.status(), AgentStatus::Running);

        // Subagent finishes; drain grace must elapse before finished.
        m.handle(
            HookEvent::SubagentStop {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now + Duration::from_secs(60),
        );
        let fx = m.tick(now + Duration::from_secs(61), None);
        assert!(fx.is_empty(), "grace not elapsed yet");
        let fx = m.tick(now + Duration::from_secs(61) + DRAIN_GRACE, None);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn subagent_start_shortly_after_finish_heals_to_running() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(10));
        assert_eq!(m.status(), AgentStatus::Finished);
        // The subagent's own POST arrives 2s later — race heal.
        let fx = m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now + Duration::from_secs(12),
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    #[test]
    fn post_turn_helper_outside_window_does_not_heal() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(10));
        // An away-summary helper fires SubagentStart minutes later.
        let fx = m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("helper".into()),
            },
            Some("s1"),
            now + Duration::from_secs(10) + RECENT_FINISH_WINDOW + Duration::from_secs(1),
        );
        assert!(fx.is_empty(), "must stay finished: {fx:?}");
        assert_eq!(m.status(), AgentStatus::Finished);
    }

    fn progress(m: &mut AgentStatusMachine, busy: bool, now: Instant) -> Vec<Effect> {
        m.handle(HookEvent::Progress { busy }, None, now)
    }

    #[test]
    fn progress_idle_finishes_a_cancelled_turn() {
        // The reported bug: the user hits escape. Claude Code fires no Stop
        // for an interrupted turn and never sends `idle_prompt` either (it
        // suppresses that when the user has just touched the keyboard), so
        // the only news is the progress bar clearing.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        progress(&mut m, true, now);
        assert_eq!(m.status(), AgentStatus::Running);
        let fx = progress(&mut m, false, now + Duration::from_secs(8));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn progress_idle_clears_a_rejected_permission_prompt() {
        // Escaping out of a permission prompt: same story, from red.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        let fx = progress(&mut m, false, now + Duration::from_secs(5));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    /// The wedge the level check exists for. `Progress` reports edges, so a
    /// status that moved to red *after* the CLI already parked has no second
    /// edge coming, and an Esc cancel fires no hook and no idle notification.
    /// Observed in the wild: a row sat on `needs_feedback` for a quarter of an
    /// hour with the CLI idle behind it.
    #[test]
    fn tick_finishes_a_turn_the_progress_edge_missed() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        // The CLI parks, then the permission prompt's hook lands: the edge is
        // spent and the status ends up red with nothing left to move it.
        progress(&mut m, false, now);
        m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now,
        );
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);

        let t = now + Duration::from_secs(30);
        assert!(
            m.tick(t, Some(false)).is_empty(),
            "one sighting is not proof"
        );
        let fx = m.tick(t + Duration::from_secs(30), Some(false));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    /// The level must not talk over a CLI that is still working, and must not
    /// speak at all for one that never advertises a level (codex, a
    /// session with no live PTY).
    #[test]
    fn tick_leaves_a_busy_or_silent_cli_alone() {
        for level in [Some(true), None] {
            let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
            let now = t0();
            m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
            assert_eq!(m.status(), AgentStatus::Running);
            for i in 1..4 {
                let fx = m.tick(now + Duration::from_secs(30 * i), level);
                assert!(fx.is_empty(), "level {level:?} must not finish a turn");
            }
            assert_eq!(m.status(), AgentStatus::Running);
        }
    }

    /// A held Stop is the drain's business. Ticking the level through it must
    /// not restart the drain clock, or the hold never promotes.
    #[test]
    fn tick_level_does_not_disturb_a_held_stop() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::SubagentStart { subagent_id: None },
            Some("s1"),
            now,
        );
        m.handle(HookEvent::Stop, Some("s1"), now);
        assert_eq!(m.status(), AgentStatus::Running, "held for the subagent");
        m.handle(
            HookEvent::SubagentStop { subagent_id: None },
            Some("s1"),
            now,
        );

        // The CLI is parked the whole way; the drain still owns the promotion.
        assert!(m.tick(now, Some(false)).is_empty(), "drain grace starts");
        let fx = m.tick(now + DRAIN_GRACE, Some(false));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn progress_idle_leaves_fresh_and_dead_agents_alone() {
        // Every CLI clears its progress bar at startup and again on exit;
        // neither is a finished turn.
        for start in [
            AgentStatus::Fresh,
            AgentStatus::Finished,
            AgentStatus::Terminated,
            AgentStatus::Disconnected,
        ] {
            let mut m = AgentStatusMachine::new(start, Some("s1".into()));
            let fx = progress(&mut m, false, t0());
            assert!(fx.is_empty(), "{start:?} must not be touched: {fx:?}");
            assert_eq!(m.status(), start);
        }
    }

    #[test]
    fn progress_busy_starts_a_turn_but_never_talks_over_feedback() {
        // A turn berth saw no prompt for (resumed session, scheduled wake).
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, Some("s1".into()));
        let now = t0();
        let fx = progress(&mut m, true, now);
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));

        // …but a pending question outranks it, and the dead stay dead.
        for start in [
            AgentStatus::NeedsFeedback,
            AgentStatus::Terminated,
            AgentStatus::Disconnected,
        ] {
            let mut m = AgentStatusMachine::new(start, Some("s1".into()));
            let fx = progress(&mut m, true, now);
            assert!(fx.is_empty(), "{start:?} must not be touched: {fx:?}");
            assert_eq!(m.status(), start);
        }
    }

    #[test]
    fn progress_idle_respects_the_subagent_drain_hold() {
        // The progress bar clears when the *main loop* parks, which on a
        // normal turn end beats the Stop hook's HTTP round-trip. It must not
        // finish out from under still-running subagents.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now,
        );
        let fx = progress(&mut m, false, now + Duration::from_secs(5));
        assert_eq!(status_of(&fx), None, "stays running — no transition");
        assert_eq!(m.status(), AgentStatus::Running);
        // The Stop that follows a beat later agrees, and the drain still owns
        // the promotion.
        let fx = m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(5));
        assert!(fx.is_empty());
        m.handle(
            HookEvent::SubagentStop {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now + Duration::from_secs(6),
        );
        let fx = m.tick(now + Duration::from_secs(7), None);
        assert!(fx.is_empty(), "grace not elapsed yet");
        let fx = m.tick(now + Duration::from_secs(7) + DRAIN_GRACE, None);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn progress_idle_preserves_the_subagent_race_heal() {
        // Progress clears just before the Stop hook lands. Both stamp
        // `finished_at`, so a subagent POST that raced the Stop still heals —
        // unlike `mark_idle`, which deliberately blocks healing.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        progress(&mut m, false, now + Duration::from_secs(10));
        m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(10));
        assert_eq!(m.status(), AgentStatus::Finished);
        let fx = m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now + Duration::from_secs(12),
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    #[test]
    fn progress_edges_across_a_whole_turn_settle_on_finished() {
        // The captured Claude Code 2.1.241 sequence, hooks and all.
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        progress(&mut m, false, now); // startup, parked at the input box
        assert_eq!(m.status(), AgentStatus::Fresh);
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        progress(&mut m, true, now);
        m.handle(
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now + Duration::from_secs(3),
        );
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        // Approving does not move the progress bar — it never left "busy".
        m.handle(
            HookEvent::PostToolUse {
                tool_name: Some("Bash".into()),
                subagent_id: None,
                detail: None,
            },
            Some("s1"),
            now + Duration::from_secs(19),
        );
        let fx = progress(&mut m, false, now + Duration::from_secs(20));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn foreign_session_events_are_ignored() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        // A manually-launched claude in the same cwd posts a Stop.
        let fx = m.handle(HookEvent::Stop, Some("other-session"), now);
        assert!(fx.is_empty());
        assert_eq!(m.status(), AgentStatus::Running);
    }

    #[test]
    fn new_session_id_on_capture_adopts_and_clears_subagents() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now,
        );
        // claude restarted (new session id) and a fresh prompt arrives.
        let fx = m.handle(
            HookEvent::UserPromptSubmit,
            Some("s2"),
            now + Duration::from_secs(5),
        );
        assert!(fx.contains(&Effect::SaveSessionId("s2".into())));
        // Old subagents gone: a Stop finishes immediately.
        let fx = m.handle(HookEvent::Stop, Some("s2"), now + Duration::from_secs(6));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn pty_death_while_running_terminates_and_blocks_heal() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let fx = m.handle(
            HookEvent::SessionEnded {
                exit_code: Some(137),
            },
            None,
            now,
        );
        assert_eq!(status_of(&fx), Some(AgentStatus::Terminated));
        // Laggard subagent POST must not resurrect the dead agent.
        let fx = m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now + Duration::from_secs(1),
        );
        assert!(fx.is_empty());
        assert_eq!(m.status(), AgentStatus::Terminated);
    }

    #[test]
    fn pty_clean_exit_while_running_is_finished() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let fx = m.handle(HookEvent::SessionEnded { exit_code: Some(0) }, None, now);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn a_restored_cli_that_dies_leaves_disconnected() {
        let now = t0();
        let mut m = AgentStatusMachine::new(AgentStatus::Disconnected, Some("s1".into()));
        let fx = m.handle(HookEvent::SessionEnded { exit_code: Some(1) }, None, now);
        assert_eq!(status_of(&fx), Some(AgentStatus::Terminated));

        let mut m = AgentStatusMachine::new(AgentStatus::Disconnected, Some("s1".into()));
        let fx = m.handle(HookEvent::SessionEnded { exit_code: Some(0) }, None, now);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn pty_exit_when_already_finished_keeps_finished() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        let fx = m.handle(HookEvent::SessionEnded { exit_code: Some(1) }, None, now);
        assert!(
            fx.is_empty(),
            "finished agent whose pty closes stays finished"
        );
        assert_eq!(m.status(), AgentStatus::Finished);
    }

    #[test]
    fn anon_and_keyed_subagent_cross_cancel() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        // Keyed start, anon stop → biases toward finishing.
        m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now,
        );
        m.handle(
            HookEvent::SubagentStop { subagent_id: None },
            Some("s1"),
            now,
        );
        let fx = m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(1));
        assert_eq!(
            status_of(&fx),
            Some(AgentStatus::Finished),
            "set drained via cross-cancel"
        );
    }

    #[test]
    fn clear_source_session_start_clears_subagents() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        m.handle(
            HookEvent::SubagentStart {
                subagent_id: Some("sub1".into()),
            },
            Some("s1"),
            now,
        );
        m.handle(
            HookEvent::SessionStart {
                source: Some("clear".into()),
            },
            Some("s1"),
            now,
        );
        let fx = m.handle(HookEvent::Stop, Some("s1"), now + Duration::from_secs(1));
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn a_respawned_cli_leaves_disconnected() {
        let start = |source: &str| {
            let mut m = AgentStatusMachine::new(AgentStatus::Disconnected, Some("s1".into()));
            let event = HookEvent::SessionStart {
                source: Some(source.into()),
            };
            status_of(&m.handle(event, Some("s1"), t0()))
        };
        assert_eq!(start("resume"), Some(AgentStatus::Finished));
        assert_eq!(start("startup"), Some(AgentStatus::Fresh));

        let mut alive = AgentStatusMachine::new(AgentStatus::Finished, Some("s1".into()));
        let event = HookEvent::SessionStart {
            source: Some("resume".into()),
        };
        assert!(status_of(&alive.handle(event, Some("s1"), t0())).is_none());
    }

    #[test]
    fn a_respawned_cli_leaves_terminated() {
        let start = |source: &str| {
            let mut m = AgentStatusMachine::new(AgentStatus::Terminated, Some("s1".into()));
            let event = HookEvent::SessionStart {
                source: Some(source.into()),
            };
            status_of(&m.handle(event, Some("s1"), t0()))
        };
        assert_eq!(start("resume"), Some(AgentStatus::Finished));
        assert_eq!(start("startup"), Some(AgentStatus::Fresh));
        assert_eq!(start("clear"), Some(AgentStatus::Fresh));
    }

    #[test]
    fn the_allowed_tool_finishing_ends_the_wait() {
        let mut m = waiting_on_bash(None, "cargo test");
        let ran = bash_ran(None, "cargo test");
        let fx = m.handle(ran, Some("s1"), t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    fn waiting_on_bash(subagent: Option<&str>, command: &str) -> AgentStatusMachine {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let asked = HookEvent::PermissionRequest {
            tool_name: Some("Bash".into()),
            subagent_id: subagent.map(str::to_string),
            detail: Some(command.into()),
        };
        m.handle(asked, Some("s1"), now);
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        m
    }

    fn bash_ran(subagent: Option<&str>, command: &str) -> HookEvent {
        HookEvent::PostToolUse {
            tool_name: Some("Bash".into()),
            subagent_id: subagent.map(str::to_string),
            detail: Some(command.into()),
        }
    }

    #[test]
    fn another_call_of_the_same_tool_does_not_end_the_wait() {
        let mut m = waiting_on_bash(Some("sub1"), "cargo test");
        for other in [
            bash_ran(Some("sub1"), "git status"),
            bash_ran(None, "cargo test"),
            bash_ran(Some("sub2"), "cargo test"),
        ] {
            let fx = m.handle(other.clone(), Some("s1"), t0());
            assert!(status_of(&fx).is_none(), "{other:?}: {fx:?}");
        }
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        let fx = m.handle(bash_ran(Some("sub1"), "cargo test"), Some("s1"), t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    fn waiting_on_edit(subagent: Option<&str>) -> AgentStatusMachine {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        let asked = HookEvent::PermissionRequest {
            tool_name: Some("Edit".into()),
            subagent_id: subagent.map(str::to_string),
            detail: Some("D:/w/settings.json".into()),
        };
        m.handle(asked, Some("s1"), now);
        m
    }

    #[test]
    fn a_later_call_of_the_agent_that_asked_for_an_edit_ends_the_wait() {
        let mut m = waiting_on_edit(None);
        let fx = m.handle(bash_ran(Some("sub1"), "git status"), Some("s1"), t0());
        assert!(status_of(&fx).is_none(), "a subagent's call: {fx:?}");
        let fx = m.handle(bash_ran(None, "git status"), Some("s1"), t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));

        let mut m = waiting_on_edit(Some("sub1"));
        let fx = m.handle(bash_ran(None, "git status"), Some("s1"), t0());
        assert!(status_of(&fx).is_none(), "the main agent's call: {fx:?}");
        let fx = m.handle(bash_ran(Some("sub1"), "git status"), Some("s1"), t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    #[test]
    fn a_later_call_does_not_end_a_wait_on_a_tool_that_runs_in_parallel() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), t0());
        let asked = HookEvent::PermissionRequest {
            tool_name: Some("Read".into()),
            subagent_id: None,
            detail: Some("D:/elsewhere/notes.md".into()),
        };
        m.handle(asked, Some("s1"), t0());
        let fx = m.handle(bash_ran(None, "ls"), Some("s1"), t0());
        assert!(status_of(&fx).is_none(), "{fx:?}");
    }

    #[test]
    fn a_request_with_no_tool_ends_only_on_its_answer() {
        let mut m = AgentStatusMachine::new(AgentStatus::Running, Some("s1".into()));
        let anonymous = HookEvent::PermissionRequest {
            tool_name: None,
            subagent_id: None,
            detail: None,
        };
        m.handle(anonymous, Some("s1"), t0());
        let ran = HookEvent::PostToolUse {
            tool_name: None,
            subagent_id: None,
            detail: None,
        };
        assert!(m.handle(ran, Some("s1"), t0()).is_empty());
        let allowed = HookEvent::FeedbackAnswered { declined: false };
        let fx = m.handle(allowed, None, t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
    }

    #[test]
    fn an_allow_sent_from_berth_resumes_the_turn() {
        let mut m = waiting_on_bash(None, "cargo test");
        let allowed = HookEvent::FeedbackAnswered { declined: false };
        let fx = m.handle(allowed, None, t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
        let fx = m.handle(bash_ran(None, "cargo test"), Some("s1"), t0());
        assert!(fx.is_empty(), "already running: {fx:?}");
    }

    #[test]
    fn a_deny_sent_from_berth_ends_the_turn_or_holds_it_for_subagents() {
        let mut m = waiting_on_bash(None, "rm -rf dist");
        let denied = HookEvent::FeedbackAnswered { declined: true };
        let fx = m.handle(denied.clone(), None, t0());
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));

        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        let asked = HookEvent::PermissionRequest {
            tool_name: Some("Bash".into()),
            subagent_id: Some("sub1".into()),
            detail: Some("rm -rf dist".into()),
        };
        m.handle(asked, Some("s1"), now);
        let fx = m.handle(denied, None, now);
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
        assert_eq!(m.held_subagents(), Some(vec!["sub1".to_string()]));
    }

    #[test]
    fn an_answer_reaches_only_an_agent_that_waits() {
        for status in EVERY_STATUS {
            if status == AgentStatus::NeedsFeedback {
                continue;
            }
            for declined in [false, true] {
                let mut m = AgentStatusMachine::new(status, Some("s1".into()));
                let fx = m.handle(HookEvent::FeedbackAnswered { declined }, None, t0());
                assert!(fx.is_empty(), "{status:?} declined={declined}: {fx:?}");
            }
        }
    }

    #[test]
    fn a_kill_forgets_the_call_the_agent_waited_on() {
        let mut m = waiting_on_bash(None, "cargo test");
        assert!(m.handle(HookEvent::Killed, None, t0()).is_empty());
        let fx = m.handle(bash_ran(None, "cargo test"), Some("s1"), t0());
        assert!(fx.is_empty(), "{fx:?}");
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
    }

    const EVERY_STATUS: [AgentStatus; 6] = [
        AgentStatus::Fresh,
        AgentStatus::Running,
        AgentStatus::NeedsFeedback,
        AgentStatus::Finished,
        AgentStatus::Terminated,
        AgentStatus::Disconnected,
    ];

    #[test]
    fn the_boot_sweep_disconnects_only_live_agents() {
        for status in EVERY_STATUS {
            let live = matches!(status, AgentStatus::Running | AgentStatus::NeedsFeedback);
            let expected = live.then_some(AgentStatus::Disconnected);
            assert_eq!(after_daemon_boot(status), expected, "{status:?}");
        }
    }

    #[test]
    fn a_kill_leaves_every_status_as_it_was() {
        for status in EVERY_STATUS {
            let mut m = AgentStatusMachine::new(status, Some("s1".into()));
            let fx = m.handle(HookEvent::Killed, None, t0());
            assert!(fx.is_empty(), "{status:?}: {fx:?}");
            assert_eq!(m.status(), status);
        }
    }

    #[test]
    fn dead_agents_ignore_turn_signals_and_subagent_traffic() {
        let ignored = || {
            vec![
                HookEvent::Progress { busy: true },
                HookEvent::Progress { busy: false },
                HookEvent::TitleIdle { idle: true },
                HookEvent::Notification {
                    notification_type: Some("idle_prompt".into()),
                },
                HookEvent::SubagentStart {
                    subagent_id: Some("sub1".into()),
                },
                HookEvent::SubagentStop {
                    subagent_id: Some("sub1".into()),
                },
                HookEvent::SessionEnded { exit_code: Some(1) },
                HookEvent::Killed,
                HookEvent::DaemonBooted,
            ]
        };
        for status in [AgentStatus::Terminated, AgentStatus::Disconnected] {
            let mut m = AgentStatusMachine::new(status, Some("s1".into()));
            let now = t0();
            for event in ignored() {
                if status == AgentStatus::Disconnected
                    && matches!(event, HookEvent::SessionEnded { .. })
                {
                    continue;
                }
                let fx = m.handle(event.clone(), Some("s1"), now);
                assert!(fx.is_empty(), "{status:?} took {event:?}: {fx:?}");
            }
            assert!(m.tick(now, Some(false)).is_empty());
            assert!(m
                .tick(now + Duration::from_secs(30), Some(false))
                .is_empty());
            assert_eq!(m.status(), status);
        }
    }

    #[test]
    fn a_question_suspends_the_held_stop_and_the_answer_resumes_it() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        assert_eq!(m.held_subagents(), Some(vec!["sub1".to_string()]));

        let ask = |tool: &str| HookEvent::PreToolUse {
            tool_name: Some(tool.into()),
            subagent_id: None,
        };
        m.handle(ask("AskUserQuestion"), Some("s1"), now);
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);
        assert_eq!(m.held_subagents(), None);
        subagent(&mut m, false, "sub1", now);
        assert!(m.tick(now + 2 * DRAIN_GRACE, None).is_empty());
        assert!(m.tick(now + 4 * DRAIN_GRACE, None).is_empty());
        assert_eq!(m.status(), AgentStatus::NeedsFeedback);

        let answered = HookEvent::PostToolUse {
            tool_name: Some("AskUserQuestion".into()),
            subagent_id: None,
            detail: None,
        };
        let fx = m.handle(answered, Some("s1"), now + 4 * DRAIN_GRACE);
        assert_eq!(status_of(&fx), Some(AgentStatus::Running));
        let t = now + 5 * DRAIN_GRACE;
        assert!(m.tick(t, None).is_empty(), "the drain starts over");
        let fx = m.tick(t + DRAIN_GRACE, None);
        assert_eq!(status_of(&fx), Some(AgentStatus::Finished));
    }

    #[test]
    fn a_new_cli_session_drops_the_old_sessions_held_stop() {
        let mut m = AgentStatusMachine::new(AgentStatus::Fresh, None);
        let now = t0();
        m.handle(HookEvent::UserPromptSubmit, Some("s1"), now);
        subagent(&mut m, true, "sub1", now);
        m.handle(HookEvent::Stop, Some("s1"), now);
        assert!(m.held_subagents().is_some());

        let resumed = HookEvent::SessionStart {
            source: Some("resume".into()),
        };
        let fx = m.handle(resumed, Some("s2"), now);
        assert_eq!(fx, vec![Effect::SaveSessionId("s2".into())]);
        assert_eq!(m.status(), AgentStatus::Running);
        assert_eq!(m.held_subagents(), None);
    }
}
