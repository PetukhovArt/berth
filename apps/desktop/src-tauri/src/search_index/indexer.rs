use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde_json::Value;
use tokio::task::spawn_blocking;

use super::store;
use crate::agent_sessions::{
    claude_messages, codex_messages, session_facts, transcript_files, Harness, Role,
    TranscriptMessage,
};

const TICK: Duration = Duration::from_secs(30);
const LOCK_RETRY: Duration = Duration::from_secs(60);
const LOCK_FILE: &str = "search-index.lock";
const TICK_BYTES: u64 = 64 * 1024 * 1024;

struct Listed {
    harness: Harness,
    path: PathBuf,
    key: String,
    size: i64,
    mtime: i64,
}

struct Known {
    size: i64,
    mtime: i64,
    offset: i64,
}

enum Plan {
    Skip,
    From(i64),
    Rebuild,
}

pub async fn run() {
    if crate::ssh_remote::target().is_some() {
        return;
    }
    if let Err(error) = store::fts5_ready() {
        eprintln!("search index: {error}");
        return;
    }
    let _one_writer = writer_lock().await;
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let outcome = spawn_blocking(sweep)
            .await
            .map_err(|error| error.to_string())
            .and_then(|swept| swept);
        if let Err(error) = outcome {
            eprintln!("search index: {error}");
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

fn sweep() -> Result<(), String> {
    let mut conn = store::open_writer()?;
    match pass(&mut conn) {
        Ok(()) => Ok(()),
        Err(error) if store::is_corrupt(&error) => {
            drop(conn);
            let _ = store::remove_files();
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

fn load_known(conn: &Connection) -> rusqlite::Result<HashMap<String, Known>> {
    let mut statement = conn.prepare("SELECT path, size, mtime, offset FROM files")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            Known {
                size: row.get(1)?,
                mtime: row.get(2)?,
                offset: row.get(3)?,
            },
        ))
    })?;
    rows.collect()
}

fn plan(file: &Listed, known: Option<&Known>) -> Plan {
    match known {
        None => Plan::From(0),
        Some(known) if file.size < known.size || file.mtime < known.mtime => Plan::Rebuild,
        Some(known) if file.size > known.size => Plan::From(known.offset),
        Some(_) => Plan::Skip,
    }
}

fn forget_removed(
    conn: &mut Connection,
    listed: &[Listed],
    known: &mut HashMap<String, Known>,
) -> rusqlite::Result<()> {
    if listed.is_empty() {
        return Ok(());
    }
    let present: HashSet<&str> = listed.iter().map(|file| file.key.as_str()).collect();
    let gone: Vec<String> = known
        .keys()
        .filter(|key| !present.contains(key.as_str()))
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

fn pass(conn: &mut Connection) -> rusqlite::Result<()> {
    let listed = listing();
    let mut known = load_known(conn)?;
    forget_removed(conn, &listed, &mut known)?;
    let mut budget = TICK_BYTES;
    let mut caught_up = 0u32;
    for file in &listed {
        let from = match plan(file, known.get(&file.key)) {
            Plan::Skip => {
                caught_up += 1;
                continue;
            }
            Plan::From(offset) => Some(offset),
            Plan::Rebuild => None,
        };
        if budget == 0 {
            continue;
        }
        if index_file(conn, file, from, &mut budget)? {
            caught_up += 1;
        }
    }
    conn.execute(
        "UPDATE progress SET indexed = ?1, total = ?2",
        params![caught_up, u32::try_from(listed.len()).unwrap_or(u32::MAX)],
    )?;
    Ok(())
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

fn harness_name(harness: Harness) -> &'static str {
    match harness {
        Harness::Claude => "claude",
        Harness::Codex => "codex",
    }
}

fn messages_of(harness: Harness, line: &Value) -> Vec<TranscriptMessage> {
    let mut out = Vec::new();
    match harness {
        Harness::Claude => claude_messages(line, &mut out),
        Harness::Codex => codex_messages(line, &mut out),
    }
    out.retain(|message| message.tool.is_none() && !message.text.is_empty());
    out
}

fn index_file(
    conn: &mut Connection,
    file: &Listed,
    from: Option<i64>,
    budget: &mut u64,
) -> rusqlite::Result<bool> {
    let Ok(handle) = File::open(&file.path) else {
        return Ok(false);
    };
    let rebuild = from.is_none();
    let start = from.unwrap_or(0);
    let mut reader = BufReader::new(handle);
    if reader
        .seek(SeekFrom::Start(u64::try_from(start).unwrap_or(0)))
        .is_err()
    {
        return Ok(false);
    }
    let facts = session_facts(file.harness, &file.path);
    let tx = conn.transaction()?;
    if rebuild {
        tx.execute("DELETE FROM messages WHERE path = ?1", [&file.key])?;
    }
    let mut offset = start;
    let mut consumed = start;
    let mut done = true;
    let mut line = Vec::new();
    {
        let mut insert = tx.prepare_cached(
            "INSERT INTO messages(text, harness, session, key, role, at, path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        loop {
            if *budget == 0 {
                done = false;
                break;
            }
            line.clear();
            let read = match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            let length = i64::try_from(read).unwrap_or(i64::MAX);
            consumed += length;
            if line.last() != Some(&b'\n') {
                break;
            }
            offset += length;
            *budget = budget.saturating_sub(read as u64);
            let Ok(value) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            for message in messages_of(file.harness, &value) {
                insert.execute(params![
                    message.text,
                    harness_name(file.harness),
                    facts.id,
                    message.key,
                    role_name(message.role),
                    message.at,
                    file.key,
                ])?;
            }
        }
    }
    let size = if done { consumed } else { offset };
    tx.execute(
        "INSERT INTO files(path, harness, session, size, mtime, offset, title, cwd, branch) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(path) DO UPDATE SET harness = excluded.harness, session = excluded.session, \
         size = excluded.size, mtime = excluded.mtime, offset = excluded.offset, \
         title = excluded.title, cwd = excluded.cwd, branch = excluded.branch",
        params![
            file.key,
            harness_name(file.harness),
            facts.id,
            size,
            file.mtime,
            offset,
            facts.title,
            facts.cwd,
            facts.branch
        ],
    )?;
    tx.commit()?;
    Ok(done && size >= file.size)
}
