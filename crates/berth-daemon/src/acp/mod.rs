mod blobs;
mod chat;
pub mod conn;
mod driver;
mod fallback;
mod files;
mod latency;
mod launch;
mod lifecycle;
pub mod map;
mod mcp;
#[cfg(test)]
mod mock;
mod persist;
mod prompt;
pub mod providers;
mod questions;
mod relocate;
#[cfg(test)]
mod replay;
mod rewind;
mod shim;
mod slash;
#[cfg(test)]
mod tests;
pub mod transcript;
pub mod wire;

use crate::registry::Daemon;
use crate::status::HookEvent;
use anyhow::{bail, Context, Result};
use berth_core::acp::{AcpItem, AcpItemAt, AcpSessionState, FeedbackAnswer, QuestionForm};
use berth_core::WorktreeId;
use berth_core::{AgentId, Backend, EntityId, FeedbackKind, FeedbackRequest, ServerEvent};
use conn::{Conn, Inbound};
pub(crate) use launch::ChatLaunch;
use providers::AcpProvider;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio::time::sleep_until;
use transcript::Transcript;
use wire::{DaemonHost, Host};

const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(180);
const ACP_PROTOCOL_VERSION: u64 = 1;
const METHOD_NOT_FOUND: i64 = -32601;
const AUTO_TITLE_CHARS: usize = 60;

#[derive(Default)]
pub struct AcpSessions {
    live: Mutex<HashMap<AgentId, Arc<AcpLive>>>,
    blobs: blobs::Blobs,
    persist: persist::Persist,
    latency: latency::Latency,
    skip_terminal_spawn: AtomicBool,
}

impl AcpSessions {
    pub fn is_live(&self, agent: &AgentId) -> bool {
        self.live.lock().unwrap().contains_key(agent)
    }

    fn get(&self, agent: &AgentId) -> Option<Arc<AcpLive>> {
        self.live.lock().unwrap().get(agent).cloned()
    }

    fn remove_if_same(&self, agent: &AgentId, live: &Arc<AcpLive>) -> bool {
        let mut map = self.live.lock().unwrap();
        if map.get(agent).is_some_and(|l| Arc::ptr_eq(l, live)) {
            map.remove(agent);
            return true;
        }
        false
    }

    pub fn kill(&self, agent: &AgentId) {
        if let Some(live) = self.live.lock().unwrap().remove(agent) {
            live.conn.kill();
        }
    }

    pub fn kill_all(&self) {
        for (_, live) in self.live.lock().unwrap().drain() {
            live.conn.kill();
        }
    }
}

enum Pending {
    Permission {
        rpc_id: Value,
        option_ids: Vec<String>,
    },
    Question {
        rpc_id: Value,
        form: QuestionForm,
        reply: questions::Reply,
    },
    SignIn {
        method_ids: Vec<String>,
        cwd: PathBuf,
        previous_session: Option<String>,
    },
}

impl Pending {
    fn cancel_reply(&self) -> Option<(Value, Value)> {
        match self {
            Pending::Permission { rpc_id, .. } => Some((rpc_id.clone(), cancelled_outcome())),
            Pending::Question { rpc_id, reply, .. } => Some((rpc_id.clone(), reply.cancel())),
            Pending::SignIn { .. } => None,
        }
    }
}

type SessionReady = Option<Result<String, String>>;

struct AcpLive {
    conn: Arc<Conn>,
    provider_name: String,
    ready: watch::Sender<SessionReady>,
    turn_open: AtomicBool,
    permission: Mutex<Option<Pending>>,
    transcript: Mutex<Transcript>,
    capabilities: Mutex<lifecycle::Capabilities>,
    notifications_handled: AtomicU64,
    replay_until: AtomicU64,
    last_active: Mutex<Instant>,
    state: Mutex<AcpSessionState>,
    next_queue_id: AtomicU64,
    prompt_item: Mutex<Option<AcpItemAt>>,
}

impl AcpLive {
    async fn session_id(&self) -> Result<String> {
        let mut rx = self.ready.subscribe();
        let ready = tokio::time::timeout(SESSION_READY_TIMEOUT, rx.wait_for(Option::is_some))
            .await
            .with_context(|| format!("{} did not start in time", self.provider_name))?
            .context("agent connection dropped")?
            .clone();
        match ready {
            Some(Ok(sid)) => Ok(sid),
            Some(Err(e)) => bail!("{e}"),
            None => unreachable!("waited for Some"),
        }
    }

    fn touch(&self) {
        *self.last_active.lock().unwrap() = Instant::now();
    }

    fn current_session_id(&self) -> Option<String> {
        self.ready
            .borrow()
            .as_ref()
            .and_then(|r| r.as_ref().ok().cloned())
    }
}

impl Daemon {
    pub async fn create_acp_agent(
        self: &Arc<Self>,
        worktree: WorktreeId,
        name: String,
        provider_id: &str,
        auto_title: bool,
        launch: ChatLaunch,
    ) -> Result<EntityId> {
        let config = crate::config::Config::load();
        let provider = providers::resolve_enabled(&config.providers, provider_id)?;
        let id = AgentId::generate();
        let host = Arc::new(DaemonHost { agent: id.clone() });
        let created =
            self.create_acp_agent_with(id.clone(), worktree, name, provider, auto_title, host)?;
        self.acp_launch(id, launch);
        Ok(created)
    }

    pub(crate) fn create_acp_agent_with(
        self: &Arc<Self>,
        id: AgentId,
        worktree_id: WorktreeId,
        name: String,
        provider: AcpProvider,
        auto_title: bool,
        host: Arc<dyn Host>,
    ) -> Result<EntityId> {
        let worktree = self
            .store
            .get_worktree(&worktree_id)?
            .context("worktree not found")?;
        let name = match name.trim() {
            "" => provider.name.clone(),
            trimmed => trimmed.to_string(),
        };
        self.store
            .insert_acp_agent(&id, &worktree_id, &name, &provider.id, auto_title)?;
        if let Err(spawn_error) = self.acp_start(&id, &worktree.path, &provider, None, host) {
            let _ = self.store.delete_agent(&id);
            return Err(spawn_error);
        }
        self.broadcast_agent(&id)?;
        Ok(EntityId::Agent(id))
    }

    fn acp_ensure(self: &Arc<Self>, agent_id: &AgentId) -> Result<Arc<AcpLive>> {
        if let Some(live) = self.acp.get(agent_id) {
            return Ok(live);
        }
        let agent = self.store.get_agent(agent_id)?.context("agent not found")?;
        if agent.backend != Backend::Chat {
            bail!("not an ACP agent");
        }
        if agent.archived {
            bail!("agent is archived — unarchive it first");
        }
        if agent.is_ended() {
            bail!("agent is ended");
        }
        let provider_id = self
            .store
            .agent_acp_provider(agent_id)?
            .context("the agent has no provider")?;
        let provider = providers::resolve(&crate::config::Config::load().providers, &provider_id)
            .with_context(|| format!("unknown ACP provider `{provider_id}`"))?;
        let worktree = self
            .store
            .get_worktree(&agent.worktree_id)?
            .context("worktree not found")?;
        let host = Arc::new(DaemonHost {
            agent: agent_id.clone(),
        });
        let live = self.acp_start(agent_id, &worktree.path, &provider, agent.session_id, host)?;
        self.try_broadcast_acp_agent(agent_id);
        Ok(live)
    }

    fn acp_start(
        self: &Arc<Self>,
        agent_id: &AgentId,
        cwd: &Path,
        provider: &AcpProvider,
        previous_session: Option<String>,
        host: Arc<dyn Host>,
    ) -> Result<Arc<AcpLive>> {
        let mut map = self.acp.live.lock().unwrap();
        if let Some(live) = map.get(agent_id) {
            return Ok(live.clone());
        }
        let wire = driver::open(provider, cwd, host).with_context(|| provider.name.clone())?;
        let (conn, inbound) = Conn::over(wire);
        let live = Arc::new(AcpLive {
            conn,
            provider_name: provider.name.clone(),
            ready: watch::channel(None).0,
            turn_open: AtomicBool::new(false),
            permission: Mutex::new(None),
            transcript: Mutex::new(Transcript::after(
                self.store.last_acp_seq(agent_id).unwrap_or(0),
            )),
            capabilities: Default::default(),
            notifications_handled: AtomicU64::new(0),
            replay_until: AtomicU64::new(0),
            last_active: Mutex::new(Instant::now()),
            state: Default::default(),
            next_queue_id: AtomicU64::new(1),
            prompt_item: Default::default(),
        });
        map.insert(agent_id.clone(), live.clone());
        drop(map);
        tokio::spawn(
            self.clone()
                .acp_inbound(agent_id.clone(), live.clone(), inbound),
        );
        tokio::spawn(self.clone().acp_handshake(
            agent_id.clone(),
            live.clone(),
            cwd.to_path_buf(),
            previous_session,
        ));
        Ok(live)
    }

    async fn acp_inbound(
        self: Arc<Self>,
        agent_id: AgentId,
        live: Arc<AcpLive>,
        mut inbound: mpsc::UnboundedReceiver<Inbound>,
    ) {
        loop {
            let due = live.transcript.lock().unwrap().flush_due();
            let message = match due {
                Some(at) => tokio::select! {
                    message = inbound.recv() => message,
                    _ = sleep_until(at) => {
                        self.acp_flush_tail(&agent_id, &live);
                        continue;
                    }
                },
                None => inbound.recv().await,
            };
            let Some(message) = message else {
                break;
            };
            match message {
                Inbound::Notification {
                    method,
                    params,
                    read_at,
                } if method == "session/update" => {
                    if let Some(commands) = map::available_commands(&params) {
                        live.state.lock().unwrap().commands = commands;
                        self.acp_push_state(&agent_id, &live);
                    }
                    if let Some(options) = map::config_option_update(&params) {
                        live.state.lock().unwrap().options = options;
                        self.acp_push_state(&agent_id, &live);
                    }
                    let n = live.notifications_handled.fetch_add(1, Ordering::SeqCst) + 1;
                    if n > live.replay_until.load(Ordering::SeqCst) {
                        self.acp_update(&agent_id, &live, &params, read_at)
                    }
                }
                Inbound::Notification { .. } => {}
                Inbound::Request {
                    id, method, params, ..
                } if method == "session/request_permission" => {
                    self.acp_permission(&agent_id, &live, id, &params).await
                }
                Inbound::Request {
                    id, method, params, ..
                } if method == "elicitation/create" => {
                    let asked = map::elicitation(&params);
                    self.acp_question(&agent_id, &live, id, asked, questions::Reply::Elicitation)
                        .await
                }
                Inbound::Request {
                    id, method, params, ..
                } if method == questions::GROK_ASK_METHOD => {
                    let asked = questions::grok_form(&params);
                    self.acp_question(&agent_id, &live, id, asked, questions::Reply::Grok)
                        .await
                }
                Inbound::Request { id, method, .. } => {
                    tracing::debug!(agent = %agent_id, method, "ACP request berth does not serve");
                    let _ = live
                        .conn
                        .respond_error(id, METHOD_NOT_FOUND, "method not found")
                        .await;
                }
                Inbound::Response(response) => response.deliver(),
                Inbound::Exited(exit_code) => {
                    self.acp_exited(&agent_id, &live, exit_code);
                    break;
                }
            }
        }
    }

    fn acp_update(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &AcpLive,
        params: &Value,
        read_at: Instant,
    ) {
        let update = map::session_update(params);
        if let map::Update::Title(title) = &update {
            let _ = self.adopt_cli_title(agent_id, title);
            return;
        }
        let (items, stamp) = {
            let mut transcript = live.transcript.lock().unwrap();
            let items = transcript.apply(update, tokio::time::Instant::now(), read_at);
            let stamp = transcript.stamp(&items);
            (items, stamp)
        };
        self.acp_emit(agent_id, items, stamp);
    }

    fn acp_flush_tail(&self, agent_id: &AgentId, live: &AcpLive) {
        let (items, stamp) = {
            let mut transcript = live.transcript.lock().unwrap();
            let items = transcript.flush();
            let stamp = transcript.stamp(&items);
            (items, stamp)
        };
        self.acp_emit(agent_id, items, stamp);
    }

    async fn acp_permission(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &AcpLive,
        rpc_id: Value,
        params: &Value,
    ) {
        let permission = map::permission(params);
        let pending = Pending::Permission {
            rpc_id,
            option_ids: permission.option_ids,
        };
        self.acp_wait_for_human(agent_id, live, pending, permission.request)
            .await;
    }

    async fn acp_question(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &AcpLive,
        rpc_id: Value,
        asked: Option<(String, QuestionForm)>,
        reply: questions::Reply,
    ) {
        let Some((message, form)) = asked else {
            let _ = live.conn.respond(rpc_id, reply.cancel()).await;
            return;
        };
        let request = FeedbackRequest {
            kind: FeedbackKind::Question,
            title: message,
            detail: None,
            options: questions::option_names(&form),
            always_allow: false,
            form: Some(form.clone()),
        };
        let pending = Pending::Question {
            rpc_id,
            form,
            reply,
        };
        self.acp_wait_for_human(agent_id, live, pending, request)
            .await;
    }

    async fn acp_wait_for_human(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &AcpLive,
        pending: Pending,
        request: FeedbackRequest,
    ) {
        self.acp_flush_tail(agent_id, live);
        live.touch();
        let replaced = live.permission.lock().unwrap().replace(pending);
        if let Some((rpc_id, reply)) = replaced.as_ref().and_then(Pending::cancel_reply) {
            let _ = live.conn.respond(rpc_id, reply).await;
        }
        self.apply_hook_event(
            agent_id,
            HookEvent::PermissionRequest {
                tool_name: None,
                subagent_id: None,
                detail: None,
            },
            live.current_session_id(),
        );
        self.set_feedback_request(agent_id, request);
    }

    fn acp_exited(&self, agent_id: &AgentId, live: &Arc<AcpLive>, exit_code: Option<i32>) {
        live.permission.lock().unwrap().take();
        live.ready.send_if_modified(|ready| {
            let unanswered = ready.is_none();
            if unanswered {
                *ready = Some(Err("the agent exited".into()));
            }
            unanswered
        });
        self.acp_flush_tail(agent_id, live);
        self.acp.persist.flush(&self.store, agent_id);
        if !self.acp.remove_if_same(agent_id, live) && self.acp.is_live(agent_id) {
            return;
        }
        self.clear_feedback(agent_id);
        self.apply_hook_event(agent_id, HookEvent::SessionEnded { exit_code }, None);
        self.try_broadcast_acp_agent(agent_id);
    }

    pub(crate) fn is_acp_agent(&self, agent_id: &AgentId) -> bool {
        self.store
            .get_agent(agent_id)
            .ok()
            .flatten()
            .is_some_and(|agent| agent.backend == Backend::Chat)
    }

    pub async fn answer_feedback(
        self: &Arc<Self>,
        agent_id: &AgentId,
        answer: FeedbackAnswer,
    ) -> Result<()> {
        let live = self
            .acp
            .get(agent_id)
            .context("only a running ACP agent can be answered this way")?;
        let pending = live
            .permission
            .lock()
            .unwrap()
            .take()
            .context("the agent is not waiting for an answer")?;
        let reply = match &pending {
            Pending::SignIn { .. } => None,
            _ => match pending.reply_to(&answer) {
                Ok(reply) => Some(reply),
                Err(error) => {
                    live.permission.lock().unwrap().replace(pending);
                    return Err(error);
                }
            },
        };
        let Some((rpc_id, result)) = reply else {
            return self.acp_answer_sign_in(agent_id, live, pending, answer);
        };
        live.touch();
        live.conn.respond(rpc_id, result).await?;
        self.apply_hook_event(
            agent_id,
            HookEvent::UserPromptSubmit,
            live.current_session_id(),
        );
        Ok(())
    }

    fn acp_answer_sign_in(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: Arc<AcpLive>,
        pending: Pending,
        answer: FeedbackAnswer,
    ) -> Result<()> {
        let Pending::SignIn {
            method_ids,
            cwd,
            previous_session,
        } = pending
        else {
            bail!("not a sign-in");
        };
        let method_id = match answer {
            FeedbackAnswer::Option(option) => method_ids.get(option as usize).cloned(),
            FeedbackAnswer::Form(_) => None,
        };
        let Some(method_id) = method_id else {
            live.permission.lock().unwrap().replace(Pending::SignIn {
                method_ids,
                cwd,
                previous_session,
            });
            bail!("no such option");
        };
        self.clear_feedback(agent_id);
        live.ready.send_replace(None);
        tokio::spawn(self.clone().acp_sign_in(
            agent_id.clone(),
            live,
            method_id,
            cwd,
            previous_session,
        ));
        Ok(())
    }

    pub(crate) async fn acp_stop(self: &Arc<Self>, agent_id: &AgentId) {
        let Some(live) = self.acp.get(agent_id) else {
            return;
        };
        let Some(sid) = live
            .current_session_id()
            .filter(|_| live.turn_open.load(Ordering::SeqCst))
        else {
            self.acp.kill(agent_id);
            return;
        };
        let cleared = std::mem::take(&mut live.state.lock().unwrap().queue);
        self.acp_push_state(agent_id, &live);
        let texts: Vec<String> = cleared
            .into_iter()
            .map(|queued| queued.prompt.text)
            .filter(|text| !text.is_empty())
            .collect();
        if !texts.is_empty() {
            self.broadcast(ServerEvent::AcpQueueCleared {
                agent: agent_id.clone(),
                texts,
            });
        }
        let _ = live
            .conn
            .notify("session/cancel", json!({"sessionId": sid}))
            .await;
        let pending = live.permission.lock().unwrap().take();
        if let Some((rpc_id, reply)) = pending.as_ref().and_then(Pending::cancel_reply) {
            let _ = live.conn.respond(rpc_id, reply).await;
        }
    }

    pub fn acp_transcript(&self, agent_id: &AgentId, after: u64) -> Result<Vec<AcpItemAt>> {
        self.acp.persist.flush(&self.store, agent_id);
        self.store
            .acp_items_after(agent_id, after)?
            .into_iter()
            .map(|(seq, item)| {
                Ok(AcpItemAt {
                    seq,
                    item: serde_json::from_str(&item)?,
                })
            })
            .collect()
    }

    fn acp_push(&self, agent_id: &AgentId, live: &AcpLive, item: AcpItem) -> Option<AcpItemAt> {
        let items = live.transcript.lock().unwrap().push(item);
        let pushed = items.last().cloned();
        self.acp_emit(agent_id, items, None);
        pushed
    }

    fn acp_emit(&self, agent_id: &AgentId, items: Vec<AcpItemAt>, read_at: Option<Instant>) {
        let mut written = Vec::with_capacity(items.len());
        for AcpItemAt { seq, item } in items {
            match serde_json::to_string(&item) {
                Ok(json) => written.push((seq, json, is_open(&item))),
                Err(e) => tracing::warn!(error = %e, "encode ACP item failed"),
            }
            self.broadcast(ServerEvent::AcpItem {
                agent: agent_id.clone(),
                seq,
                item: Box::new(item),
            });
        }
        if let Some(read_at) = read_at {
            self.acp.latency.record(agent_id, read_at.elapsed());
        }
        for (seq, json, open) in written {
            self.acp.persist.put(&self.store, agent_id, seq, json, open);
        }
    }

    fn try_broadcast_acp_agent(&self, agent_id: &AgentId) {
        let _ = self.broadcast_agent(agent_id);
    }
}

impl Pending {
    fn reply_to(&self, answer: &FeedbackAnswer) -> Result<(Value, Value)> {
        match (self, answer) {
            (Pending::Permission { rpc_id, option_ids }, FeedbackAnswer::Option(option)) => {
                let option_id = option_ids.get(*option as usize).context("no such option")?;
                Ok((
                    rpc_id.clone(),
                    json!({"outcome": {"outcome": "selected", "optionId": option_id}}),
                ))
            }
            (
                Pending::Question {
                    rpc_id,
                    form,
                    reply,
                },
                answer,
            ) => {
                let answers = match answer {
                    FeedbackAnswer::Option(option) => {
                        questions::answers_for_option(form, *option as usize)?
                    }
                    FeedbackAnswer::Form(answers) => answers.clone(),
                };
                Ok((rpc_id.clone(), reply.accept(form, &answers)?))
            }
            _ => bail!("this request takes another kind of answer"),
        }
    }
}

fn is_open(item: &AcpItem) -> bool {
    match item {
        AcpItem::User { .. } | AcpItem::Assistant { .. } | AcpItem::Thought { .. } => true,
        AcpItem::Tool(tool) => !matches!(tool.status.as_str(), "completed" | "failed"),
        AcpItem::Plan { .. } | AcpItem::Notice { .. } => false,
    }
}

fn cancelled_outcome() -> Value {
    json!({"outcome": {"outcome": "cancelled"}})
}
