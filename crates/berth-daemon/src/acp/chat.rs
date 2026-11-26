use super::conn::RpcError;
use super::driver::{INCOMPATIBLE, STEER};
use super::{files, map, prompt, AcpLive, AUTO_TITLE_CHARS};
use crate::registry::{sanitize_title, Daemon};
use crate::status::HookEvent;
use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use berth_core::acp::{
    AcpItem, AcpSessionState, AttachmentUpload, Choice, OptionCategory, PromptInput, QueueId,
    QueuedPrompt, Rewind,
};
use berth_core::{AgentId, ServerEvent, WorktreeId};
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn remembered_choice<'a>(
    category: OptionCategory,
    choices: &'a [Choice],
    wanted: &'a str,
) -> Option<&'a str> {
    choices
        .iter()
        .find(|choice| choice.value == wanted)
        .map(|choice| choice.value.as_str())
        .or((category == OptionCategory::Model).then_some(wanted))
}

impl Daemon {
    pub async fn acp_prompt(self: &Arc<Self>, agent_id: &AgentId, text: &str) -> Result<()> {
        self.acp_prompt_with(agent_id, text, Vec::new()).await
    }

    pub async fn acp_prompt_with(
        self: &Arc<Self>,
        agent_id: &AgentId,
        text: &str,
        uploads: Vec<AttachmentUpload>,
    ) -> Result<()> {
        self.acp_submit(agent_id, text, uploads, false).await
    }

    pub(crate) async fn acp_instruct(
        self: &Arc<Self>,
        agent_id: &AgentId,
        text: &str,
    ) -> Result<()> {
        self.acp_submit(agent_id, text, Vec::new(), true).await
    }

    async fn acp_submit(
        self: &Arc<Self>,
        agent_id: &AgentId,
        text: &str,
        uploads: Vec<AttachmentUpload>,
        notice: bool,
    ) -> Result<()> {
        let live = self.acp_ensure(agent_id)?;
        let sid = live.session_id().await?;
        live.touch();
        if !notice && self.acp_slash(agent_id, &live, text).await? {
            return Ok(());
        }
        let worktree = self.checkpoint_dir(agent_id)?;
        let input = PromptInput {
            text: text.trim().into(),
            attachments: prompt::store(&self.acp.blobs, &worktree, uploads).await?,
        };
        if input.text.is_empty() && input.attachments.is_empty() {
            bail!("the prompt is empty");
        }
        let blocks = self.acp_blocks(agent_id, &live, &input).await?;
        let queued = {
            let mut state = live.state.lock().unwrap();
            let running = live.turn_open.swap(true, Ordering::SeqCst);
            if running {
                let id = QueueId(live.next_queue_id.fetch_add(1, Ordering::SeqCst));
                state.queue.push(QueuedPrompt {
                    id,
                    prompt: input.clone(),
                });
            }
            running
        };
        if queued {
            self.acp_push_state(agent_id, &live);
            return Ok(());
        }
        self.acp_run_turn(agent_id, &live, sid, input, blocks, notice)
            .await;
        Ok(())
    }

    async fn acp_blocks(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        input: &PromptInput,
    ) -> Result<Vec<Value>> {
        let worktree = self.checkpoint_dir(agent_id)?;
        let caps = live.state.lock().unwrap().caps.clone();
        prompt::blocks(input, &caps, &worktree, &self.acp.blobs).await
    }

    fn acp_run_turn(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &Arc<AcpLive>,
        sid: String,
        input: PromptInput,
        blocks: Vec<Value>,
        notice: bool,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let daemon = self.clone();
        let agent_id = agent_id.clone();
        let live = live.clone();
        Box::pin(async move {
            let prompt = input.text.clone();
            let item = if notice {
                AcpItem::Notice { text: input.text }
            } else {
                let checkpoint = daemon.checkpoint_for_prompt(&agent_id).await;
                daemon.acp_title_from_prompt(&agent_id, &input.text);
                AcpItem::User {
                    text: input.text,
                    attachments: input.attachments,
                    rewind: Some(Rewind {
                        checkpoint,
                        after: None,
                    }),
                }
            };
            let pushed = daemon.acp_push(&agent_id, &live, item);
            *live.prompt_item.lock().unwrap() = pushed;
            daemon.apply_hook_event(&agent_id, HookEvent::UserPromptSubmit, Some(sid.clone()));
            let params = json!({"sessionId": sid, "prompt": blocks});
            tokio::spawn(async move {
                let outcome = live.conn.request("session/prompt", params).await;
                daemon
                    .acp_turn_ended(&agent_id, &live, sid, &prompt, outcome)
                    .await;
            });
        })
    }

    fn acp_title_from_prompt(self: &Arc<Self>, agent_id: &AgentId, text: &str) {
        if !self
            .store
            .agent_auto_title_pending(agent_id)
            .unwrap_or(false)
        {
            return;
        }
        let first_line = text.lines().next().unwrap_or_default();
        let title = sanitize_title(
            &first_line
                .chars()
                .take(AUTO_TITLE_CHARS)
                .collect::<String>(),
        );
        if !title.is_empty()
            && self
                .store
                .set_prompt_title(agent_id, &title)
                .unwrap_or(false)
        {
            let _ = self.broadcast_agent(agent_id);
        }
    }

    async fn acp_turn_ended(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &Arc<AcpLive>,
        sid: String,
        prompt: &str,
        outcome: Result<Value, RpcError>,
    ) {
        self.acp_flush_tail(agent_id, live);
        self.acp_mark_rewind_point(agent_id, live, &outcome);
        self.acp.persist.flush(&self.store, agent_id);
        live.touch();
        live.permission.lock().unwrap().take();
        if let Err(RpcError::Remote {
            code: INCOMPATIBLE,
            message,
        }) = &outcome
        {
            if self.acp_fall_back(agent_id, prompt, message) {
                return;
            }
        }
        let notice = match &outcome {
            Err(RpcError::Closed) => {
                live.state.lock().unwrap().queue.clear();
                self.acp_push_state(agent_id, live);
                live.turn_open.store(false, Ordering::SeqCst);
                return;
            }
            Err(error) => {
                tracing::debug!(agent = %agent_id, error = ?error, "turn failed");
                Some(format!("The turn failed: {error}"))
            }
            Ok(result) => match result.get("stopReason").and_then(Value::as_str) {
                Some("end_turn") | Some("cancelled") => None,
                Some("refusal") => Some("The agent refused to continue.".into()),
                Some("max_tokens") => Some("The turn hit the token limit.".into()),
                Some("max_turn_requests") => Some("The turn hit its request limit.".into()),
                other => other.map(|r| format!("The turn stopped: {r}")),
            },
        };
        if let Some(text) = notice {
            self.acp_push(agent_id, live, AcpItem::Notice { text });
        }
        self.apply_hook_event(agent_id, HookEvent::Stop, Some(sid.clone()));
        self.complete_pending_move(agent_id, &HookEvent::Stop);
        self.complete_pending_end(agent_id, &HookEvent::Stop);
        self.acp_start_queued(agent_id, live, sid).await;
    }

    async fn acp_start_queued(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &Arc<AcpLive>,
        sid: String,
    ) {
        loop {
            let next = {
                let mut state = live.state.lock().unwrap();
                if state.queue.is_empty() {
                    live.turn_open.store(false, Ordering::SeqCst);
                    return;
                }
                state.queue.remove(0)
            };
            self.acp_push_state(agent_id, live);
            match self.acp_blocks(agent_id, live, &next.prompt).await {
                Ok(blocks) => {
                    self.acp_run_turn(agent_id, live, sid, next.prompt, blocks, false)
                        .await;
                    return;
                }
                Err(error) => {
                    self.acp_push(
                        agent_id,
                        live,
                        AcpItem::Notice {
                            text: format!("A queued message was not sent: {error:#}"),
                        },
                    );
                }
            }
        }
    }

    pub fn acp_edit_queued(&self, agent_id: &AgentId, id: QueueId, text: &str) -> Result<()> {
        let live = self.acp.get(agent_id).context("the agent is not running")?;
        {
            let mut state = live.state.lock().unwrap();
            let queued = state
                .queue
                .iter_mut()
                .find(|queued| queued.id == id)
                .context("that message was already sent")?;
            let text = text.trim();
            if text.is_empty() && queued.prompt.attachments.is_empty() {
                bail!("the prompt is empty");
            }
            queued.prompt.text = text.into();
        }
        self.acp_push_state(agent_id, &live);
        Ok(())
    }

    pub fn acp_remove_queued(&self, agent_id: &AgentId, id: QueueId) -> Result<()> {
        let live = self.acp.get(agent_id).context("the agent is not running")?;
        {
            let mut state = live.state.lock().unwrap();
            let before = state.queue.len();
            state.queue.retain(|queued| queued.id != id);
            if state.queue.len() == before {
                bail!("that message was already sent");
            }
        }
        self.acp_push_state(agent_id, &live);
        Ok(())
    }

    pub async fn acp_send_queued_now(
        self: &Arc<Self>,
        agent_id: &AgentId,
        id: QueueId,
    ) -> Result<()> {
        let live = self.acp.get(agent_id).context("the agent is not running")?;
        ensure!(
            live.state.lock().unwrap().caps.steer,
            "this agent cannot take a message during a turn"
        );
        let sid = live.session_id().await?;
        let (at, queued) = {
            let mut state = live.state.lock().unwrap();
            ensure!(
                live.turn_open.load(Ordering::SeqCst),
                "the turn already ended"
            );
            let at = state
                .queue
                .iter()
                .position(|queued| queued.id == id)
                .context("that message was already sent")?;
            (at, state.queue.remove(at))
        };
        self.acp_push_state(agent_id, &live);
        let steered = async {
            let prompt = self.acp_blocks(agent_id, &live, &queued.prompt).await?;
            live.conn
                .request(STEER, json!({"sessionId": sid, "prompt": prompt}))
                .await
                .map_err(|error| anyhow!("{error}"))
        }
        .await;
        match steered {
            Ok(_) => {
                self.acp_push(
                    agent_id,
                    &live,
                    AcpItem::User {
                        text: queued.prompt.text,
                        attachments: queued.prompt.attachments,
                        rewind: None,
                    },
                );
                Ok(())
            }
            Err(error) => {
                {
                    let mut state = live.state.lock().unwrap();
                    let at = at.min(state.queue.len());
                    state.queue.insert(at, queued);
                }
                self.acp_push_state(agent_id, &live);
                Err(error)
            }
        }
    }

    pub async fn acp_set_option(
        self: &Arc<Self>,
        agent_id: &AgentId,
        id: &str,
        value: &str,
    ) -> Result<()> {
        let live = self.acp_ensure(agent_id)?;
        let sid = live.session_id().await?;
        let category = live
            .state
            .lock()
            .unwrap()
            .options
            .iter()
            .find(|option| option.id == id)
            .map(|option| option.category)
            .context("the agent has no such option")?;
        self.acp_request_option(agent_id, &live, &sid, id, value)
            .await
            .map_err(|error| anyhow!("{error}"))?;
        match category {
            OptionCategory::Model => self.store.set_agent_model(agent_id, Some(value))?,
            OptionCategory::ThoughtLevel => self.store.set_agent_effort(agent_id, Some(value))?,
            OptionCategory::Mode | OptionCategory::Other => {}
        }
        Ok(())
    }

    async fn acp_request_option(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        sid: &str,
        id: &str,
        value: &str,
    ) -> Result<(), RpcError> {
        let reply = live
            .conn
            .request(
                "session/set_config_option",
                json!({"sessionId": sid, "configId": id, "value": value}),
            )
            .await?;
        {
            let mut state = live.state.lock().unwrap();
            match map::config_options(&reply) {
                Some(options) => state.options = options,
                None => {
                    if let Some(option) = state.options.iter_mut().find(|o| o.id == id) {
                        option.current = value.into();
                    }
                }
            }
        }
        self.acp_push_state(agent_id, live);
        Ok(())
    }

    pub(super) async fn acp_apply_remembered(&self, agent_id: &AgentId, live: &AcpLive, sid: &str) {
        let Ok(Some(row)) = self.store.get_agent(agent_id) else {
            return;
        };
        for (category, wanted) in [
            (OptionCategory::Model, row.model),
            (OptionCategory::ThoughtLevel, row.effort),
        ] {
            let Some(wanted) = wanted else {
                continue;
            };
            let found = live
                .state
                .lock()
                .unwrap()
                .options
                .iter()
                .filter(|option| option.category == category)
                .find_map(|option| {
                    let value = remembered_choice(category, &option.choices, &wanted)?;
                    (option.current != value).then(|| (option.id.clone(), value.to_string()))
                });
            let Some((id, value)) = found else {
                continue;
            };
            if let Err(error) = self
                .acp_request_option(agent_id, live, sid, &id, &value)
                .await
            {
                tracing::debug!(agent = %agent_id, error = %error, "remembered option not applied");
            }
        }
    }

    pub(super) fn acp_push_state(&self, agent_id: &AgentId, live: &AcpLive) {
        let state = live.state.lock().unwrap().clone();
        self.broadcast(ServerEvent::AcpSessionChanged {
            agent: agent_id.clone(),
            state: Box::new(state),
        });
    }

    pub fn acp_session(&self, agent_id: &AgentId) -> AcpSessionState {
        self.acp
            .get(agent_id)
            .map(|live| live.state.lock().unwrap().clone())
            .unwrap_or_default()
    }

    pub async fn acp_attachment(&self, blob: &str) -> Result<String> {
        Ok(STANDARD.encode(self.acp.blobs.get(blob).await?))
    }

    pub async fn list_worktree_files(
        &self,
        worktree: &WorktreeId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let worktree = self
            .store
            .get_worktree(worktree)?
            .context("worktree not found")?;
        files::list(&worktree.path, query, limit).await
    }
}

#[cfg(test)]
mod remembered_tests {
    use super::*;

    fn choice(value: &str) -> Choice {
        Choice {
            value: value.into(),
            name: value.into(),
            description: None,
        }
    }

    #[test]
    fn a_remembered_model_keeps_its_exact_id() {
        let choices = [choice("default"), choice("haiku")];
        let model = OptionCategory::Model;
        assert_eq!(remembered_choice(model, &choices, "haiku"), Some("haiku"));
        assert_eq!(
            remembered_choice(model, &choices, "claude-haiku-4-5-20251001"),
            Some("claude-haiku-4-5-20251001")
        );
    }

    #[test]
    fn an_unknown_effort_is_not_applied() {
        let choices = [choice("low"), choice("high")];
        let effort = OptionCategory::ThoughtLevel;
        assert_eq!(remembered_choice(effort, &choices, "high"), Some("high"));
        assert_eq!(remembered_choice(effort, &choices, "max"), None);
    }
}
