use super::conn::RpcError;
use super::driver::REWIND;
use super::AcpLive;
use crate::registry::Daemon;
use crate::status::HookEvent;
use anyhow::{anyhow, bail, ensure, Context, Result};
use berth_core::acp::{AcpItem, Rewind};
use berth_core::{AgentId, ServerEvent};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl Daemon {
    pub async fn acp_rewind(self: &Arc<Self>, agent_id: &AgentId, seq: u64) -> Result<()> {
        let (checkpoint, to) = self.acp_rewind_target(agent_id, seq)?;
        let live = self.acp_ensure(agent_id)?;
        ensure!(
            live.state.lock().unwrap().caps.rewind,
            "this agent cannot rewind"
        );
        ensure!(
            !live.turn_open.load(Ordering::SeqCst),
            "wait for the turn to end"
        );
        if let Some(number) = checkpoint {
            self.restore_checkpoint_files(agent_id, number).await?;
        }
        self.acp_rewind_session(agent_id, &live, to).await?;
        self.acp.persist.flush(&self.store, agent_id);
        self.store.delete_acp_items_from(agent_id, seq)?;
        self.broadcast(ServerEvent::AcpTruncated {
            agent: agent_id.clone(),
            from_seq: seq,
        });
        Ok(())
    }

    fn acp_rewind_target(
        &self,
        agent_id: &AgentId,
        seq: u64,
    ) -> Result<(Option<u32>, Option<String>)> {
        let items = self.acp_transcript(agent_id, 0)?;
        let mut users = items
            .iter()
            .filter_map(|at| match &at.item {
                AcpItem::User { rewind, .. } => Some((at.seq, rewind.as_ref())),
                _ => None,
            })
            .rev()
            .skip_while(|(at, _)| *at > seq);
        let Some((found, Some(rewind))) = users.next() else {
            bail!("this message cannot be rewound to");
        };
        ensure!(found == seq, "this message cannot be rewound to");
        let to = match users.next() {
            None => None,
            Some((
                _,
                Some(Rewind {
                    after: Some(after), ..
                }),
            )) => Some(after.clone()),
            Some(_) => bail!("the message before it has no rewind point"),
        };
        Ok((rewind.checkpoint, to))
    }

    async fn acp_rewind_session(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        to: Option<String>,
    ) -> Result<()> {
        let sid = live.session_id().await?;
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
        live.ready.send_replace(Some(Ok(forked.clone())));
        self.apply_hook_event(
            agent_id,
            HookEvent::SessionStart {
                source: Some("acp".into()),
            },
            Some(forked),
        );
        Ok(())
    }

    pub(super) fn acp_mark_rewind_point(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        outcome: &Result<Value, RpcError>,
    ) {
        let Some(mut marked) = live.prompt_item.lock().unwrap().take() else {
            return;
        };
        let after = outcome
            .as_ref()
            .ok()
            .and_then(|reply| reply.pointer("/_meta/berth/rewindPoint"))
            .and_then(Value::as_str);
        let (
            Some(after),
            AcpItem::User {
                rewind: Some(rewind),
                ..
            },
        ) = (after, &mut marked.item)
        else {
            return;
        };
        rewind.after = Some(after.to_string());
        self.acp_emit(agent_id, vec![marked], None);
    }
}
