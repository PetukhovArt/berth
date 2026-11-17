use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use berth_core::merge_watch::{
    Effect, Event, FinishedTurn, ForgeAnswer, Lookup, Memory, MergeRequestRef, MergeRequestState,
    MergeWatch, Observation, PipelineRun, Prompt, Settings, Thread, ThreadNote, WatchAnswer,
    WATCHED_LIMIT,
};
use berth_core::{AgentId, ClientRequest, Entity, SessionRef, WorktreeId};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::berth::{self, Berth, WatchEvent};
use crate::connectors::types::{CrState, PipelineState};
use crate::connectors::{forge, tracker, Connectors};
use crate::settings;
use crate::sink::Sink;

const TICK: Duration = Duration::from_secs(30);
const LOCK_RETRY: Duration = Duration::from_secs(60);
const LOCK_FILE: &str = "merge-watch.lock";
const MEMORY_FILE: &str = "merge-watch.json";
const MEMORY_TEMP: &str = "merge-watch.json.tmp";
const LIST_FILE: &str = "merge-watch-list.json";
const LIST_TEMP: &str = "merge-watch-list.json.tmp";

#[derive(Serialize)]
pub struct Watched {
    #[serde(flatten)]
    target: MergeRequestRef,
    last_wake: Option<String>,
}

struct Forward(UnboundedSender<WatchEvent>);

impl Sink<WatchEvent> for Forward {
    fn send(&self, event: WatchEvent) -> bool {
        self.0.send(event).is_ok()
    }
}

pub async fn run(connectors: Arc<Connectors>) {
    if crate::ssh_remote::target().is_some() {
        return;
    }
    let _one_per_daemon = daemon_lock().await;
    let mut watch = MergeWatch::new(load_memory());
    let berth = Berth::spawn();
    let (events_tx, mut events) = unbounded_channel();
    berth::berth_watch(&berth, Box::new(Forward(events_tx)));
    let (answers_tx, mut answers) = unbounded_channel::<(WorktreeId, ForgeAnswer)>();
    let (observed_tx, mut observed) = unbounded_channel::<(MergeRequestRef, WatchAnswer)>();
    let mut tick = tokio::time::interval(TICK);
    loop {
        let effects = tokio::select! {
            event = events.recv() => {
                let Some(event) = event else { return };
                let Some(event) = from_daemon(event) else { continue };
                watch.apply(event, read_settings())
            }
            Some((worktree, answer)) = answers.recv() => {
                watch.merge_request(&worktree, answer, Instant::now(), read_settings())
            }
            Some((target, answer)) = observed.recv() => {
                watch.watched_merge_request(&target, answer, Instant::now())
            }
            _ = tick.tick() => {
                for lookup in watch.due(Instant::now(), read_settings()) {
                    tokio::spawn(ask_forge(connectors.clone(), lookup, answers_tx.clone()));
                }
                for target in watch.watches_due(Instant::now(), &load_list()) {
                    tokio::spawn(observe(connectors.clone(), target, observed_tx.clone()));
                }
                continue;
            }
        };
        carry_out(&berth, &connectors, effects).await;
    }
}

async fn daemon_lock() -> File {
    loop {
        if let Some(file) = try_daemon_lock() {
            return file;
        }
        tokio::time::sleep(LOCK_RETRY).await;
    }
}

fn try_daemon_lock() -> Option<File> {
    let dir = berth_core::paths::runtime_dir();
    if !dir.is_dir() {
        return None;
    }
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_FILE))
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

fn read_json<T: serde::de::DeserializeOwned + Default>(file: &str) -> T {
    fs::read_to_string(berth_core::paths::data_dir().join(file))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_json<T: Serialize>(file: &str, temp: &str, value: &T) -> Result<(), String> {
    let dir = berth_core::paths::data_dir();
    let path: PathBuf = dir.join(file);
    let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let temp = dir.join(temp);
    fs::create_dir_all(&dir)
        .and_then(|()| fs::write(&temp, text))
        .and_then(|()| fs::rename(&temp, &path))
        .map_err(|e| {
            let _ = fs::remove_file(&temp);
            format!("{}: {e}", path.display())
        })
}

fn load_memory() -> Memory {
    read_json(MEMORY_FILE)
}

fn load_list() -> Vec<MergeRequestRef> {
    read_json(LIST_FILE)
}

fn listed() -> Vec<Watched> {
    let memory = load_memory();
    load_list()
        .into_iter()
        .map(|target| Watched {
            last_wake: memory.last_wake(&target).map(str::to_owned),
            target,
        })
        .collect()
}

pub async fn merge_watch_list() -> Result<Vec<Watched>, String> {
    Ok(listed())
}

pub async fn merge_watch_set(
    connector: String,
    project: String,
    number: u64,
    on: bool,
) -> Result<Vec<Watched>, String> {
    let target = MergeRequestRef {
        connector,
        project,
        number,
    };
    let mut list = load_list();
    list.retain(|entry| entry != &target);
    if on {
        list.push(target);
    }
    let excess = list.len().saturating_sub(WATCHED_LIMIT);
    list.drain(..excess);
    write_json(LIST_FILE, LIST_TEMP, &list)?;
    Ok(listed())
}

fn read_settings() -> Settings {
    let value: Value = serde_json::from_str(&settings::settings_text()).unwrap_or(Value::Null);
    let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
    Settings {
        archive_on_merge: flag("sessions.archiveOnMerge"),
        tracker_write_back: flag("connectors.trackerWriteBack"),
    }
}

fn comment_body(turn: &FinishedTurn) -> String {
    let value: Value = serde_json::from_str(&settings::settings_text()).unwrap_or(Value::Null);
    let FinishedTurn { name, branch, .. } = turn;
    match value.get("ui.language").and_then(Value::as_str) {
        Some("ru") => format!("Berth: агент «{name}» закончил работу в ветке {branch}."),
        _ => format!("Berth: Agent \"{name}\" finished on branch {branch}."),
    }
}

fn from_daemon(event: WatchEvent) -> Option<Event> {
    Some(match event {
        WatchEvent::Snapshot {
            worktrees, agents, ..
        } => Event::Snapshot { worktrees, agents },
        WatchEvent::Upserted {
            entity: Entity::Worktree(worktree),
        } => Event::Worktree(worktree),
        WatchEvent::Upserted {
            entity: Entity::Agent(agent),
        } => Event::Agent(agent),
        WatchEvent::Removed { id } => Event::Removed(id),
        WatchEvent::Status { agent, status, .. } => Event::Status { agent, status },
        _ => return None,
    })
}

async fn ask_forge(
    connectors: Arc<Connectors>,
    lookup: Lookup,
    answers: UnboundedSender<(WorktreeId, ForgeAnswer)>,
) {
    let path = lookup.path.to_string_lossy().into_owned();
    let answer = forge::forge_for_worktree(&connectors, path, lookup.branch)
        .await
        .map(|summary| {
            summary.map(|summary| match summary.state {
                CrState::Open => MergeRequestState::Open,
                CrState::Merged => MergeRequestState::Merged,
                CrState::Closed => MergeRequestState::Closed,
            })
        });
    let _ = answers.send((lookup.worktree, answer));
}

async fn observe(
    connectors: Arc<Connectors>,
    target: MergeRequestRef,
    answers: UnboundedSender<(MergeRequestRef, WatchAnswer)>,
) {
    let MergeRequestRef {
        connector,
        project,
        number,
    } = target.clone();
    let (request, pipeline, discussions) = tokio::join!(
        forge::forge_change_request(&connectors, connector.clone(), project.clone(), number),
        forge::forge_pipeline(&connectors, connector.clone(), project.clone(), number),
        forge::forge_discussions(&connectors, connector, project, number),
    );
    let answer =
        request.and_then(|request| {
            let pipeline = pipeline?;
            let discussions = discussions?;
            let summary = request.summary;
            let mark = if summary.url.contains("/merge_requests/") {
                '!'
            } else {
                '#'
            };
            Ok(Observation {
                state: match summary.state {
                    CrState::Open => MergeRequestState::Open,
                    CrState::Merged => MergeRequestState::Merged,
                    CrState::Closed => MergeRequestState::Closed,
                },
                label: format!("{mark}{}", summary.number),
                source_branch: summary.source_branch,
                target_branch: summary.target_branch,
                conflicts: request.mergeable == Some(false),
                pipeline: pipeline.map(|run| PipelineRun {
                    failed: run.status == PipelineState::Failed,
                    failed_jobs: run
                        .jobs
                        .into_iter()
                        .filter(|job| job.status == "failed")
                        .map(|job| job.name)
                        .collect(),
                    id: run.id,
                }),
                threads: discussions
                    .into_iter()
                    .map(|discussion| Thread {
                        opener: discussion.notes.into_iter().find(|note| !note.system).map(
                            |note| ThreadNote {
                                author: if note.author.name.is_empty() {
                                    note.author.username
                                } else {
                                    note.author.name
                                },
                                body: note.body,
                            },
                        ),
                        line: discussion.position.as_ref().and_then(|at| at.new_line),
                        path: discussion.position.map(|at| at.path),
                        id: discussion.id,
                        resolved: discussion.resolved,
                    })
                    .collect(),
            })
        });
    let _ = answers.send((target, answer));
}

async fn type_prompt(berth: &Berth, prompt: Prompt) {
    let Prompt { agent, acp, text } = prompt;
    if !acp {
        let session = SessionRef::Agent(agent);
        berth::berth_input(berth, session.clone(), format!("\x1b[200~{text}\x1b[201~"));
        berth::berth_input(berth, session, "\r".into());
        return;
    }
    let request = ClientRequest::AcpPrompt {
        req_id: 0,
        agent,
        text,
    };
    let result = match serde_json::to_value(request) {
        Ok(request) => berth::berth_call(berth, request).await.map(drop),
        Err(error) => Err(error.to_string()),
    };
    if let Err(error) = result {
        eprintln!("berth: watch a merge request: {error}");
    }
}

async fn carry_out(berth: &Berth, connectors: &Arc<Connectors>, effects: Vec<Effect>) {
    for effect in effects {
        match effect {
            Effect::Save(memory) => {
                if let Err(error) = write_json(MEMORY_FILE, MEMORY_TEMP, &memory) {
                    eprintln!("berth: merge-request watcher could not save {error}");
                }
            }
            Effect::Archive(id) => archive(berth, id).await,
            Effect::Prompt(prompt) => type_prompt(berth, prompt).await,
            Effect::Comment(turn) => {
                tokio::spawn(post_comment(connectors.clone(), turn));
            }
        }
    }
}

async fn archive(berth: &Berth, id: AgentId) {
    let request = match serde_json::to_value(ClientRequest::ArchiveAgent { req_id: 0, id }) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("berth: archive on merge: {error}");
            return;
        }
    };
    if let Err(error) = berth::berth_call(berth, request).await {
        eprintln!("berth: archive on merge: {error}");
    }
}

async fn post_comment(connectors: Arc<Connectors>, turn: FinishedTurn) {
    let body = comment_body(&turn);
    let path = turn.path.to_string_lossy().into_owned();
    if let Err(error) = tracker::tracker_comment(&connectors, path, turn.branch, body).await {
        eprintln!("berth: tracker write-back: {error}");
    }
}
