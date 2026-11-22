use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::Serialize;
use tokio::task::spawn_blocking;

use super::store;
use crate::agent_sessions::Role;

const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;
const TERM_CHARS: usize = 3;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum HitHarness {
    Claude,
    Codex,
    Acp,
}

#[derive(Debug, Serialize)]
pub struct MessageHit {
    harness: HitHarness,
    session: String,
    key: String,
    role: Role,
    at: Option<String>,
    snippet: String,
    title: Option<String>,
    cwd: Option<String>,
    branch: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct SearchReply {
    hits: Vec<MessageHit>,
    indexed: u32,
    total: u32,
}

struct Terms {
    fts: String,
    like: Vec<String>,
}

fn terms(query: &str) -> Option<Terms> {
    let mut quoted = Vec::new();
    let mut like = Vec::new();
    for word in query.split_whitespace() {
        if word.chars().count() >= TERM_CHARS {
            quoted.push(format!("\"{}\"", word.replace('"', "\"\"")));
        } else {
            let escaped = word
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            like.push(format!("%{escaped}%"));
        }
    }
    if quoted.is_empty() {
        return None;
    }
    Some(Terms {
        fts: quoted.join(" AND "),
        like,
    })
}

fn harness_of(name: &str) -> Option<HitHarness> {
    match name {
        "claude" => Some(HitHarness::Claude),
        "codex" => Some(HitHarness::Codex),
        "acp" => Some(HitHarness::Acp),
        _ => None,
    }
}

fn role_of(name: &str) -> Role {
    if name == "user" {
        Role::User
    } else {
        Role::Assistant
    }
}

fn progress(conn: &Connection) -> rusqlite::Result<(u32, u32)> {
    conn.query_row("SELECT indexed, total FROM progress", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })
}

fn hits(conn: &Connection, terms: &Terms, limit: u32) -> rusqlite::Result<Vec<MessageHit>> {
    let mut newest = String::from("SELECT rowid FROM messages WHERE messages MATCH ?");
    let mut values = vec![Value::Text(terms.fts.clone())];
    for pattern in &terms.like {
        newest.push_str(" AND text LIKE ? ESCAPE '\\'");
        values.push(Value::Text(pattern.clone()));
    }
    newest.push_str(" ORDER BY at DESC LIMIT ?");
    values.push(Value::Integer(i64::from(limit)));
    values.push(Value::Text(terms.fts.clone()));
    let sql = format!(
        "SELECT messages.harness, messages.session, messages.key, messages.role, messages.at, \
         snippet(messages, 0, char(1), char(2), '…', 64), files.title, files.cwd, files.branch \
         FROM messages LEFT JOIN files ON files.path = messages.path \
         WHERE messages.rowid IN ({newest}) AND messages MATCH ? ORDER BY messages.at DESC"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        let harness: String = row.get(0)?;
        let role: String = row.get(3)?;
        Ok(harness_of(&harness).map(|harness| MessageHit {
            harness,
            session: row.get(1).unwrap_or_default(),
            key: row.get(2).unwrap_or_default(),
            role: role_of(&role),
            at: row.get(4).unwrap_or_default(),
            snippet: row.get(5).unwrap_or_default(),
            title: row.get(6).unwrap_or_default(),
            cwd: row.get(7).unwrap_or_default(),
            branch: row.get(8).unwrap_or_default(),
        }))
    })?;
    let mut out = Vec::new();
    for row in rows {
        if let Some(hit) = row? {
            out.push(hit);
        }
    }
    Ok(out)
}

fn search(query: &str, limit: u32) -> Result<SearchReply, String> {
    let Some(conn) = store::open_reader()? else {
        return Ok(SearchReply::default());
    };
    let (indexed, total) = progress(&conn).map_err(|e| e.to_string())?;
    let Some(terms) = terms(query) else {
        return Ok(SearchReply {
            hits: Vec::new(),
            indexed,
            total,
        });
    };
    let hits = hits(&conn, &terms, limit).map_err(|e| e.to_string())?;
    Ok(SearchReply {
        hits,
        indexed,
        total,
    })
}

pub async fn search_messages(query: String, limit: Option<u32>) -> Result<SearchReply, String> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    spawn_blocking(move || search(&query, limit))
        .await
        .map_err(|e| e.to_string())?
}
