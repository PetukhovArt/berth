use crate::config::Config;
use crate::registry::Daemon;
use crate::store::Store;
use anyhow::{anyhow, bail, Result};
use berth_core::{AgentId, ChannelEvent, ChannelPresent, ChannelSummary, ChannelTouching};
use berth_hub::client::HubClient;
use berth_hub::{Event, MENTION_TAG};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

pub(crate) fn hub() -> Result<HubClient> {
    let config = Config::load();
    let url = config.hub_url.ok_or_else(|| {
        anyhow!("no team hub: set \"hub_url\" and \"hub_token\" in the config file")
    })?;
    let token = config
        .hub_token
        .ok_or_else(|| anyhow!("no hub token: set \"hub_token\" in the config file"))?;
    HubClient::new(&url, &token)
}

pub(crate) fn convert<T: Serialize, U: DeserializeOwned>(value: T) -> Result<U> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

async fn agent_member(hub: &HubClient, agent: &Option<AgentId>) -> Result<Option<String>> {
    match agent {
        Some(id) => Ok(Some(hub.agent(id.as_str()).await?)),
        None => Ok(None),
    }
}

struct HubIds {
    hub: String,
    me: String,
    agents: HashMap<AgentId, String>,
}

static HUB_IDS: Mutex<Option<HubIds>> = Mutex::new(None);

#[derive(Default)]
pub(crate) struct Own {
    me: String,
    agents: HashMap<String, AgentId>,
}

fn live_agents(store: &Store) -> Vec<AgentId> {
    let conn = store.conn();
    let Ok(mut st) = conn.prepare("SELECT id FROM agents WHERE archived = 0") else {
        return Vec::new();
    };
    let ids = st
        .query_map([], |r| r.get::<_, String>(0))
        .map(|rows| rows.filter_map(Result::ok).map(AgentId).collect())
        .unwrap_or_default();
    ids
}

pub(crate) async fn own(store: &Store, hub: &HubClient) -> Own {
    let config = Config::load();
    let key = format!(
        "{}\n{}",
        config.hub_url.unwrap_or_default(),
        config.hub_token.unwrap_or_default()
    );
    let cached = HUB_IDS
        .lock()
        .unwrap()
        .as_ref()
        .filter(|ids| ids.hub == key)
        .map(|ids| (ids.me.clone(), ids.agents.clone()));
    let (me, mut members) = match cached {
        Some(cached) => cached,
        None => match hub.me().await {
            Ok(me) => (me, HashMap::new()),
            Err(_) => return Own::default(),
        },
    };
    let mut agents = HashMap::new();
    for agent in live_agents(store) {
        let member = match members.get(&agent) {
            Some(member) => member.clone(),
            None => match hub.agent(agent.as_str()).await {
                Ok(member) if !member.is_empty() => {
                    members.insert(agent.clone(), member.clone());
                    member
                }
                _ => continue,
            },
        };
        agents.insert(member, agent);
    }
    *HUB_IDS.lock().unwrap() = Some(HubIds {
        hub: key,
        me: me.clone(),
        agents: members,
    });
    Own { me, agents }
}

pub(crate) fn mark(event: Event, own: &Own) -> Result<ChannelEvent> {
    let mut marked: ChannelEvent = convert(&event)?;
    marked.from_me = !own.me.is_empty() && event.author == own.me;
    marked.author_agent = own.agents.get(&event.author).cloned();
    marked.mentioned_agents = event
        .tags
        .iter()
        .filter(|t| t.len() == 2 && t[0] == MENTION_TAG)
        .filter_map(|t| own.agents.get(&t[1]).cloned())
        .collect();
    Ok(marked)
}

pub async fn list_channels() -> Result<Vec<ChannelSummary>> {
    convert(hub()?.channels().await?)
}

pub async fn read_channel(
    store: &Store,
    channel: &str,
    after: i64,
    agent: &Option<AgentId>,
) -> Result<Vec<ChannelEvent>> {
    let hub = hub()?;
    let member = agent_member(&hub, agent).await?;
    let events = hub.read(channel, after, member.as_deref()).await?;
    let own = own(store, &hub).await;
    events.into_iter().map(|e| mark(e, &own)).collect()
}

pub async fn post_to_channel(
    store: &Store,
    channel: &str,
    content: &str,
    agent: &Option<AgentId>,
) -> Result<ChannelEvent> {
    let hub = hub()?;
    let member = agent_member(&hub, agent).await?;
    let event = hub.post(channel, content, member.as_deref()).await?;
    mark(event, &own(store, &hub).await)
}

pub async fn post_channel_note(
    store: &Store,
    channel: &str,
    content: &str,
    agent: &Option<AgentId>,
) -> Result<ChannelEvent> {
    let hub = hub()?;
    let member = agent_member(&hub, agent).await?;
    let event = hub.post_note(channel, content, member.as_deref()).await?;
    mark(event, &own(store, &hub).await)
}

pub async fn channel_touched(channel: &str) -> Result<Vec<ChannelTouching>> {
    convert(hub()?.touched(channel).await?)
}

pub async fn channel_presence(channel: &str) -> Result<Vec<ChannelPresent>> {
    convert(hub()?.presence(channel).await?)
}

pub fn issue_key_in(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    let bytes = upper.as_bytes();
    (0..bytes.len()).find_map(|start| {
        if !bytes[start].is_ascii_uppercase() {
            return None;
        }
        let word_end = start
            + 1
            + bytes[start + 1..]
                .iter()
                .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                .count();
        let digits = bytes[word_end..].strip_prefix(b"-").map_or(0, |rest| {
            rest.iter().take_while(|b| b.is_ascii_digit()).count()
        });
        (word_end > start + 1 && digits > 0)
            .then(|| upper[start..word_end + 1 + digits].to_string())
    })
}

impl Daemon {
    pub async fn open_channel(
        &self,
        key: Option<String>,
        agent: Option<AgentId>,
    ) -> Result<String> {
        let key = match (key, agent) {
            (Some(key), _) => key,
            (None, Some(agent)) => {
                let agent = self
                    .store
                    .get_agent(&agent)?
                    .ok_or_else(|| anyhow!("unknown agent"))?;
                let branch = self
                    .store
                    .get_worktree(&agent.worktree_id)?
                    .ok_or_else(|| anyhow!("the agent has no worktree"))?
                    .branch;
                issue_key_in(&branch).unwrap_or(branch)
            }
            (None, None) => bail!("name a channel key, or run this inside a session"),
        };
        hub()?.create_channel(&key).await?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::{issue_key_in, mark, Own};
    use berth_core::AgentId;
    use berth_hub::Event;
    use std::collections::HashMap;

    #[test]
    fn marks_this_devices_author_and_agents_by_hub_id_not_by_name() {
        let own = Own {
            me: "m-me".into(),
            agents: HashMap::from([("a-mine".to_string(), AgentId("agent-1".into()))]),
        };
        let event = |author: &str| Event {
            seq: 1,
            kind: 1,
            author: author.into(),
            author_name: "fix login".into(),
            author_kind: "agent".into(),
            channel: "PROJ-1".into(),
            tags: vec![
                vec!["p".into(), "a-mine".into()],
                vec!["p".into(), "a-theirs".into()],
            ],
            content: "@fix-login look".into(),
            created_at: 0,
        };
        let mine = mark(event("a-mine"), &own).unwrap();
        assert_eq!(mine.author_agent, Some(AgentId("agent-1".into())));
        assert_eq!(mine.mentioned_agents, vec![AgentId("agent-1".into())]);
        assert!(!mine.from_me);
        let theirs = mark(event("a-theirs"), &own).unwrap();
        assert_eq!(theirs.author_agent, None);
        assert!(mark(event("m-me"), &own).unwrap().from_me);
    }

    #[test]
    fn finds_the_issue_key_the_project_tree_would() {
        assert_eq!(
            issue_key_in("feature/proj-12-orders").as_deref(),
            Some("PROJ-12")
        );
        assert_eq!(
            issue_key_in("fix the A-1 and ABC-34").as_deref(),
            Some("ABC-34")
        );
        assert_eq!(issue_key_in("main"), None);
        assert_eq!(issue_key_in("feature/login-form"), None);
    }
}
