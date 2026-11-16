use std::collections::{BTreeSet, VecDeque};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use berth_core::{AgentId, ChannelGrant, ServerEvent};
use berth_hub::{Event, Mention};
use rusqlite::{params, OptionalExtension};

use crate::channels::hub;
use crate::store::Store;

struct Promoted {
    queue: Vec<Mention>,
    marked: BTreeSet<(i64, String)>,
}

static PROMOTED: Mutex<Promoted> = Mutex::new(Promoted {
    queue: Vec::new(),
    marked: BTreeSet::new(),
});

pub(crate) fn may_instruct(store: &Store, m: &Mention) -> bool {
    if PROMOTED
        .lock()
        .unwrap()
        .marked
        .contains(&(m.event.seq, m.agent.clone()))
    {
        return true;
    }
    store
        .conn()
        .query_row(
            "SELECT 1 FROM channel_grants WHERE channel = ?1 AND agent_id = ?2 AND member = ?3",
            params![m.event.channel, m.agent_name, m.event.author],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some()
}

pub(crate) fn take_promoted(pending: &mut VecDeque<Mention>) {
    let mut promoted = PROMOTED.lock().unwrap();
    for m in promoted.queue.drain(..) {
        let queued = pending
            .iter()
            .any(|p| p.event.seq == m.event.seq && p.agent == m.agent);
        if !queued {
            pending.push_back(m);
        }
    }
}

fn promote_mentions(mentions: Vec<Mention>) {
    let mut promoted = PROMOTED.lock().unwrap();
    for m in mentions {
        promoted.marked.insert((m.event.seq, m.agent.clone()));
        promoted.queue.push(m);
    }
}

fn own_agent(store: &Store, agent: &AgentId) -> Result<()> {
    match store.get_agent(agent)? {
        Some(_) => Ok(()),
        None => bail!("no Agent {} on this machine", agent.as_str()),
    }
}

pub async fn promote(store: &Store, channel: &str, seq: i64, agent: Option<AgentId>) -> Result<()> {
    let hub = hub()?;
    let mentions = match agent {
        Some(agent) => {
            own_agent(store, &agent)?;
            let member = hub.agent(agent.as_str()).await?;
            let event = hub
                .read(channel, seq - 1, Some(member.as_str()))
                .await?
                .into_iter()
                .find(|e| e.seq == seq)
                .ok_or_else(|| anyhow!("no message {seq} in {channel}"))?;
            vec![Mention {
                event,
                agent: member,
                agent_name: agent.0,
                from_owner: false,
                chain: 0,
            }]
        }
        None => {
            let found: Vec<Mention> = hub
                .mentions(seq - 1)
                .await?
                .into_iter()
                .filter(|m| m.event.seq == seq && m.event.channel == channel)
                .collect();
            if found.is_empty() {
                bail!("message {seq} in {channel} mentions none of your Agents: name one with --agent");
            }
            found
        }
    };
    promote_mentions(mentions);
    Ok(())
}

fn colleague(events: &[Event], who: &str) -> Result<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    for e in events {
        let named = e.author == who || e.author_name == who;
        if named && e.author_kind == "person" && !found.iter().any(|(id, _)| *id == e.author) {
            found.push((e.author.clone(), e.author_name.clone()));
        }
    }
    match found.len() {
        0 => bail!("nobody called {who} has written in this channel"),
        1 => Ok(found.remove(0)),
        _ => bail!(
            "several members are called {who}, name one by id: {}",
            found
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn insert_grant(
    store: &Store,
    channel: &str,
    agent: &AgentId,
    member: &str,
    name: &str,
) -> Result<()> {
    store.conn().execute(
        "INSERT OR REPLACE INTO channel_grants (channel, agent_id, member, member_name)
         VALUES (?1, ?2, ?3, ?4)",
        params![channel, agent.as_str(), member, name],
    )?;
    Ok(())
}

fn delete_grant(store: &Store, channel: &str, agent: &AgentId, who: &str) -> Result<()> {
    let n = store.conn().execute(
        "DELETE FROM channel_grants WHERE channel = ?1 AND agent_id = ?2
         AND (member = ?3 OR member_name = ?3)",
        params![channel, agent.as_str(), who],
    )?;
    if n == 0 {
        bail!("{who} has no grant for {} in {channel}", agent.as_str());
    }
    Ok(())
}

pub fn list_grants(store: &Store, channel: Option<&str>) -> Result<Vec<ChannelGrant>> {
    let conn = store.conn();
    let mut st = conn.prepare(
        "SELECT channel, agent_id, member, member_name FROM channel_grants
         WHERE ?1 IS NULL OR channel = ?1 ORDER BY channel, agent_id, member_name",
    )?;
    let rows = st.query_map([channel], |r| {
        Ok(ChannelGrant {
            channel: r.get(0)?,
            agent: AgentId(r.get(1)?),
            member: r.get(2)?,
            member_name: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn grants_reply(req_id: u64, result: Result<Vec<ChannelGrant>>) -> ServerEvent {
    match result {
        Ok(grants) => ServerEvent::ChannelGrants { req_id, grants },
        Err(e) => ServerEvent::Error {
            req_id: Some(req_id),
            message: format!("{e:#}"),
        },
    }
}

pub async fn set_grant(
    store: &Store,
    channel: &str,
    agent: &AgentId,
    who: &str,
    granted: bool,
) -> Result<Vec<ChannelGrant>> {
    if granted {
        own_agent(store, agent)?;
        let hub = hub()?;
        let reader = hub.agent(agent.as_str()).await?;
        let events = hub.read(channel, 0, Some(reader.as_str())).await?;
        let (member, name) = colleague(&events, who)?;
        insert_grant(store, channel, agent, &member, &name)?;
    } else {
        delete_grant(store, channel, agent, who)?;
    }
    list_grants(store, Some(channel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: i64, author: &str, name: &str, kind: &str, channel: &str) -> Event {
        Event {
            seq,
            kind: 1,
            author: author.into(),
            author_name: name.into(),
            author_kind: kind.into(),
            channel: channel.into(),
            tags: vec![],
            content: "@agent-1 use the v2 api".into(),
            created_at: 0,
        }
    }

    fn mention(e: Event) -> Mention {
        Mention {
            event: e,
            agent: "a-1".into(),
            agent_name: "agent-1".into(),
            from_owner: false,
            chain: 0,
        }
    }

    #[test]
    fn a_grant_lets_one_colleague_instruct_one_agent_in_one_channel_until_revoked() {
        let store = Store::open_in_memory().unwrap();
        store
            .conn()
            .pragma_update(None, "foreign_keys", "OFF")
            .unwrap();
        let agent = AgentId("agent-1".into());
        let history = [
            event(1, "m-bob", "bob", "person", "PROJ-1"),
            event(2, "a-9", "bob", "agent", "PROJ-1"),
        ];
        let (member, name) = colleague(&history, "bob").unwrap();
        insert_grant(&store, "PROJ-1", &agent, &member, &name).unwrap();

        assert!(may_instruct(
            &store,
            &mention(event(5, "m-bob", "bob", "person", "PROJ-1"))
        ));
        assert!(!may_instruct(
            &store,
            &mention(event(6, "m-bob", "bob", "person", "PROJ-2"))
        ));
        assert!(!may_instruct(
            &store,
            &mention(event(7, "m-eve", "eve", "person", "PROJ-1"))
        ));

        delete_grant(&store, "PROJ-1", &agent, "bob").unwrap();
        assert!(!may_instruct(
            &store,
            &mention(event(8, "m-bob", "bob", "person", "PROJ-1"))
        ));
        assert!(list_grants(&store, None).unwrap().is_empty());
    }

    #[test]
    fn a_promoted_message_instructs_once_queued_without_a_duplicate() {
        let store = Store::open_in_memory().unwrap();
        let m = mention(event(40, "m-bob", "bob", "person", "PROJ-1"));
        assert!(!may_instruct(&store, &m));
        let mut pending = VecDeque::from([m.clone()]);
        promote_mentions(vec![m.clone()]);
        take_promoted(&mut pending);
        assert_eq!(pending.len(), 1);
        assert!(may_instruct(&store, &m));
    }
}
