use std::collections::HashMap;
use std::sync::Arc;

use berth_core::acp::{AcpItem, AcpItemAt};
use berth_core::{
    Agent, AgentId, AgentKind, ClientRequest, Entity, EntityId, ServerEvent, Worktree, WorktreeId,
};
use rusqlite::{params, Connection};
use tokio::runtime::Handle;

use crate::berth::{self, Berth, ConnectionState, WatchEvent};
use berth_transcripts::search::Known;

const PREFIX: &str = "acp:";
const HARNESS: &str = "acp";

pub fn key_of(agent: &AgentId) -> String {
    format!("{PREFIX}{agent}")
}

pub fn is_key(path: &str) -> bool {
    path.starts_with(PREFIX)
}

#[derive(Clone)]
pub struct AcpAgent {
    id: AgentId,
    title: String,
    cwd: Option<String>,
    branch: Option<String>,
}

impl AcpAgent {
    pub fn key(&self) -> String {
        key_of(&self.id)
    }
}

#[derive(Default)]
pub struct Roster {
    synced: bool,
    agents: HashMap<AgentId, Agent>,
    worktrees: HashMap<WorktreeId, Worktree>,
}

impl Roster {
    pub fn apply(&mut self, event: WatchEvent) {
        match event {
            WatchEvent::Connection { state, .. } => {
                if state != ConnectionState::Connected {
                    self.synced = false;
                }
            }
            WatchEvent::Snapshot {
                worktrees, agents, ..
            } => {
                self.worktrees = worktrees.into_iter().map(|w| (w.id.clone(), w)).collect();
                self.agents = agents.into_iter().map(|a| (a.id.clone(), a)).collect();
                self.synced = true;
            }
            WatchEvent::Upserted {
                entity: Entity::Agent(agent),
            } => {
                self.agents.insert(agent.id.clone(), *agent);
            }
            WatchEvent::Upserted {
                entity: Entity::Worktree(worktree),
            } => {
                self.worktrees.insert(worktree.id.clone(), worktree);
            }
            WatchEvent::Removed {
                id: EntityId::Agent(id),
            } => {
                self.agents.remove(&id);
            }
            WatchEvent::Removed {
                id: EntityId::Worktree(id),
            } => {
                self.worktrees.remove(&id);
            }
            _ => {}
        }
    }

    pub fn acp_agents(&self) -> Option<Vec<AcpAgent>> {
        if !self.synced {
            return None;
        }
        Some(
            self.agents
                .values()
                .filter(|agent| agent.kind == AgentKind::Acp)
                .map(|agent| {
                    let worktree = self.worktrees.get(&agent.worktree_id);
                    AcpAgent {
                        id: agent.id.clone(),
                        title: agent.name.clone(),
                        cwd: worktree.map(|w| w.path.to_string_lossy().into_owned()),
                        branch: worktree.map(|w| w.branch.clone()),
                    }
                })
                .collect(),
        )
    }
}

pub struct Daemon {
    berth: Arc<Berth>,
    runtime: Handle,
}

impl Daemon {
    pub fn new(berth: Arc<Berth>, runtime: Handle) -> Self {
        Self { berth, runtime }
    }

    fn transcript(&self, agent: &AgentId, after: u64) -> Option<Vec<AcpItemAt>> {
        let request = serde_json::to_value(ClientRequest::AcpTranscript {
            req_id: 0,
            agent: agent.clone(),
            after,
        })
        .ok()?;
        let reply = self
            .runtime
            .block_on(berth::berth_call(&self.berth, request))
            .ok()?;
        match serde_json::from_value::<ServerEvent>(reply).ok()? {
            ServerEvent::AcpTranscript { items, .. } => Some(items),
            _ => None,
        }
    }
}

fn indexed(item: AcpItem) -> Option<(&'static str, String)> {
    let (role, text) = match item {
        AcpItem::User { text, .. } => ("user", text),
        AcpItem::Assistant { text } => ("assistant", text),
        _ => return None,
    };
    (!text.is_empty()).then_some((role, text))
}

fn width(text: &str) -> i64 {
    i64::try_from(text.len()).unwrap_or(i64::MAX)
}

pub fn index(
    conn: &mut Connection,
    daemon: &Daemon,
    agent: &AcpAgent,
    known: Option<&Known>,
) -> rusqlite::Result<bool> {
    let key = agent.key();
    let (seen, tail) = known.map_or((0, 0), |known| (known.offset, known.mtime));
    let after = u64::try_from(seen.saturating_sub(1)).unwrap_or(0);
    let Some(items) = daemon.transcript(&agent.id, after) else {
        return Ok(false);
    };
    let last = items
        .last()
        .map_or(seen, |item| i64::try_from(item.seq).unwrap_or(i64::MAX));
    let mut fresh = Vec::new();
    let mut last_width = tail;
    for AcpItemAt { seq, item } in items {
        let seq = i64::try_from(seq).unwrap_or(i64::MAX);
        let message = indexed(item);
        let message_width = message.as_ref().map_or(0, |(_, text)| width(text));
        if seq == last {
            last_width = message_width;
        }
        if known.is_some() && seq == seen && message_width == tail {
            continue;
        }
        if let Some((role, text)) = message {
            fresh.push((seq, role, text));
        }
    }
    if fresh.is_empty() && last == seen {
        conn.execute(
            "UPDATE files SET title = ?2, cwd = ?3, branch = ?4 WHERE path = ?1 \
             AND (title IS NOT ?2 OR cwd IS NOT ?3 OR branch IS NOT ?4)",
            params![key, agent.title, agent.cwd, agent.branch],
        )?;
        return Ok(true);
    }
    let tx = conn.transaction()?;
    {
        let mut remove = tx.prepare_cached("DELETE FROM messages WHERE path = ?1 AND key = ?2")?;
        let mut insert = tx.prepare_cached(
            "INSERT INTO messages(text, harness, session, key, role, at, path) \
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?6)",
        )?;
        for (seq, role, text) in &fresh {
            let seq_key = format!("{PREFIX}{seq}");
            if known.is_some() && *seq == seen {
                remove.execute(params![key, seq_key])?;
            }
            insert.execute(params![
                text,
                HARNESS,
                agent.id.as_str(),
                seq_key,
                role,
                key
            ])?;
        }
    }
    tx.execute(
        "INSERT INTO files(path, harness, session, size, mtime, offset, title, cwd, branch) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?6, ?7, ?8) \
         ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime, \
         offset = excluded.offset, title = excluded.title, cwd = excluded.cwd, \
         branch = excluded.branch",
        params![
            key,
            HARNESS,
            agent.id.as_str(),
            last,
            last_width,
            agent.title,
            agent.cwd,
            agent.branch
        ],
    )?;
    tx.commit()?;
    Ok(true)
}
