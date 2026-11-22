use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::Serialize;
use tokio::task::spawn_blocking;

use super::store;
use crate::agent_sessions::Role;

const DEFAULT_LIMIT: u32 = 200;
const MAX_LIMIT: u32 = 500;
const SNIPPETS_PER_CONVERSATION: u32 = 3;
const TERM_CHARS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
enum HitHarness {
    Claude,
    Codex,
    Acp,
}

#[derive(Debug, Serialize)]
pub struct MessageSnippet {
    key: String,
    role: Role,
    at: Option<String>,
    snippet: String,
}

#[derive(Debug, Serialize)]
pub struct ConversationHit {
    harness: HitHarness,
    session: String,
    title: Option<String>,
    cwd: Option<String>,
    branch: Option<String>,
    matches: u32,
    at: Option<String>,
    snippets: Vec<MessageSnippet>,
}

#[derive(Debug, Default, Serialize)]
pub struct SearchReply {
    conversations: Vec<ConversationHit>,
    total: u32,
    indexed: u32,
    total_files: u32,
    remote: bool,
}

struct Terms {
    fts: String,
    like: Vec<String>,
}

impl Terms {
    fn filter(&self) -> String {
        let mut sql = String::from("messages MATCH ?");
        for _ in &self.like {
            sql.push_str(" AND text LIKE ? ESCAPE '\\'");
        }
        sql
    }

    fn values(&self) -> Vec<Value> {
        let mut values = vec![Value::Text(self.fts.clone())];
        values.extend(self.like.iter().cloned().map(Value::Text));
        values
    }
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

fn harness_name(harness: HitHarness) -> &'static str {
    match harness {
        HitHarness::Claude => "claude",
        HitHarness::Codex => "codex",
        HitHarness::Acp => "acp",
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

fn newest_conversations(
    conn: &Connection,
    terms: &Terms,
    limit: u32,
) -> rusqlite::Result<(Vec<ConversationHit>, u32)> {
    let sql = format!(
        "SELECT harness, session, COUNT(*), MAX(at), COUNT(*) OVER () FROM messages \
         WHERE {} GROUP BY harness, session ORDER BY MAX(at) DESC LIMIT ?",
        terms.filter()
    );
    let mut values = terms.values();
    values.push(Value::Integer(i64::from(limit)));
    let mut statement = conn.prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(values))?;
    let mut conversations = Vec::new();
    let mut total = 0;
    while let Some(row) = rows.next()? {
        total = row.get(4)?;
        let harness: String = row.get(0)?;
        if let Some(harness) = harness_of(&harness) {
            conversations.push(ConversationHit {
                harness,
                session: row.get(1)?,
                title: None,
                cwd: None,
                branch: None,
                matches: row.get(2)?,
                at: row.get(3)?,
                snippets: Vec::new(),
            });
        }
    }
    Ok((conversations, total))
}

fn add_snippets(
    conn: &Connection,
    terms: &Terms,
    conversations: &mut [ConversationHit],
) -> rusqlite::Result<()> {
    let wanted = vec!["(?, ?)"; conversations.len()].join(", ");
    let sql = format!(
        "SELECT messages.harness, messages.session, messages.key, messages.role, messages.at, \
         snippet(messages, 0, char(1), char(2), '…', 64), files.title, files.cwd, files.branch \
         FROM messages LEFT JOIN files ON files.path = messages.path \
         WHERE messages.rowid IN (SELECT id FROM (SELECT rowid AS id, \
         ROW_NUMBER() OVER (PARTITION BY harness, session ORDER BY at DESC) AS n \
         FROM messages WHERE {} AND (harness, session) IN (VALUES {wanted})) WHERE n <= ?) \
         AND messages MATCH ? ORDER BY messages.at DESC",
        terms.filter()
    );
    let mut values = terms.values();
    for conversation in conversations.iter() {
        values.push(Value::Text(harness_name(conversation.harness).to_string()));
        values.push(Value::Text(conversation.session.clone()));
    }
    values.push(Value::Integer(i64::from(SNIPPETS_PER_CONVERSATION)));
    values.push(Value::Text(terms.fts.clone()));
    let positions: HashMap<(HitHarness, String), usize> = conversations
        .iter()
        .enumerate()
        .map(|(at, conversation)| ((conversation.harness, conversation.session.clone()), at))
        .collect();
    let mut statement = conn.prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(values))?;
    while let Some(row) = rows.next()? {
        let harness: String = row.get(0)?;
        let session: String = row.get(1)?;
        let role: String = row.get(3)?;
        let Some(harness) = harness_of(&harness) else {
            continue;
        };
        let Some(&at) = positions.get(&(harness, session)) else {
            continue;
        };
        let conversation = &mut conversations[at];
        if conversation.snippets.is_empty() {
            conversation.title = row.get(6)?;
            conversation.cwd = row.get(7)?;
            conversation.branch = row.get(8)?;
        }
        conversation.snippets.push(MessageSnippet {
            key: row.get(2)?,
            role: role_of(&role),
            at: row.get(4)?,
            snippet: row.get(5)?,
        });
    }
    Ok(())
}

fn conversations(
    conn: &Connection,
    terms: &Terms,
    limit: u32,
) -> rusqlite::Result<(Vec<ConversationHit>, u32)> {
    let (mut conversations, total) = newest_conversations(conn, terms, limit)?;
    if !conversations.is_empty() {
        add_snippets(conn, terms, &mut conversations)?;
    }
    Ok((conversations, total))
}

fn search(query: &str, limit: u32) -> Result<SearchReply, String> {
    if crate::ssh_remote::target().is_some() {
        return Ok(SearchReply {
            remote: true,
            ..SearchReply::default()
        });
    }
    let Some(conn) = store::open_reader()? else {
        return Ok(SearchReply::default());
    };
    let (indexed, total_files) = progress(&conn).map_err(|e| e.to_string())?;
    let (conversations, total) = match terms(query) {
        Some(terms) => conversations(&conn, &terms, limit).map_err(|e| e.to_string())?,
        None => (Vec::new(), 0),
    };
    Ok(SearchReply {
        conversations,
        total,
        indexed,
        total_files,
        remote: false,
    })
}

pub async fn search_conversations(
    query: String,
    limit: Option<u32>,
) -> Result<SearchReply, String> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    spawn_blocking(move || search(&query, limit))
        .await
        .map_err(|e| e.to_string())?
}
