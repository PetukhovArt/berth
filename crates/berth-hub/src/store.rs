use crate::mention::{mention_key, mentions_in};
use anyhow::{anyhow, bail, Result};
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use subtle::ConstantTimeEq;

pub const KIND_MESSAGE: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub seq: i64,
    pub kind: u32,
    pub author: String,
    pub author_name: String,
    pub author_kind: String,
    pub channel: String,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelSummary {
    pub key: String,
    pub head: i64,
    pub unread: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mention {
    pub event: Event,
    pub agent: String,
    pub agent_name: String,
    pub from_owner: bool,
    pub chain: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub status: String,
    pub scopes: Vec<String>,
}

pub const MENTION_TAG: &str = "p";
const CHAIN_SCAN: i64 = 100;

#[derive(Debug, Clone)]
pub struct Member {
    pub id: String,
    pub scopes: Vec<String>,
}

impl Member {
    pub fn can(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

pub struct Store {
    db: Connection,
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    fn init(db: Connection) -> Result<Self> {
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY, v TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS members (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL,
               owner TEXT, token TEXT NOT NULL, status TEXT NOT NULL,
               scopes TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS channels (key TEXT PRIMARY KEY, created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS channel_members (
               channel TEXT NOT NULL, member TEXT NOT NULL, last_read INTEGER NOT NULL DEFAULT 0,
               PRIMARY KEY (channel, member));
             CREATE TABLE IF NOT EXISTS events (
               seq INTEGER PRIMARY KEY AUTOINCREMENT, kind INTEGER NOT NULL, author TEXT NOT NULL,
               channel TEXT NOT NULL, tags TEXT NOT NULL, content TEXT NOT NULL,
               created_at INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS events_channel ON events (channel, seq);",
        )?;
        let has_display: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('members') WHERE name='display')",
            [],
            |r| r.get(0),
        )?;
        if !has_display {
            db.execute_batch("ALTER TABLE members ADD COLUMN display TEXT;")?;
        }
        Ok(Self { db })
    }

    pub fn admin_token(&self, preset: Option<String>) -> Result<String> {
        if let Some(t) = preset {
            self.db.execute(
                "INSERT INTO meta (k, v) VALUES ('admin_token', ?1)
                 ON CONFLICT(k) DO UPDATE SET v = ?1",
                [&t],
            )?;
            return Ok(t);
        }
        if let Some(t) = self
            .db
            .query_row("SELECT v FROM meta WHERE k='admin_token'", [], |r| r.get(0))
            .optional()?
        {
            return Ok(t);
        }
        let t = random_token();
        self.db
            .execute("INSERT INTO meta (k, v) VALUES ('admin_token', ?1)", [&t])?;
        Ok(t)
    }

    pub fn is_admin(&self, presented: &str) -> bool {
        let stored: Option<String> = self
            .db
            .query_row("SELECT v FROM meta WHERE k='admin_token'", [], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        stored.is_some_and(|s| s.as_bytes().ct_eq(presented.as_bytes()).into())
    }

    pub fn admit_device(&self, name: &str) -> Result<(String, String)> {
        let id = format!("m-{}", &random_token()[..12]);
        let token = random_token();
        self.db.execute(
            "INSERT INTO members (id, name, kind, token, status, created_at)
             VALUES (?1, ?2, 'person', ?3, 'pending', ?4)",
            params![id, name, token, now_ms()],
        )?;
        Ok((id, token))
    }

    pub fn pending(&self) -> Result<Vec<(String, String)>> {
        let mut st = self
            .db
            .prepare("SELECT id, name FROM members WHERE status='pending' ORDER BY created_at")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let mut st = self.db.prepare(
            "SELECT id, name, status, scopes FROM members WHERE kind='person' ORDER BY created_at",
        )?;
        let rows = st.query_map([], |r| {
            let scopes: String = r.get(3)?;
            Ok(Device {
                id: r.get(0)?,
                name: r.get(1)?,
                status: r.get(2)?,
                scopes: scopes
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn approve_scopes(&self, id: &str, scopes: &[String]) -> Result<()> {
        let n = self.db.execute(
            "UPDATE members SET status='approved', scopes=?2
             WHERE id=?1 AND kind='person' AND status <> 'revoked'",
            params![id, scopes.join(",")],
        )?;
        if n == 0 {
            bail!("no such device, or it was revoked: pair again");
        }
        Ok(())
    }

    pub fn revoke_device(&self, id: &str) -> Result<()> {
        let n = self.db.execute(
            "UPDATE members SET status='revoked' WHERE id=?1 AND kind='person'",
            [id],
        )?;
        if n == 0 {
            bail!("no such device");
        }
        self.db.execute(
            "UPDATE members SET status='revoked' WHERE owner=?1 AND kind='agent'",
            [id],
        )?;
        Ok(())
    }

    pub fn member_by_token(&self, presented: &str) -> Result<Option<Member>> {
        let mut st = self
            .db
            .prepare("SELECT id, scopes, token FROM members WHERE status = 'approved'")?;
        let mut rows = st.query([])?;
        let mut found = None;
        while let Some(r) = rows.next()? {
            let token: String = r.get(2)?;
            if bool::from(token.as_bytes().ct_eq(presented.as_bytes())) {
                let scopes: String = r.get(1)?;
                found = Some(Member {
                    id: r.get(0)?,
                    scopes: scopes
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect(),
                });
            }
        }
        Ok(found)
    }

    pub fn ensure_agent(
        &self,
        owner: &Member,
        name: &str,
        display: Option<&str>,
    ) -> Result<String> {
        let existing: Option<String> = self
            .db
            .query_row(
                "SELECT id FROM members WHERE kind='agent' AND owner=?1 AND name=?2",
                params![owner.id, name],
                |r| r.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => id,
            None => {
                let id = format!("a-{}", &random_token()[..12]);
                self.db.execute(
                    "INSERT INTO members (id, name, kind, owner, token, status, scopes, created_at)
                     VALUES (?1, ?2, 'agent', ?3, ?4, 'agent', '', ?5)",
                    params![id, name, owner.id, random_token(), now_ms()],
                )?;
                id
            }
        };
        if let Some(display) = display.map(str::trim).filter(|d| !d.is_empty()) {
            self.db.execute(
                "UPDATE members SET display=?2 WHERE id=?1",
                params![id, display],
            )?;
        }
        Ok(id)
    }

    fn agent_owner(&self, agent: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT owner FROM members WHERE id=?1 AND kind='agent' AND status='agent'",
                [agent],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn acting_member(&self, caller: &Member, as_agent: Option<&str>) -> Result<String> {
        let Some(agent) = as_agent else {
            return Ok(caller.id.clone());
        };
        match self.agent_owner(agent)? {
            Some(owner) if owner == caller.id => Ok(agent.to_string()),
            _ => bail!("not your Agent"),
        }
    }

    pub fn create_channel(&self, key: &str, creator: &Member) -> Result<()> {
        if key.trim().is_empty() {
            bail!("empty channel key");
        }
        let taken: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM channels WHERE key=?1)",
            [key],
            |r| r.get(0),
        )?;
        if taken && !self.is_member(key, &creator.id)? {
            bail!("channel exists and you are not in it");
        }
        self.db.execute(
            "INSERT OR IGNORE INTO channels (key, created_at) VALUES (?1, ?2)",
            params![key, now_ms()],
        )?;
        self.db.execute(
            "INSERT OR IGNORE INTO channel_members (channel, member) VALUES (?1, ?2)",
            params![key, creator.id],
        )?;
        Ok(())
    }

    pub fn add_member(&self, key: &str, by: &Member, member: &str) -> Result<()> {
        if !self.is_member(key, &by.id)? {
            bail!("not a member of this channel");
        }
        self.db.execute(
            "INSERT OR IGNORE INTO channel_members (channel, member) VALUES (?1, ?2)",
            params![key, member],
        )?;
        Ok(())
    }

    pub fn is_member(&self, key: &str, member: &str) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM channel_members WHERE channel=?1 AND member=?2)",
            params![key, member],
            |r| r.get(0),
        )?)
    }

    fn can_access(&self, key: &str, member: &str) -> Result<bool> {
        if self.is_member(key, member)? {
            return Ok(true);
        }
        match self.agent_owner(member)? {
            Some(owner) => self.is_member(key, &owner),
            None => Ok(false),
        }
    }

    pub fn post(
        &self,
        key: &str,
        author: &str,
        kind: u32,
        content: &str,
        tags: &[Vec<String>],
    ) -> Result<Event> {
        if !self.can_access(key, author)? {
            bail!("not a member of this channel");
        }
        self.db.execute(
            "INSERT INTO events (kind, author, channel, tags, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                kind,
                author,
                key,
                serde_json::to_string(tags)?,
                content,
                now_ms()
            ],
        )?;
        let seq = self.db.last_insert_rowid();
        self.events_where("e.seq = ?1", params![seq])?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("event vanished"))
    }

    fn events_where(&self, cond: &str, p: impl rusqlite::Params) -> Result<Vec<Event>> {
        let sql = format!(
            "SELECT e.seq, e.kind, e.author, COALESCE(m.display, m.name), m.kind, e.channel, e.tags, e.content, e.created_at
             FROM events e JOIN members m ON m.id = e.author WHERE {cond} ORDER BY e.seq"
        );
        let mut st = self.db.prepare(&sql)?;
        let rows = st.query_map(p, |r| {
            let tags: String = r.get(6)?;
            Ok(Event {
                seq: r.get(0)?,
                kind: r.get(1)?,
                author: r.get(2)?,
                author_name: r.get(3)?,
                author_kind: r.get(4)?,
                channel: r.get(5)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                content: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn mention_tags(&self, key: &str, author: &str, content: &str) -> Result<Vec<Vec<String>>> {
        let words = mentions_in(content);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let author_owner = self
            .agent_owner(author)?
            .unwrap_or_else(|| author.to_string());
        let mut st = self.db.prepare(
            "SELECT id, name, display, owner FROM members
             WHERE kind='agent' AND status='agent' ORDER BY created_at",
        )?;
        let agents = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut tags: Vec<Vec<String>> = Vec::new();
        for (_, word) in words {
            let mut found = Vec::new();
            for (id, name, display, owner) in &agents {
                let named = mention_key(name) == word
                    || display.as_deref().is_some_and(|d| mention_key(d) == word);
                if named && self.can_access(key, id)? {
                    found.push((id, owner));
                }
            }
            if found.iter().any(|(_, owner)| **owner == author_owner) {
                found.retain(|(_, owner)| **owner == author_owner);
            }
            for (id, _) in found {
                let tag = vec![MENTION_TAG.to_string(), id.clone()];
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
        Ok(tags)
    }

    fn agent_chain(&self, channel: &str, seq: i64) -> Result<u32> {
        let mut st = self.db.prepare(
            "SELECT m.kind FROM events e JOIN members m ON m.id = e.author
             WHERE e.channel = ?1 AND e.seq <= ?2 ORDER BY e.seq DESC LIMIT ?3",
        )?;
        let kinds = st.query_map(params![channel, seq, CHAIN_SCAN], |r| r.get::<_, String>(0))?;
        let mut n = 0;
        for kind in kinds {
            if kind? != "agent" {
                break;
            }
            n += 1;
        }
        Ok(n)
    }

    pub fn mentions(&self, caller: &Member, after: i64) -> Result<Vec<Mention>> {
        let events = self.events_where("e.seq > ?1 AND e.tags LIKE '%\"p\"%'", params![after])?;
        let mut out = Vec::new();
        for event in events {
            for tag in event
                .tags
                .iter()
                .filter(|t| t.len() == 2 && t[0] == MENTION_TAG)
            {
                let agent = &tag[1];
                let Some(owner) = self.agent_owner(agent)? else {
                    continue;
                };
                if owner != caller.id || !self.can_access(&event.channel, agent)? {
                    continue;
                }
                let agent_name: String =
                    self.db
                        .query_row("SELECT name FROM members WHERE id=?1", [agent], |r| {
                            r.get(0)
                        })?;
                out.push(Mention {
                    from_owner: event.author == owner,
                    chain: self.agent_chain(&event.channel, event.seq)?,
                    agent: agent.clone(),
                    agent_name,
                    event: event.clone(),
                });
            }
        }
        Ok(out)
    }

    pub fn read_after(&self, key: &str, reader: &Member, after: i64) -> Result<Vec<Event>> {
        if !self.can_access(key, &reader.id)? {
            bail!("not a member of this channel");
        }
        let events = self.events_where("e.channel = ?1 AND e.seq > ?2", params![key, after])?;
        if let Some(last) = events.last() {
            self.db.execute(
                "UPDATE channel_members SET last_read = MAX(last_read, ?3)
                 WHERE channel=?1 AND member=?2",
                params![key, reader.id, last.seq],
            )?;
        }
        Ok(events)
    }

    pub fn channels(&self, member: &Member) -> Result<Vec<ChannelSummary>> {
        let mut st = self.db.prepare(
            "SELECT cm.channel,
                    COALESCE((SELECT MAX(seq) FROM events WHERE channel = cm.channel), 0),
                    (SELECT COUNT(*) FROM events WHERE channel = cm.channel AND seq > cm.last_read
                       AND author <> cm.member)
             FROM channel_members cm WHERE cm.member = ?1 ORDER BY cm.channel",
        )?;
        let rows = st.query_map([&member.id], |r| {
            Ok(ChannelSummary {
                key: r.get(0)?,
                head: r.get(1)?,
                unread: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn member_name(&self, id: &str) -> Result<String> {
        Ok(self
            .db
            .query_row("SELECT name FROM members WHERE id=?1", [id], |r| r.get(0))?)
    }
}
