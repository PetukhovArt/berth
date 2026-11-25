use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use berth_transcripts::search::store;
use berth_transcripts::search::{index_listed, load_known, Known, Listed};
use rusqlite::{params, Connection};
use tokio::runtime::Handle;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::spawn_blocking;

use super::acp::{self, AcpAgent, Daemon, Roster};
use super::db_path;
use crate::agent_sessions::transcript_files;
use crate::berth::{self, Berth};
use crate::merge_watch::Forward;

const TICK: Duration = Duration::from_secs(30);
const LOCK_RETRY: Duration = Duration::from_secs(60);
const LOCK_FILE: &str = "search-index.lock";
const TICK_BYTES: u64 = 64 * 1024 * 1024;

pub async fn run() {
    if crate::ssh_remote::target().is_some() {
        return;
    }
    if let Err(error) = store::fts5_ready() {
        eprintln!("search index: {error}");
        return;
    }
    let _one_writer = writer_lock().await;
    let berth = Arc::new(Berth::spawn());
    let (events_tx, mut events) = unbounded_channel();
    berth::berth_watch(&berth, Box::new(Forward(events_tx)));
    let daemon = Arc::new(Daemon::new(berth, Handle::current()));
    let mut roster = Roster::default();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            Some(event) = events.recv() => roster.apply(event),
            _ = tick.tick() => {
                let agents = roster.acp_agents();
                let truncated = roster.take_truncated();
                let daemon = daemon.clone();
                let outcome = spawn_blocking(move || sweep(agents, &truncated, &daemon))
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|swept| swept);
                if let Err(error) = outcome {
                    eprintln!("search index: {error}");
                }
            }
        }
    }
}

async fn writer_lock() -> File {
    loop {
        if let Some(file) = try_writer_lock() {
            return file;
        }
        tokio::time::sleep(LOCK_RETRY).await;
    }
}

fn try_writer_lock() -> Option<File> {
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

fn sweep(
    agents: Option<Vec<AcpAgent>>,
    truncated: &[String],
    daemon: &Daemon,
) -> Result<(), String> {
    let mut conn = store::open_writer(&db_path())?;
    match acp::forget(&mut conn, truncated).and_then(|()| pass(&mut conn, agents, daemon)) {
        Ok(()) => Ok(()),
        Err(error) if store::is_corrupt(&error) => {
            drop(conn);
            let _ = store::remove_files(&db_path());
            Err(error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn listing() -> Vec<Listed> {
    let mut listed: Vec<Listed> = transcript_files()
        .into_iter()
        .filter_map(|(harness, path)| {
            let meta = fs::metadata(&path).ok().filter(fs::Metadata::is_file)?;
            Some(Listed {
                harness,
                key: path.to_string_lossy().into_owned(),
                size: i64::try_from(meta.len()).unwrap_or(i64::MAX),
                mtime: meta.modified().map(millis).unwrap_or(0),
                path,
            })
        })
        .collect();
    listed.sort_by_key(|file| std::cmp::Reverse(file.mtime));
    listed
}

fn forget_removed(
    conn: &mut Connection,
    listed: &[Listed],
    agents: Option<&[AcpAgent]>,
    known: &mut HashMap<String, Known>,
) -> rusqlite::Result<()> {
    let files: HashSet<&str> = listed.iter().map(|file| file.key.as_str()).collect();
    let sessions: Option<HashSet<String>> =
        agents.map(|agents| agents.iter().map(AcpAgent::key).collect());
    let gone: Vec<String> = known
        .keys()
        .filter(|key| {
            if acp::is_key(key) {
                sessions
                    .as_ref()
                    .is_some_and(|present| !present.contains(*key))
            } else {
                !listed.is_empty() && !files.contains(key.as_str())
            }
        })
        .cloned()
        .collect();
    if gone.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for key in &gone {
        tx.execute("DELETE FROM files WHERE path = ?1", [key])?;
        known.remove(key);
    }
    tx.execute(
        "DELETE FROM messages WHERE path NOT IN (SELECT path FROM files)",
        [],
    )?;
    tx.commit()
}

fn pass(
    conn: &mut Connection,
    agents: Option<Vec<AcpAgent>>,
    daemon: &Daemon,
) -> rusqlite::Result<()> {
    let listed = listing();
    let mut known = load_known(conn)?;
    forget_removed(conn, &listed, agents.as_deref(), &mut known)?;
    let mut budget = TICK_BYTES;
    let mut caught_up = index_listed(conn, &listed, &known, &mut budget)?;
    let conversations = agents.unwrap_or_default();
    for agent in &conversations {
        if acp::index(conn, daemon, agent, known.get(&agent.key()))? {
            caught_up += 1;
        }
    }
    conn.execute(
        "UPDATE progress SET indexed = ?1, total = ?2",
        params![
            caught_up,
            u32::try_from(listed.len() + conversations.len()).unwrap_or(u32::MAX)
        ],
    )?;
    Ok(())
}
