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

    pub fn approve_scopes(&self, id: &str, scopes: &[String]) -> Result<()> {
        let n = self.db.execute(
            "UPDATE members SET status='approved', scopes=?2 WHERE id=?1 AND kind='person'",
            params![id, scopes.join(",")],
        )?;
        if n == 0 {
            bail!("no such device");
        }
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

    pub fn ensure_agent(&self, owner: &Member, name: &str) -> Result<String> {
        if let Some(id) = self
            .db
            .query_row(
                "SELECT id FROM members WHERE kind='agent' AND owner=?1 AND name=?2",
                params![owner.id, name],
                |r| r.get(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        let id = format!("a-{}", &random_token()[..12]);
        self.db.execute(
            "INSERT INTO members (id, name, kind, owner, token, status, scopes, created_at)
             VALUES (?1, ?2, 'agent', ?3, ?4, 'agent', '', ?5)",
            params![id, name, owner.id, random_token(), now_ms()],
        )?;
        Ok(id)
    }

    fn agent_owner(&self, agent: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT owner FROM members WHERE id=?1 AND kind='agent'",
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

    fn is_member(&self, key: &str, member: &str) -> Result<bool> {
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
            params![kind, author, key, serde_json::to_string(tags)?, content, now_ms()],
        )?;
        let seq = self.db.last_insert_rowid();
        self.events_where("e.seq = ?1", params![seq])?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("event vanished"))
    }

    fn events_where(&self, cond: &str, p: impl rusqlite::Params) -> Result<Vec<Event>> {
        let sql = format!(
            "SELECT e.seq, e.kind, e.author, m.name, m.kind, e.channel, e.tags, e.content, e.created_at
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
}
