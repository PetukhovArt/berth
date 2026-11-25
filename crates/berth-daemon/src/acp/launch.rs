use crate::registry::Daemon;
use anyhow::{Context, Result};
use berth_core::acp::OptionCategory;
use berth_core::AgentId;
use std::sync::Arc;

#[derive(Default)]
pub struct ChatLaunch {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub prompt: Option<String>,
}

impl Daemon {
    pub(crate) fn acp_launch(self: &Arc<Self>, id: AgentId, launch: ChatLaunch) {
        let daemon = self.clone();
        tokio::spawn(async move {
            let mut skipped = Vec::new();
            for (label, category, value) in [
                ("model", OptionCategory::Model, launch.model),
                ("effort", OptionCategory::ThoughtLevel, launch.effort),
            ] {
                let Some(value) = value else { continue };
                if daemon
                    .acp_set_category(&id, category, &value)
                    .await
                    .is_err()
                {
                    skipped.push(format!("{label} `{value}`"));
                }
            }
            if !skipped.is_empty() {
                let notice = format!("This agent did not take its {}.", skipped.join(" and "));
                daemon.acp_notice(&id, notice);
            }
            if let Some(prompt) = launch.prompt {
                if let Err(error) = daemon.acp_prompt(&id, &prompt).await {
                    tracing::warn!(agent = %id, error = %error, "first prompt of a new chat failed");
                }
            }
        });
    }

    async fn acp_set_category(
        self: &Arc<Self>,
        id: &AgentId,
        category: OptionCategory,
        value: &str,
    ) -> Result<()> {
        let live = self.acp_ensure(id)?;
        live.session_id().await?;
        let option = live
            .state
            .lock()
            .unwrap()
            .options
            .iter()
            .find(|option| option.category == category)
            .map(|option| option.id.clone())
            .context("the agent has no such option")?;
        self.acp_set_option(id, &option, value).await
    }
}
