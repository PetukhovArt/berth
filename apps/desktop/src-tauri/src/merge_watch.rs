use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use berth_core::merge_watch::{
    Effect, Event, FinishedTurn, ForgeAnswer, Lookup, Memory, MergeRequestState, MergeWatch,
    Settings,
};
use berth_core::{AgentId, ClientRequest, Entity, WorktreeId};
use serde_json::Value;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::berth::{self, Berth, WatchEvent};
use crate::connectors::types::CrState;
use crate::connectors::{forge, tracker, Connectors};
use crate::settings;
use crate::sink::Sink;

const TICK: Duration = Duration::from_secs(30);
const LOCK_RETRY: Duration = Duration::from_secs(60);
const LOCK_FILE: &str = "merge-watch.lock";
const MEMORY_FILE: &str = "merge-watch.json";
const MEMORY_TEMP: &str = "merge-watch.json.tmp";

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
            _ = tick.tick() => {
                for lookup in watch.due(Instant::now(), read_settings()) {
                    tokio::spawn(ask_forge(connectors.clone(), lookup, answers_tx.clone()));
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

fn memory_path() -> PathBuf {
    berth_core::paths::data_dir().join(MEMORY_FILE)
}

fn load_memory() -> Memory {
    fs::read_to_string(memory_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_memory(memory: &Memory) -> Result<(), String> {
    let dir = berth_core::paths::data_dir();
    let text = serde_json::to_string(memory).map_err(|e| e.to_string())?;
    let temp = dir.join(MEMORY_TEMP);
    fs::write(&temp, text)
        .and_then(|()| fs::rename(&temp, dir.join(MEMORY_FILE)))
        .map_err(|e| {
            let _ = fs::remove_file(&temp);
            format!("{}: {e}", memory_path().display())
        })
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

async fn carry_out(berth: &Berth, connectors: &Arc<Connectors>, effects: Vec<Effect>) {
    for effect in effects {
        match effect {
            Effect::SaveBeforeArchiving(memory) => {
                if let Err(error) = save_memory(&memory) {
                    eprintln!("berth: archive on merge could not save {error}");
                }
            }
            Effect::Archive(id) => archive(berth, id).await,
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
