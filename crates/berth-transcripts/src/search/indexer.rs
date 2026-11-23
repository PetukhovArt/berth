use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::transcript::{
    claude_messages, codex_messages, renamed_title, session_facts, title_line, Harness, Role,
    TranscriptMessage,
};

pub struct Listed {
    pub harness: Harness,
    pub path: PathBuf,
    pub key: String,
    pub size: i64,
    pub mtime: i64,
}

pub struct Known {
    size: i64,
    pub mtime: i64,
    pub offset: i64,
}

enum Plan {
    Skip,
    From(i64),
    Rebuild,
}

pub fn load_known(conn: &Connection) -> rusqlite::Result<HashMap<String, Known>> {
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

pub fn index_listed(
    conn: &mut Connection,
    listed: &[Listed],
    known: &HashMap<String, Known>,
    budget: &mut u64,
) -> rusqlite::Result<u32> {
    let mut caught_up = 0u32;
    for file in listed {
        let from = match plan(file, known.get(&file.key)) {
            Plan::Skip => {
                caught_up += 1;
                continue;
            }
            Plan::From(offset) => Some(offset),
            Plan::Rebuild => None,
        };
        if *budget == 0 {
            continue;
        }
        if index_file(conn, file, from, budget)? {
            caught_up += 1;
        }
    }
    Ok(caught_up)
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

fn messages_of(harness: Harness, line: &Value, record: u64) -> Vec<TranscriptMessage> {
    let mut out = Vec::new();
    match harness {
        Harness::Claude => claude_messages(line, record, false, &mut out),
        Harness::Codex => codex_messages(line, record, &mut out),
    }
    out.retain(|message| message.tool.is_none() && !message.text.is_empty());
    out
}

pub fn index_file(
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
    let mut renamed = None;
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
            let record = u64::try_from(offset).unwrap_or(0);
            offset += length;
            *budget = budget.saturating_sub(read as u64);
            let Ok(value) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            if let Some(title) = renamed_title(file.harness, &value) {
                renamed = Some(title_line(&title));
            }
            for message in messages_of(file.harness, &value, record) {
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
         title = CASE WHEN ?11 THEN excluded.title ELSE coalesce(?10, files.title, excluded.title) END, \
         cwd = excluded.cwd, branch = excluded.branch",
        params![
            file.key,
            harness_name(file.harness),
            facts.id,
            size,
            file.mtime,
            offset,
            renamed.as_ref().or(facts.title.as_ref()),
            facts.cwd,
            facts.branch,
            renamed,
            start == 0
        ],
    )?;
    tx.commit()?;
    Ok(done && size >= file.size)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::search::store;
    use crate::transcript::read_transcript_file;

    const BUDGET: u64 = 64 * 1024 * 1024;

    const CLAUDE: &str = concat!(
        r#"{"type":"user","uuid":"u1","timestamp":"2026-11-01T10:00:00Z","message":{"role":"user","content":"where do lanterns go"}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-11-01T10:00:01Z","message":{"role":"assistant","content":[{"type":"text","text":"they drift"},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}"#,
        "\n",
        r#"{"type":"user","uuid":"u2","timestamp":"2026-11-01T10:00:02Z","message":{"role":"user","content":"and then"}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-11-01T10:00:03Z","message":{"role":"assistant","content":[{"type":"text","text":"they land"}]}}"#,
        "\n",
    );

    const CODEX: &str = concat!(
        r#"{"timestamp":"2026-11-01T10:00:00.000Z","type":"session_meta","payload":{"id":"0199aaaa-bbbb-cccc-dddd-eeeeeeeeeeee","cwd":"/tmp"}}"#,
        "\n",
        r#"{"timestamp":"2026-11-01T10:00:01.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"where do lanterns go"}]}}"#,
        "\n",
        r#"{"timestamp":"2026-11-01T10:00:02.000Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"ls\"]}","call_id":"c1"}}"#,
        "\n",
        r#"{"timestamp":"2026-11-01T10:00:02.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"they drift"}]}}"#,
        "\n",
        r#"{"timestamp":"2026-11-01T10:00:03.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"and then"}]}}"#,
        "\n",
        r#"{"timestamp":"2026-11-01T10:00:04.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"they land"}]}}"#,
        "\n",
    );

    const START: &str = concat!(
        r#"{"type":"user","uuid":"u1","sessionId":"s1","cwd":"/tmp","timestamp":"2026-11-01T10:00:00Z","message":{"role":"user","content":"first prompt words"}}"#,
        "\n",
    );
    const RENAME: &str = concat!(
        r#"{"type":"custom-title","customTitle":"Renamed Later","sessionId":"s1"}"#,
        "\n",
    );
    const MORE: &str = concat!(
        r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-11-01T10:00:02Z","message":{"role":"user","content":"another line"}}"#,
        "\n",
    );

    fn memory_index() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(store::SCHEMA).unwrap();
        conn
    }

    fn index(conn: &mut Connection, harness: Harness, path: &Path, from: Option<i64>) -> String {
        let meta = fs::metadata(path).unwrap();
        let file = Listed {
            harness,
            path: path.to_path_buf(),
            key: path.to_string_lossy().into_owned(),
            size: i64::try_from(meta.len()).unwrap(),
            mtime: 1,
        };
        let mut budget = BUDGET;
        assert!(index_file(conn, &file, from, &mut budget).unwrap());
        file.key
    }

    fn indexed_keys(conn: &mut Connection, harness: Harness, path: &Path) -> Vec<String> {
        let key = index(conn, harness, path, Some(0));
        let mut statement = conn
            .prepare("SELECT key FROM messages WHERE path = ?1 ORDER BY rowid")
            .unwrap();
        let keys = statement
            .query_map([&key], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        keys
    }

    fn chat_keys(harness: Harness, path: &Path, tail: u32, reach: Option<&str>) -> Vec<String> {
        read_transcript_file(harness, path, Some(tail), reach)
            .unwrap()
            .into_iter()
            .filter(|message| message.tool.is_none())
            .map(|message| message.key)
            .collect()
    }

    #[test]
    fn index_and_chat_view_give_a_message_the_same_key_whatever_window_is_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = memory_index();
        for (harness, text) in [(Harness::Claude, CLAUDE), (Harness::Codex, CODEX)] {
            let path = dir.path().join(format!("{}.jsonl", harness_name(harness)));
            fs::write(&path, text).unwrap();
            let indexed = indexed_keys(&mut conn, harness, &path);
            assert_eq!(indexed.len(), 4, "{harness:?}");
            assert_eq!(
                chat_keys(harness, &path, 1000, None),
                indexed,
                "{harness:?}"
            );
            assert_eq!(
                chat_keys(harness, &path, 1, None).last(),
                indexed.last(),
                "{harness:?}"
            );
            let reached = chat_keys(harness, &path, 1, Some(&indexed[0]));
            assert_eq!(reached, indexed, "{harness:?}");
        }
    }

    fn title_and_offset(conn: &mut Connection, path: &Path, from: Option<i64>) -> (String, i64) {
        let key = index(conn, Harness::Claude, path, from);
        conn.query_row(
            "SELECT title, offset FROM files WHERE path = ?1",
            [&key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn a_rename_appended_later_replaces_the_title_and_survives_later_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let mut conn = memory_index();

        fs::write(&path, START).unwrap();
        let (title, offset) = title_and_offset(&mut conn, &path, Some(0));
        assert_eq!(title, "first prompt words");

        fs::write(&path, format!("{START}{RENAME}")).unwrap();
        let (title, offset) = title_and_offset(&mut conn, &path, Some(offset));
        assert_eq!(title, "Renamed Later");

        let mut grown = format!("{START}{RENAME}");
        for _ in 0..2000 {
            grown.push_str(MORE);
        }
        fs::write(&path, &grown).unwrap();
        let (title, _) = title_and_offset(&mut conn, &path, Some(offset));
        assert_eq!(title, "Renamed Later");

        let (title, _) = title_and_offset(&mut conn, &path, Some(0));
        assert_eq!(title, "Renamed Later");
        let (title, _) = title_and_offset(&mut conn, &path, None);
        assert_eq!(title, "Renamed Later");
    }
}
