use crate::config::Config;
use crate::registry::Daemon;
use anyhow::{anyhow, bail, Result};
use berth_core::{AgentId, ChannelEvent, ChannelPresent, ChannelSummary, ChannelTouching};
use berth_hub::client::HubClient;
use serde::{de::DeserializeOwned, Serialize};

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

pub async fn list_channels() -> Result<Vec<ChannelSummary>> {
    convert(hub()?.channels().await?)
}

pub async fn read_channel(
    channel: &str,
    after: i64,
    agent: &Option<AgentId>,
) -> Result<Vec<ChannelEvent>> {
    let hub = hub()?;
    let member = agent_member(&hub, agent).await?;
    convert(hub.read(channel, after, member.as_deref()).await?)
}

pub async fn post_to_channel(
    channel: &str,
    content: &str,
    agent: &Option<AgentId>,
) -> Result<ChannelEvent> {
    let hub = hub()?;
    let member = agent_member(&hub, agent).await?;
    convert(hub.post(channel, content, member.as_deref()).await?)
}

pub async fn post_channel_note(
    channel: &str,
    content: &str,
    agent: &Option<AgentId>,
) -> Result<ChannelEvent> {
    let hub = hub()?;
    let member = agent_member(&hub, agent).await?;
    convert(hub.post_note(channel, content, member.as_deref()).await?)
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
    use super::issue_key_in;

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
