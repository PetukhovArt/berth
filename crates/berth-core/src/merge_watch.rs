use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{Agent, AgentId, AgentStatus, Backend, EntityId, Worktree, WorktreeId};

pub const CHECK_EVERY: Duration = Duration::from_secs(5 * 60);
pub const WATCH_EVERY: Duration = Duration::from_secs(60);
pub const MAX_BACKOFF: Duration = Duration::from_secs(60 * 60);
pub const WATCHED_LIMIT: usize = 50;
const ARCHIVED_LIMIT: usize = 500;
const SNIPPET_LIMIT: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub archived_after_merge: VecDeque<AgentId>,
    #[serde(default)]
    pub watched: BTreeMap<String, Seen>,
    #[serde(default)]
    pub queued: BTreeMap<AgentId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MergeRequestRef {
    pub connector: String,
    pub project: String,
    pub number: u64,
}

impl MergeRequestRef {
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.connector, self.project, self.number)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub failed_pipeline: Option<String>,
    #[serde(default)]
    pub threads: BTreeSet<String>,
    #[serde(default)]
    pub conflicts: bool,
    #[serde(default)]
    pub last_wake: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub state: MergeRequestState,
    pub label: String,
    pub source_branch: String,
    pub target_branch: String,
    pub conflicts: bool,
    pub pipeline: Option<PipelineRun>,
    pub threads: Vec<Thread>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRun {
    pub id: String,
    pub failed: bool,
    pub failed_jobs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub resolved: bool,
    pub opener: Option<ThreadNote>,
    pub path: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadNote {
    pub author: String,
    pub body: String,
}

pub type WatchAnswer = Result<Observation, String>;

impl Memory {
    pub fn last_wake(&self, target: &MergeRequestRef) -> Option<&str> {
        self.watched.get(&target.key())?.last_wake.as_deref()
    }
}

fn open_threads(observed: &Observation) -> impl Iterator<Item = &Thread> {
    observed
        .threads
        .iter()
        .filter(|thread| !thread.resolved && thread.opener.is_some())
}

fn wakes(observed: &Observation, before: &Seen) -> Vec<String> {
    let label = &observed.label;
    let mut found = Vec::new();
    if let Some(run) = observed.pipeline.as_ref().filter(|run| run.failed) {
        if before.failed_pipeline.as_ref() != Some(&run.id) {
            let jobs = if run.failed_jobs.is_empty() {
                String::new()
            } else {
                format!(" in {}", run.failed_jobs.join(", "))
            };
            found.push(format!(
                "Pipeline #{} of {label} failed{jobs}. Read its log, fix the cause and push.",
                run.id
            ));
        }
    }
    for thread in open_threads(observed) {
        let Some(note) = thread.opener.as_ref() else {
            continue;
        };
        if before.threads.contains(&thread.id) {
            continue;
        }
        let place = match (&thread.path, thread.line) {
            (Some(path), Some(line)) if line > 0 => format!(" on {path}:{line}"),
            (Some(path), _) => format!(" on {path}"),
            (None, _) => String::new(),
        };
        found.push(format!(
            "{} opened a thread{place} in {label}: \"{}\". Answer it or fix it and push.",
            note.author,
            snippet(&note.body)
        ));
    }
    if observed.conflicts && !before.conflicts {
        let target = &observed.target_branch;
        found.push(format!(
            "{label} now conflicts with {target}. Rebase on {target}, resolve the conflicts and push."
        ));
    }
    found
}

fn snippet(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= SNIPPET_LIMIT {
        return flat;
    }
    let cut: String = flat.chars().take(SNIPPET_LIMIT).collect();
    format!("{}…", cut.trim_end())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Settings {
    pub archive_on_merge: bool,
    pub tracker_write_back: bool,
}

#[derive(Debug, Clone)]
pub enum Event {
    Snapshot {
        worktrees: Vec<Worktree>,
        agents: Vec<Agent>,
    },
    Worktree(Worktree),
    Agent(Box<Agent>),
    Removed(EntityId),
    Status {
        agent: AgentId,
        status: AgentStatus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeRequestState {
    Open,
    Merged,
    Closed,
}

pub type ForgeAnswer = Result<Option<MergeRequestState>, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    pub worktree: WorktreeId,
    pub path: PathBuf,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedTurn {
    pub agent: AgentId,
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Save(Memory),
    End(AgentId),
    Comment(FinishedTurn),
    Prompt(Prompt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub agent: AgentId,
    pub acp: bool,
    pub text: String,
}

struct Poll {
    next: Instant,
    backoff: Duration,
}

impl Poll {
    fn due(now: Instant) -> Self {
        Poll {
            next: now,
            backoff: Duration::ZERO,
        }
    }

    fn answered(&mut self, ok: bool, now: Instant, every: Duration) {
        if ok {
            self.backoff = Duration::ZERO;
            self.next = now + every;
        } else {
            self.backoff = (self.backoff.max(every / 2) * 2).min(MAX_BACKOFF);
            self.next = now + self.backoff;
        }
    }
}

pub struct MergeWatch {
    memory: Memory,
    worktrees: HashMap<WorktreeId, Worktree>,
    agents: HashMap<AgentId, Agent>,
    baseline_taken: bool,
    polls: HashMap<WorktreeId, Poll>,
    watching: Vec<MergeRequestRef>,
    watch_polls: HashMap<MergeRequestRef, Poll>,
}

impl MergeWatch {
    pub fn new(memory: Memory) -> Self {
        Self {
            memory,
            worktrees: HashMap::new(),
            agents: HashMap::new(),
            baseline_taken: false,
            polls: HashMap::new(),
            watching: Vec::new(),
            watch_polls: HashMap::new(),
        }
    }

    pub fn apply(&mut self, event: Event, settings: Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        match event {
            Event::Snapshot { worktrees, agents } => {
                self.worktrees = worktrees
                    .into_iter()
                    .map(|worktree| (worktree.id.clone(), worktree))
                    .collect();
                let previous = std::mem::take(&mut self.agents);
                for agent in agents {
                    if self.baseline_taken {
                        if let Some(before) = previous.get(&agent.id) {
                            effects.extend(self.finished_turn(before.status, &agent, settings));
                        }
                    }
                    effects.extend(self.release(&agent));
                    self.agents.insert(agent.id.clone(), agent);
                }
                let worktrees = &self.worktrees;
                self.polls.retain(|id, _| worktrees.contains_key(id));
                self.baseline_taken = true;
            }
            Event::Worktree(worktree) => {
                self.worktrees.insert(worktree.id.clone(), worktree);
            }
            Event::Agent(agent) => {
                if let Some(before) = self.agents.get(&agent.id) {
                    effects.extend(self.finished_turn(before.status, &agent, settings));
                }
                effects.extend(self.release(&agent));
                self.agents.insert(agent.id.clone(), *agent);
            }
            Event::Removed(EntityId::Agent(id)) => {
                self.agents.remove(&id);
                self.memory.queued.remove(&id);
            }
            Event::Removed(EntityId::Worktree(id)) => {
                self.worktrees.remove(&id);
                self.polls.remove(&id);
            }
            Event::Removed(_) => {}
            Event::Status { agent, status } => {
                if let Some(mut updated) = self.agents.get(&agent).cloned() {
                    let before = updated.status;
                    updated.status = status;
                    effects.extend(self.finished_turn(before, &updated, settings));
                    effects.extend(self.release(&updated));
                    self.agents.insert(agent, updated);
                }
            }
        }
        effects
    }

    pub fn due(&mut self, now: Instant, settings: Settings) -> Vec<Lookup> {
        if !settings.archive_on_merge {
            self.polls.clear();
            return Vec::new();
        }
        let candidates: Vec<WorktreeId> = self
            .worktrees
            .values()
            .filter(|worktree| !worktree.is_main && !worktree.branch.is_empty())
            .filter(|worktree| !self.idle_agents(&worktree.id).is_empty())
            .map(|worktree| worktree.id.clone())
            .collect();
        let mut lookups = Vec::new();
        for id in candidates {
            let poll = self.polls.entry(id.clone()).or_insert(Poll::due(now));
            if poll.next > now {
                continue;
            }
            poll.next = now + CHECK_EVERY;
            let worktree = &self.worktrees[&id];
            lookups.push(Lookup {
                worktree: id,
                path: worktree.path.clone(),
                branch: worktree.branch.clone(),
            });
        }
        lookups
    }

    pub fn merge_request(
        &mut self,
        worktree: &WorktreeId,
        answer: ForgeAnswer,
        now: Instant,
        settings: Settings,
    ) -> Vec<Effect> {
        if let Some(poll) = self.polls.get_mut(worktree) {
            poll.answered(answer.is_ok(), now, CHECK_EVERY);
        }
        if !settings.archive_on_merge || answer != Ok(Some(MergeRequestState::Merged)) {
            return Vec::new();
        }
        let agents = self.idle_agents(worktree);
        if agents.is_empty() {
            return Vec::new();
        }
        let archived = &mut self.memory.archived_after_merge;
        archived.extend(agents.iter().cloned());
        while archived.len() > ARCHIVED_LIMIT {
            archived.pop_front();
        }
        let mut effects = vec![Effect::Save(self.memory.clone())];
        effects.extend(agents.into_iter().map(Effect::End));
        effects
    }

    pub fn watches_due(
        &mut self,
        now: Instant,
        watched: &[MergeRequestRef],
    ) -> Vec<MergeRequestRef> {
        self.watching = watched.to_vec();
        let keys: BTreeSet<String> = watched.iter().map(MergeRequestRef::key).collect();
        self.memory.watched.retain(|key, _| keys.contains(key));
        self.watch_polls
            .retain(|target, _| watched.contains(target));
        let mut due = Vec::new();
        for target in watched {
            let poll = self
                .watch_polls
                .entry(target.clone())
                .or_insert(Poll::due(now));
            if poll.next > now {
                continue;
            }
            poll.next = now + WATCH_EVERY;
            due.push(target.clone());
        }
        due
    }

    pub fn watched_merge_request(
        &mut self,
        target: &MergeRequestRef,
        answer: WatchAnswer,
        now: Instant,
    ) -> Vec<Effect> {
        if !self.watching.contains(target) {
            return Vec::new();
        }
        if let Some(poll) = self.watch_polls.get_mut(target) {
            match &answer {
                Ok(seen) if seen.state != MergeRequestState::Open => {
                    poll.answered(true, now, MAX_BACKOFF)
                }
                other => poll.answered(other.is_ok(), now, WATCH_EVERY),
            }
        }
        let Ok(observed) = answer else {
            return Vec::new();
        };
        if observed.state != MergeRequestState::Open {
            return Vec::new();
        }
        let key = target.key();
        let mut next = Seen {
            failed_pipeline: observed
                .pipeline
                .as_ref()
                .filter(|run| run.failed)
                .map(|run| run.id.clone()),
            threads: open_threads(&observed)
                .map(|thread| thread.id.clone())
                .collect(),
            conflicts: observed.conflicts,
            last_wake: None,
        };
        let Some(before) = self.memory.watched.get(&key) else {
            self.memory.watched.insert(key, next);
            return vec![Effect::Save(self.memory.clone())];
        };
        next.last_wake = before.last_wake.clone();
        let wakes = wakes(&observed, before);
        if wakes.is_empty() {
            if &next == before {
                return Vec::new();
            }
            self.memory.watched.insert(key, next);
            return vec![Effect::Save(self.memory.clone())];
        }
        let Some(agent) = self.agent_on(&observed.source_branch) else {
            return Vec::new();
        };
        let (agent, running, acp) = (
            agent.id.clone(),
            agent.status == AgentStatus::Running,
            agent.backend == Backend::Chat,
        );
        let text = wakes.join(" ");
        next.last_wake = Some(text.clone());
        self.memory.watched.insert(key, next);
        if running {
            let queued = self.memory.queued.entry(agent).or_default();
            if !queued.is_empty() {
                queued.push(' ');
            }
            queued.push_str(&text);
            return vec![Effect::Save(self.memory.clone())];
        }
        let prompt = Prompt { agent, acp, text };
        vec![Effect::Save(self.memory.clone()), Effect::Prompt(prompt)]
    }

    fn agent_on(&self, branch: &str) -> Option<&Agent> {
        let worktrees: BTreeSet<&WorktreeId> = self
            .worktrees
            .values()
            .filter(|worktree| worktree.branch == branch)
            .map(|worktree| &worktree.id)
            .collect();
        self.agents
            .values()
            .filter(|agent| worktrees.contains(&agent.worktree_id))
            .filter(|agent| !agent.archived && agent.alive)
            .max_by_key(|agent| agent.status_changed_at)
    }

    fn release(&mut self, agent: &Agent) -> Vec<Effect> {
        if agent.status != AgentStatus::Finished || agent.archived || !agent.alive {
            return Vec::new();
        }
        let Some(text) = self.memory.queued.remove(&agent.id) else {
            return Vec::new();
        };
        let prompt = Prompt {
            agent: agent.id.clone(),
            acp: agent.backend == Backend::Chat,
            text,
        };
        vec![Effect::Save(self.memory.clone()), Effect::Prompt(prompt)]
    }

    fn idle_agents(&self, worktree: &WorktreeId) -> Vec<AgentId> {
        let mut ids: Vec<AgentId> = self
            .agents
            .values()
            .filter(|agent| &agent.worktree_id == worktree && !agent.archived && !agent.is_ended())
            .filter(|agent| {
                !matches!(
                    agent.status,
                    AgentStatus::Running | AgentStatus::NeedsFeedback
                )
            })
            .filter(|agent| !self.memory.archived_after_merge.contains(&agent.id))
            .map(|agent| agent.id.clone())
            .collect();
        ids.sort();
        ids
    }

    fn finished_turn(
        &self,
        before: AgentStatus,
        agent: &Agent,
        settings: Settings,
    ) -> Option<Effect> {
        if !settings.tracker_write_back
            || before != AgentStatus::Running
            || agent.status != AgentStatus::Finished
        {
            return None;
        }
        let worktree = self.worktrees.get(&agent.worktree_id)?;
        if worktree.branch.is_empty() {
            return None;
        }
        Some(Effect::Comment(FinishedTurn {
            agent: agent.id.clone(),
            name: agent.name.clone(),
            path: worktree.path.clone(),
            branch: worktree.branch.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectId;

    const BOTH: Settings = Settings {
        archive_on_merge: true,
        tracker_write_back: true,
    };

    const WRITE_BACK_ONLY: Settings = Settings {
        archive_on_merge: false,
        tracker_write_back: true,
    };

    fn worktree(id: &str, branch: &str) -> Worktree {
        Worktree {
            id: WorktreeId(id.into()),
            project_id: ProjectId("p".into()),
            path: PathBuf::from(format!("/repo-worktrees/{id}")),
            branch: branch.into(),
            is_main: false,
            sort_order: 0,
        }
    }

    fn agent(id: &str, worktree: &str, status: AgentStatus) -> Agent {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "worktree_id": worktree,
            "name": format!("agent {id}"),
            "status": status,
            "archived": false,
            "sort_order": 0,
            "alive": true,
        }))
        .expect("agent")
    }

    struct FakeWorld {
        watch: MergeWatch,
        settings: Settings,
        now: Instant,
        disk: Option<Memory>,
        forge: HashMap<WorktreeId, ForgeAnswer>,
        daemon_worktrees: Vec<Worktree>,
        daemon_agents: Vec<Agent>,
        ended: Vec<AgentId>,
        comments: Vec<FinishedTurn>,
        lookups: usize,
        watched: Vec<MergeRequestRef>,
        observed: Option<Observation>,
        prompts: Vec<Prompt>,
    }

    impl FakeWorld {
        fn new(settings: Settings, worktrees: Vec<Worktree>, agents: Vec<Agent>) -> Self {
            let mut world = Self {
                watch: MergeWatch::new(Memory::default()),
                settings,
                now: Instant::now(),
                disk: None,
                forge: HashMap::new(),
                daemon_worktrees: worktrees,
                daemon_agents: agents,
                ended: Vec::new(),
                comments: Vec::new(),
                lookups: 0,
                watched: Vec::new(),
                observed: None,
                prompts: Vec::new(),
            };
            world.connect();
            world
        }

        fn connect(&mut self) {
            let snapshot = Event::Snapshot {
                worktrees: self.daemon_worktrees.clone(),
                agents: self.daemon_agents.clone(),
            };
            self.send(snapshot);
        }

        fn restart_from_disk(&mut self) {
            self.watch = MergeWatch::new(self.disk.clone().unwrap_or_default());
            self.connect();
        }

        fn send(&mut self, event: Event) {
            let effects = self.watch.apply(event, self.settings);
            self.carry_out(effects);
        }

        fn carry_out(&mut self, effects: Vec<Effect>) {
            for effect in effects {
                match effect {
                    Effect::Save(memory) => self.disk = Some(memory),
                    Effect::Prompt(prompt) => self.prompts.push(prompt),
                    Effect::End(id) => {
                        self.ended.push(id.clone());
                        let agent = self.set_ended(&id, true);
                        self.send(Event::Agent(Box::new(agent)));
                    }
                    Effect::Comment(turn) => self.comments.push(turn),
                }
            }
        }

        fn set_ended(&mut self, id: &AgentId, ended: bool) -> Agent {
            let agent = self
                .daemon_agents
                .iter_mut()
                .find(|agent| &agent.id == id)
                .expect("known agent");
            agent.ended_at = i64::from(ended);
            agent.clone()
        }

        fn reopen(&mut self, id: &str) {
            let agent = self.set_ended(&AgentId(id.into()), false);
            self.send(Event::Agent(Box::new(agent)));
        }

        fn status(&mut self, id: &str, status: AgentStatus) {
            for agent in &mut self.daemon_agents {
                if agent.id.as_str() == id {
                    agent.status = status;
                }
            }
            self.send(Event::Status {
                agent: AgentId(id.into()),
                status,
            });
        }

        fn poll(&mut self) {
            for lookup in self.watch.due(self.now, self.settings) {
                self.lookups += 1;
                let answer = self
                    .forge
                    .get(&lookup.worktree)
                    .cloned()
                    .unwrap_or(Ok(None));
                let effects =
                    self.watch
                        .merge_request(&lookup.worktree, answer, self.now, self.settings);
                self.carry_out(effects);
            }
            for target in self.watch.watches_due(self.now, &self.watched) {
                let answer = self.observed.clone().ok_or_else(|| "offline".to_string());
                let effects = self.watch.watched_merge_request(&target, answer, self.now);
                self.carry_out(effects);
            }
        }

        fn later(&mut self, by: Duration) {
            self.now += by;
        }

        fn merged(&mut self, worktree: &str) {
            self.forge.insert(
                WorktreeId(worktree.into()),
                Ok(Some(MergeRequestState::Merged)),
            );
        }
    }

    fn ids(list: &[&str]) -> Vec<AgentId> {
        list.iter().map(|id| AgentId((*id).into())).collect()
    }

    #[test]
    fn a_merged_merge_request_ends_its_idle_agents_once() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![
                agent("a", "w", AgentStatus::Finished),
                agent("b", "w", AgentStatus::Fresh),
                Agent {
                    ended_at: 1,
                    ..agent("c", "w", AgentStatus::Finished)
                },
            ],
        );
        world.poll();
        assert!(world.ended.is_empty(), "an open branch ends nothing");
        world.merged("w");
        world.later(CHECK_EVERY);
        world.poll();
        assert_eq!(world.ended, ids(&["a", "b"]));
        for _ in 0..3 {
            world.later(CHECK_EVERY);
            world.poll();
        }
        assert_eq!(world.ended, ids(&["a", "b"]));
    }

    #[test]
    fn an_agent_brought_back_after_the_merge_stays_open() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Finished)],
        );
        world.merged("w");
        world.poll();
        world.reopen("a");
        world.later(CHECK_EVERY);
        world.poll();
        assert_eq!(world.ended, ids(&["a"]));
    }

    #[test]
    fn a_restart_reads_the_saved_memory_and_does_not_end_again() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Finished)],
        );
        world.merged("w");
        world.poll();
        world.reopen("a");
        world.restart_from_disk();
        world.poll();
        assert_eq!(world.ended, ids(&["a"]));
    }

    #[test]
    fn busy_agents_and_the_main_worktree_are_left_alone() {
        let mut main = worktree("m", "main");
        main.is_main = true;
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat"), main],
            vec![
                agent("a", "w", AgentStatus::Running),
                agent("b", "w", AgentStatus::NeedsFeedback),
                agent("c", "m", AgentStatus::Finished),
            ],
        );
        world.merged("w");
        world.merged("m");
        world.poll();
        assert!(world.ended.is_empty());
        assert_eq!(world.lookups, 0);
    }

    #[test]
    fn with_both_settings_off_nothing_happens() {
        let mut world = FakeWorld::new(
            Settings::default(),
            vec![worktree("w", "feat")],
            vec![
                agent("a", "w", AgentStatus::Finished),
                agent("b", "w", AgentStatus::Running),
            ],
        );
        world.merged("w");
        world.poll();
        world.status("b", AgentStatus::Finished);
        assert!(world.ended.is_empty());
        assert!(world.comments.is_empty());
        assert_eq!(world.lookups, 0);
        assert_eq!(world.disk, None);
    }

    fn watched_mr() -> MergeRequestRef {
        MergeRequestRef {
            connector: "gitlab".into(),
            project: "group/app".into(),
            number: 7,
        }
    }

    fn open_mr() -> Observation {
        Observation {
            state: MergeRequestState::Open,
            label: "!7".into(),
            source_branch: "feat".into(),
            target_branch: "main".into(),
            conflicts: false,
            pipeline: Some(PipelineRun {
                id: "100".into(),
                failed: false,
                failed_jobs: Vec::new(),
            }),
            threads: Vec::new(),
        }
    }

    fn failed_pipeline(id: &str) -> Option<PipelineRun> {
        Some(PipelineRun {
            id: id.into(),
            failed: true,
            failed_jobs: vec!["test".into()],
        })
    }

    fn watching(status: AgentStatus) -> FakeWorld {
        let mut world = FakeWorld::new(
            Settings::default(),
            vec![worktree("w", "feat")],
            vec![agent("a", "w", status)],
        );
        world.watched = vec![watched_mr()];
        world.observed = Some(open_mr());
        world.poll();
        world
    }

    fn texts(world: &FakeWorld) -> Vec<&str> {
        world
            .prompts
            .iter()
            .map(|prompt| prompt.text.as_str())
            .collect()
    }

    const PIPELINE_101: &str =
        "Pipeline #101 of !7 failed in test. Read its log, fix the cause and push.";

    #[test]
    fn a_failed_pipeline_wakes_the_agent_once() {
        let mut world = watching(AgentStatus::Finished);
        assert!(world.prompts.is_empty(), "turning a watch on is a baseline");
        world.observed.as_mut().unwrap().pipeline = failed_pipeline("101");
        world.later(WATCH_EVERY);
        world.poll();
        world.later(WATCH_EVERY);
        world.poll();
        assert_eq!(texts(&world), [PIPELINE_101]);
        assert_eq!(world.prompts[0].agent, AgentId("a".into()));
    }

    #[test]
    fn a_new_thread_wakes_the_agent_once() {
        let mut world = watching(AgentStatus::Finished);
        world.observed.as_mut().unwrap().threads = vec![
            Thread {
                id: "t1".into(),
                resolved: false,
                opener: Some(ThreadNote {
                    author: "Ann".into(),
                    body: "Please   rename\n this".into(),
                }),
                path: Some("src/a.rs".into()),
                line: Some(12),
            },
            Thread {
                id: "t2".into(),
                resolved: true,
                opener: Some(ThreadNote {
                    author: "Bob".into(),
                    body: "done".into(),
                }),
                path: None,
                line: None,
            },
        ];
        world.later(WATCH_EVERY);
        world.poll();
        world.later(WATCH_EVERY);
        world.poll();
        assert_eq!(
            texts(&world),
            ["Ann opened a thread on src/a.rs:12 in !7: \"Please rename this\". Answer it or fix it and push."]
        );
    }

    #[test]
    fn with_the_watch_off_nothing_is_typed() {
        let mut world = watching(AgentStatus::Finished);
        world.watched.clear();
        world.observed.as_mut().unwrap().pipeline = failed_pipeline("101");
        world.observed.as_mut().unwrap().conflicts = true;
        world.later(WATCH_EVERY);
        world.poll();
        assert!(world.prompts.is_empty());
    }

    #[test]
    fn a_restart_from_saved_state_does_not_wake_again() {
        let mut world = watching(AgentStatus::Finished);
        world.observed.as_mut().unwrap().pipeline = failed_pipeline("101");
        world.later(WATCH_EVERY);
        world.poll();
        world.restart_from_disk();
        world.poll();
        assert_eq!(texts(&world), [PIPELINE_101]);
    }

    #[test]
    fn a_failure_while_the_watcher_was_down_wakes_after_restart() {
        let mut world = watching(AgentStatus::Finished);
        world.observed.as_mut().unwrap().pipeline = failed_pipeline("101");
        world.restart_from_disk();
        world.poll();
        assert_eq!(texts(&world), [PIPELINE_101]);
    }

    #[test]
    fn a_busy_agent_is_woken_when_its_turn_finishes() {
        let mut world = watching(AgentStatus::Running);
        world.observed.as_mut().unwrap().pipeline = failed_pipeline("101");
        world.later(WATCH_EVERY);
        world.poll();
        assert!(world.prompts.is_empty(), "no typing into a running turn");
        assert_eq!(
            world.disk.as_ref().unwrap().last_wake(&watched_mr()),
            Some(PIPELINE_101)
        );
        world.restart_from_disk();
        world.status("a", AgentStatus::Finished);
        world.status("a", AgentStatus::Running);
        world.status("a", AgentStatus::Finished);
        assert_eq!(texts(&world), [PIPELINE_101]);
    }

    #[test]
    fn a_finished_turn_posts_one_comment() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat/PROJ-1")],
            vec![agent("a", "w", AgentStatus::Running)],
        );
        world.status("a", AgentStatus::Finished);
        world.status("a", AgentStatus::Finished);
        let upsert = world.daemon_agents[0].clone();
        world.send(Event::Agent(Box::new(upsert)));
        assert_eq!(world.comments.len(), 1);
        assert_eq!(world.comments[0].branch, "feat/PROJ-1");
        assert_eq!(world.comments[0].name, "agent a");
    }

    #[test]
    fn write_back_alone_comments_but_never_ends() {
        let mut world = FakeWorld::new(
            WRITE_BACK_ONLY,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Running)],
        );
        world.merged("w");
        world.status("a", AgentStatus::Finished);
        world.poll();
        assert_eq!(world.comments.len(), 1);
        assert!(world.ended.is_empty());
    }

    #[test]
    fn agents_already_finished_at_the_first_snapshot_get_no_comment() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Finished)],
        );
        world.restart_from_disk();
        assert!(world.comments.is_empty());
    }

    #[test]
    fn a_turn_that_finished_while_disconnected_is_reported_on_reconnect() {
        let mut world = FakeWorld::new(
            WRITE_BACK_ONLY,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Running)],
        );
        world.daemon_agents[0].status = AgentStatus::Finished;
        world.connect();
        world.connect();
        assert_eq!(world.comments.len(), 1);
    }

    #[test]
    fn failed_lookups_back_off_and_a_lookup_in_flight_is_not_repeated() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Finished)],
        );
        let id = WorktreeId("w".into());
        assert_eq!(world.watch.due(world.now, BOTH).len(), 1);
        assert!(world.watch.due(world.now, BOTH).is_empty());
        world
            .watch
            .merge_request(&id, Err("offline".into()), world.now, BOTH);
        world.later(CHECK_EVERY);
        assert_eq!(world.watch.due(world.now, BOTH).len(), 1);
        world
            .watch
            .merge_request(&id, Err("offline".into()), world.now, BOTH);
        world.later(CHECK_EVERY);
        assert!(world.watch.due(world.now, BOTH).is_empty());
        world.later(CHECK_EVERY);
        assert_eq!(world.watch.due(world.now, BOTH).len(), 1);
    }
}
