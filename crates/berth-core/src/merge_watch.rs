use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{Agent, AgentId, AgentStatus, EntityId, Worktree, WorktreeId};

pub const CHECK_EVERY: Duration = Duration::from_secs(5 * 60);
pub const MAX_BACKOFF: Duration = Duration::from_secs(60 * 60);
const ARCHIVED_LIMIT: usize = 500;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub archived_after_merge: VecDeque<AgentId>,
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
    SaveBeforeArchiving(Memory),
    Archive(AgentId),
    Comment(FinishedTurn),
}

struct Poll {
    next: Instant,
    backoff: Duration,
}

pub struct MergeWatch {
    memory: Memory,
    worktrees: HashMap<WorktreeId, Worktree>,
    agents: HashMap<AgentId, Agent>,
    baseline_taken: bool,
    polls: HashMap<WorktreeId, Poll>,
}

impl MergeWatch {
    pub fn new(memory: Memory) -> Self {
        Self {
            memory,
            worktrees: HashMap::new(),
            agents: HashMap::new(),
            baseline_taken: false,
            polls: HashMap::new(),
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
                self.agents.insert(agent.id.clone(), *agent);
            }
            Event::Removed(EntityId::Agent(id)) => {
                self.agents.remove(&id);
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
            let poll = self.polls.entry(id.clone()).or_insert(Poll {
                next: now,
                backoff: Duration::ZERO,
            });
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
            if answer.is_ok() {
                poll.backoff = Duration::ZERO;
                poll.next = now + CHECK_EVERY;
            } else {
                poll.backoff = (poll.backoff.max(CHECK_EVERY / 2) * 2).min(MAX_BACKOFF);
                poll.next = now + poll.backoff;
            }
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
        let mut effects = vec![Effect::SaveBeforeArchiving(self.memory.clone())];
        effects.extend(agents.into_iter().map(Effect::Archive));
        effects
    }

    fn idle_agents(&self, worktree: &WorktreeId) -> Vec<AgentId> {
        let mut ids: Vec<AgentId> = self
            .agents
            .values()
            .filter(|agent| &agent.worktree_id == worktree && !agent.archived)
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
        archived: Vec<AgentId>,
        comments: Vec<FinishedTurn>,
        lookups: usize,
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
                archived: Vec::new(),
                comments: Vec::new(),
                lookups: 0,
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
                    Effect::SaveBeforeArchiving(memory) => self.disk = Some(memory),
                    Effect::Archive(id) => {
                        self.archived.push(id.clone());
                        let agent = self.set_archived(&id, true);
                        self.send(Event::Agent(Box::new(agent)));
                    }
                    Effect::Comment(turn) => self.comments.push(turn),
                }
            }
        }

        fn set_archived(&mut self, id: &AgentId, archived: bool) -> Agent {
            let agent = self
                .daemon_agents
                .iter_mut()
                .find(|agent| &agent.id == id)
                .expect("known agent");
            agent.archived = archived;
            agent.clone()
        }

        fn unarchive(&mut self, id: &str) {
            let agent = self.set_archived(&AgentId(id.into()), false);
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
    fn a_merged_merge_request_archives_its_idle_agents_once() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![
                agent("a", "w", AgentStatus::Finished),
                agent("b", "w", AgentStatus::Fresh),
            ],
        );
        world.poll();
        assert!(world.archived.is_empty(), "an open branch archives nothing");
        world.merged("w");
        world.later(CHECK_EVERY);
        world.poll();
        assert_eq!(world.archived, ids(&["a", "b"]));
        for _ in 0..3 {
            world.later(CHECK_EVERY);
            world.poll();
        }
        assert_eq!(world.archived, ids(&["a", "b"]));
    }

    #[test]
    fn an_agent_brought_back_after_the_merge_stays_unarchived() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Finished)],
        );
        world.merged("w");
        world.poll();
        world.unarchive("a");
        world.later(CHECK_EVERY);
        world.poll();
        assert_eq!(world.archived, ids(&["a"]));
    }

    #[test]
    fn a_restart_reads_the_saved_memory_and_does_not_archive_again() {
        let mut world = FakeWorld::new(
            BOTH,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Finished)],
        );
        world.merged("w");
        world.poll();
        world.unarchive("a");
        world.restart_from_disk();
        world.poll();
        assert_eq!(world.archived, ids(&["a"]));
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
        assert!(world.archived.is_empty());
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
        assert!(world.archived.is_empty());
        assert!(world.comments.is_empty());
        assert_eq!(world.lookups, 0);
        assert_eq!(world.disk, None);
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
    fn write_back_alone_comments_but_never_archives() {
        let mut world = FakeWorld::new(
            WRITE_BACK_ONLY,
            vec![worktree("w", "feat")],
            vec![agent("a", "w", AgentStatus::Running)],
        );
        world.merged("w");
        world.status("a", AgentStatus::Finished);
        world.poll();
        assert_eq!(world.comments.len(), 1);
        assert!(world.archived.is_empty());
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
