use std::sync::Arc;

use anyhow::{bail, Context, Result};
use berth_core::{AgentId, Backend, EntityId, SessionRef};

use crate::acp::{providers, ChatLaunch};
use crate::agent;
use crate::registry::{cli_missing_message, CreateAgentSpec, Daemon};

const BACK_IN_CHAT: &str =
    "Back in the chat. The turns taken in the terminal are not shown here, but the agent still has them.";

impl Daemon {
    pub async fn switch_backend(self: &Arc<Self>, id: &AgentId, to: Backend) -> Result<()> {
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if agent.backend == to {
            return Ok(());
        }
        if agent.archived {
            bail!("agent is archived — unarchive it first");
        }
        if agent.is_ended() {
            bail!("agent is ended");
        }
        let provider = match to {
            Backend::Terminal => {
                if agent::terminal(agent.kind).is_none() {
                    bail!("{} has no terminal", agent.kind.label());
                }
                if !self.cli_available_for_create(agent.kind).await {
                    bail!("{}", cli_missing_message(agent.kind));
                }
                None
            }
            Backend::Chat => {
                let id = agent.acp_provider.as_deref().unwrap_or(agent.kind.as_str());
                Some(providers::resolve_enabled(&crate::config::Config::load().providers, id)?.id)
            }
        };
        self.set_backend(id, to, provider.as_deref())
    }

    pub(crate) async fn create_agent_by_default(
        self: &Arc<Self>,
        spec: CreateAgentSpec,
    ) -> Result<EntityId> {
        let providers = crate::config::Config::load().providers;
        let Some(provider) = providers::default_chat(&providers, spec.kind) else {
            return self.create_agent(spec).await;
        };
        let launch = ChatLaunch {
            model: spec.model,
            effort: spec.effort,
            prompt: spec.starting_prompt,
        };
        self.create_acp_agent(
            spec.worktree,
            spec.name,
            &provider.id,
            spec.auto_title,
            launch,
        )
        .await
    }

    pub(crate) fn set_backend(
        &self,
        id: &AgentId,
        to: Backend,
        provider: Option<&str>,
    ) -> Result<()> {
        if to == Backend::Terminal {
            self.acp_drain_queue(id);
        }
        self.kill_session(&SessionRef::Agent(id.clone()));
        self.store.set_agent_backend(id, to, provider)?;
        if to == Backend::Chat {
            self.acp_notice(id, BACK_IN_CHAT.into());
        }
        self.broadcast_agent(id)
    }
}
