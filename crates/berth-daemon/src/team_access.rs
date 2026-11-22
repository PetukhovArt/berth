use std::collections::BTreeSet;

use anyhow::{anyhow, bail, Result};
use berth_core::{AgentId, ChannelGrant, ServerEvent};
use berth_hub::client::HubClient;
use berth_hub::{Ceiling, Event, Mention, KIND_MESSAGE};
use rusqlite::{params, OptionalExtension};

use crate::channels::hub;
use crate::config::Config;
use crate::store::Store;
use crate::team_pending::Pending;

pub(crate) fn may_instruct(
    store: &Store,
    ceiling: Option<&Ceiling>,
    m: &Mention,
    promoted: bool,
) -> Option<bool> {
    if promoted {
        return Some(true);
    }
    if !has_grant(store, m) {
        return Some(false);
    }
    ceiling.map(|c| c.allows(&m.event.author))
}

pub(crate) async fn ceiling_for(
    hub: &HubClient,
    store: &Store,
    pending: &[Pending],
) -> Option<Ceiling> {
    let authors: BTreeSet<String> = pending
        .iter()
        .filter(|p| !p.mention.from_owner && !p.promoted && has_grant(store, &p.mention))
        .map(|p| p.mention.event.author.clone())
        .collect();
    if authors.is_empty() {
        return None;
    }
    match hub.ceiling(&authors.into_iter().collect::<Vec<_>>()).await {
        Ok(ceiling) => Some(ceiling),
        Err(e) => {
            tracing::debug!(error = %e, "reading the instruct ceiling failed");
            None
        }
    }
}

fn has_grant(store: &Store, m: &Mention) -> bool {
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
                .find(|e| e.seq == seq && e.kind == KIND_MESSAGE)
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
    let hub_url = Config::load().hub_url.unwrap_or_default();
    crate::team_pending::promote(store, &hub_url, &mentions)
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
        if !hub
            .ceiling(std::slice::from_ref(&member))
            .await?
            .allows(&member)
        {
            bail!("{name} cannot instruct Agents: their role on this hub does not include it");
        }
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

        assert!(has_grant(
            &store,
            &mention(event(5, "m-bob", "bob", "person", "PROJ-1"))
        ));
        assert!(!has_grant(
            &store,
            &mention(event(6, "m-bob", "bob", "person", "PROJ-2"))
        ));
        assert!(!has_grant(
            &store,
            &mention(event(7, "m-eve", "eve", "person", "PROJ-1"))
        ));

        delete_grant(&store, "PROJ-1", &agent, "bob").unwrap();
        assert!(!has_grant(
            &store,
            &mention(event(8, "m-bob", "bob", "person", "PROJ-1"))
        ));
        assert!(list_grants(&store, None).unwrap().is_empty());
    }

    #[test]
    fn a_grant_instructs_only_inside_the_hub_ceiling_and_waits_while_it_is_unknown() {
        let store = Store::open_in_memory().unwrap();
        store
            .conn()
            .pragma_update(None, "foreign_keys", "OFF")
            .unwrap();
        let agent = AgentId("agent-1".into());
        insert_grant(&store, "PROJ-1", &agent, "m-bob", "bob").unwrap();
        let granted = mention(event(5, "m-bob", "bob", "person", "PROJ-1"));
        let stranger = mention(event(6, "m-eve", "eve", "person", "PROJ-1"));
        let bob_may = Ceiling::Only(BTreeSet::from(["m-bob".to_string()]));
        let nobody_may = Ceiling::Only(BTreeSet::new());

        assert_eq!(
            may_instruct(&store, Some(&bob_may), &granted, false),
            Some(true)
        );
        assert_eq!(
            may_instruct(&store, Some(&Ceiling::Unlimited), &granted, false),
            Some(true)
        );
        assert_eq!(
            may_instruct(&store, Some(&nobody_may), &granted, false),
            Some(false)
        );
        assert_eq!(
            may_instruct(&store, Some(&bob_may), &stranger, false),
            Some(false)
        );
        assert_eq!(
            may_instruct(&store, Some(&nobody_may), &stranger, true),
            Some(true)
        );

        assert_eq!(may_instruct(&store, None, &granted, false), None);
        assert_eq!(may_instruct(&store, None, &stranger, false), Some(false));
        assert_eq!(may_instruct(&store, None, &granted, true), Some(true));
    }
}
