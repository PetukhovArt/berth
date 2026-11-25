use super::driver::REWIND;
use super::AcpLive;
use crate::registry::Daemon;
use crate::status::HookEvent;
use anyhow::{anyhow, ensure, Context, Result};
use berth_core::acp::AcpItem;
use berth_core::AgentId;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl Daemon {
    pub async fn acp_rewind(
        self: &Arc<Self>,
        agent_id: &AgentId,
        to: Option<String>,
    ) -> Result<()> {
        let live = self.acp_ensure(agent_id)?;
        let sid = live.session_id().await?;
        ensure!(
            live.state.lock().unwrap().caps.rewind,
            "this agent cannot rewind"
        );
        ensure!(
            !live.turn_open.load(Ordering::SeqCst),
            "wait for the turn to end"
        );
        let reply = live
            .conn
            .request(REWIND, json!({"sessionId": sid, "to": to}))
            .await
            .map_err(|error| anyhow!("{error}"))?;
        let forked = reply
            .get("sessionId")
            .and_then(Value::as_str)
            .context("the agent returned no session id")?
            .to_string();
        self.acp_adopt_session(agent_id, &live, forked);
        Ok(())
    }

    fn acp_adopt_session(&self, agent_id: &AgentId, live: &AcpLive, session: String) {
        live.ready.send_replace(Some(Ok(session.clone())));
        self.apply_hook_event(
            agent_id,
            HookEvent::SessionStart {
                source: Some("acp".into()),
            },
            Some(session),
        );
        self.acp_push(
            agent_id,
            live,
            AcpItem::Notice {
                text: "The conversation continues from an earlier message.".into(),
            },
        );
    }
}
