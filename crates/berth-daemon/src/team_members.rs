use std::collections::HashMap;

use berth_core::AgentId;
use berth_hub::client::HubClient;
use berth_hub::Mention;

use crate::config::Config;
use crate::registry::Daemon;
use crate::store::Store;

#[derive(Default)]
pub(crate) struct RegisteredAgents(HashMap<AgentId, String>);

#[derive(Default)]
pub(crate) struct MentionCursor {
    hub_url: String,
    seq: Option<i64>,
}

impl MentionCursor {
    pub(crate) async fn after(&mut self, store: &Store, hub: &HubClient) -> Option<i64> {
        let url = Config::load().hub_url.unwrap_or_default();
        if url != self.hub_url {
            self.hub_url = url;
            self.seq = None;
        }
        if self.seq.is_none() {
            self.seq = match store.mention_cursor(&self.hub_url).ok()? {
                Some(seq) => Some(seq),
                None => {
                    let channels = hub.channels().await.ok()?;
                    let head = channels.iter().map(|c| c.head).max().unwrap_or(0);
                    store.set_mention_cursor(&self.hub_url, head).ok()?;
                    Some(head)
                }
            };
        }
        self.seq
    }

    pub(crate) fn hub_url(&self) -> &str {
        &self.hub_url
    }

    pub(crate) fn queue(&mut self, store: &Store, found: &[Mention], seq: i64) {
        let seq = self.seq.map_or(seq, |s| s.max(seq));
        match crate::team_pending::queue_and_advance(store, &self.hub_url, found, seq) {
            Ok(()) => self.seq = Some(seq),
            Err(e) => tracing::warn!(error = %e, "queueing fetched mentions failed"),
        }
    }
}

impl Daemon {
    pub(crate) async fn register_agents(&self, hub: &HubClient, registered: &mut RegisteredAgents) {
        let Ok((_, _, agents, _)) = self.store.load_tree() else {
            return;
        };
        for agent in agents.iter().filter(|a| !a.archived) {
            if registered.0.get(&agent.id) == Some(&agent.name) {
                continue;
            }
            if let Err(e) = hub
                .register_agent(agent.id.as_str(), Some(&agent.name))
                .await
            {
                tracing::debug!(error = %e, "registering Agents on the hub failed");
                return;
            }
            registered.0.insert(agent.id.clone(), agent.name.clone());
        }
    }
}
