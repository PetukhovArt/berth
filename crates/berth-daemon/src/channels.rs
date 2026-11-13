use crate::config::Config;
use anyhow::{anyhow, Result};
use berth_core::{AgentId, ChannelEvent, ChannelPresent, ChannelSummary};
use berth_hub::client::HubClient;
use serde::{de::DeserializeOwned, Serialize};

pub(crate) fn hub() -> Result<HubClient> {
    let config = Config::load();
    let url = config
        .hub_url
        .ok_or_else(|| anyhow!("no team hub: set \"hub_url\" and \"hub_token\" in the config file"))?;
    let token = config
        .hub_token
        .ok_or_else(|| anyhow!("no hub token: set \"hub_token\" in the config file"))?;
    HubClient::new(&url, &token)
}

fn convert<T: Serialize, U: DeserializeOwned>(value: T) -> Result<U> {
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

pub async fn channel_presence(channel: &str) -> Result<Vec<ChannelPresent>> {
    convert(hub()?.presence(channel).await?)
}
