pub mod conn;
pub mod map;
pub mod providers;
pub mod transcript;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use crate::registry::Daemon;
use crate::status::HookEvent;
use anyhow::{bail, Context, Result};
use berth_core::acp::{AcpItem, AcpItemAt};
use berth_core::{AgentId, AgentKind, EntityId, ServerEvent, WorktreeId};
use conn::{Conn, Inbound, RpcError};
use providers::AcpProvider;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use transcript::Transcript;

const ITEMS_KEPT_PER_AGENT: u64 = 5000;
const SESSION_READY_TIMEOUT: Duration = Duration::from_secs(180);
const ACP_PROTOCOL_VERSION: u64 = 1;
const METHOD_NOT_FOUND: i64 = -32601;
const AUTO_TITLE_CHARS: usize = 60;

#[derive(Default)]
pub struct AcpSessions {
    live: Mutex<HashMap<AgentId, Arc<AcpLive>>>,
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

struct PendingPermission {
    rpc_id: Value,
    option_ids: Vec<String>,
}

type SessionReady = Option<Result<String, String>>;

struct AcpLive {
    conn: Arc<Conn>,
    provider_name: String,
    ready: watch::Sender<SessionReady>,
    turn_open: AtomicBool,
    permission: Mutex<Option<PendingPermission>>,
    transcript: Mutex<Transcript>,
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
    ) -> Result<EntityId> {
        let config = crate::config::Config::load();
        let provider = providers::resolve_enabled(&config.providers, provider_id)?;
        self.create_acp_agent_with(worktree, name, provider, auto_title)
    }

    pub(crate) fn create_acp_agent_with(
        self: &Arc<Self>,
        worktree_id: WorktreeId,
        name: String,
        provider: AcpProvider,
        auto_title: bool,
    ) -> Result<EntityId> {
        let worktree = self
            .store
            .get_worktree(&worktree_id)?
            .context("worktree not found")?;
        let program = provider.program()?;
        let id = AgentId::generate();
        let name = match name.trim() {
            "" => provider.name.clone(),
            trimmed => trimmed.to_string(),
        };
        self.store
            .insert_acp_agent(&id, &worktree_id, &name, &provider.id, auto_title)?;
        if let Err(spawn_error) = self.acp_start(&id, &worktree.path, &provider, &program, None) {
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
        if agent.kind != AgentKind::Acp {
            bail!("not an ACP agent");
        }
        if agent.archived {
            bail!("agent is archived — unarchive it first");
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
        let program = provider.program()?;
        let live = self.acp_start(
            agent_id,
            &worktree.path,
            &provider,
            &program,
            agent.session_id,
        )?;
        self.try_broadcast_acp_agent(agent_id);
        Ok(live)
    }

    fn acp_start(
        self: &Arc<Self>,
        agent_id: &AgentId,
        cwd: &Path,
        provider: &AcpProvider,
        program: &Path,
        previous_session: Option<String>,
    ) -> Result<Arc<AcpLive>> {
        let mut map = self.acp.live.lock().unwrap();
        if let Some(live) = map.get(agent_id) {
            return Ok(live.clone());
        }
        let (conn, inbound) = Conn::spawn(program, &provider.args, cwd)?;
        let live = Arc::new(AcpLive {
            conn,
            provider_name: provider.name.clone(),
            ready: watch::channel(None).0,
            turn_open: AtomicBool::new(false),
            permission: Mutex::new(None),
            transcript: Mutex::new(Transcript::after(
                self.store.last_acp_seq(agent_id).unwrap_or(0),
            )),
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

    async fn acp_handshake(
        self: Arc<Self>,
        agent_id: AgentId,
        live: Arc<AcpLive>,
        cwd: PathBuf,
        previous_session: Option<String>,
    ) {
        let started = async {
            live.conn
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": ACP_PROTOCOL_VERSION,
                        "clientCapabilities": {
                            "fs": {"readTextFile": false, "writeTextFile": false},
                            "terminal": false
                        },
                        "clientInfo": {"name": "berth", "version": env!("CARGO_PKG_VERSION")}
                    }),
                )
                .await?;
            let created = live
                .conn
                .request("session/new", json!({"cwd": cwd, "mcpServers": []}))
                .await?;
            created
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or(RpcError::Remote {
                    code: 0,
                    message: "session/new returned no sessionId".into(),
                })
        }
        .await;
        match started {
            Ok(sid) => {
                if previous_session.is_some_and(|old| old != sid) {
                    self.acp_push(
                        &agent_id,
                        &live,
                        AcpItem::Notice {
                            text: "The previous conversation could not be resumed; this is a new session."
                                .into(),
                        },
                    );
                }
                self.apply_hook_event(
                    &agent_id,
                    HookEvent::SessionStart {
                        source: Some("acp".into()),
                    },
                    Some(sid.clone()),
                );
                live.ready.send_replace(Some(Ok(sid)));
            }
            Err(error) => {
                let message = format!("{} did not start: {error}", live.provider_name);
                tracing::warn!(agent = %agent_id, stderr = ?live.conn.stderr_tail(), "{message}");
                self.acp_push(
                    &agent_id,
                    &live,
                    AcpItem::Notice {
                        text: message.clone(),
                    },
                );
                live.ready.send_replace(Some(Err(message)));
                live.conn.kill();
            }
        }
    }

    async fn acp_inbound(
        self: Arc<Self>,
        agent_id: AgentId,
        live: Arc<AcpLive>,
        mut inbound: mpsc::UnboundedReceiver<Inbound>,
    ) {
        while let Some(message) = inbound.recv().await {
            match message {
                Inbound::Notification { method, params } if method == "session/update" => {
                    self.acp_update(&agent_id, &live, &params)
                }
                Inbound::Notification { .. } => {}
                Inbound::Request { id, method, params }
                    if method == "session/request_permission" =>
                {
                    self.acp_permission(&agent_id, &live, id, &params).await
                }
                Inbound::Request { id, method, .. } => {
                    tracing::debug!(agent = %agent_id, method, "ACP request berth does not serve");
                    let _ = live
                        .conn
                        .respond_error(id, METHOD_NOT_FOUND, "method not found")
                        .await;
                }
                Inbound::Exited(exit_code) => {
                    self.acp_exited(&agent_id, &live, exit_code);
                    break;
                }
            }
        }
    }

    fn acp_update(self: &Arc<Self>, agent_id: &AgentId, live: &AcpLive, params: &Value) {
        let update = map::session_update(params);
        if let map::Update::Title(title) = &update {
            let _ = self.adopt_cli_title(agent_id, title);
            return;
        }
        let items = live
            .transcript
            .lock()
            .unwrap()
            .apply(update, Instant::now());
        self.acp_emit(agent_id, items);
    }

    async fn acp_permission(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &AcpLive,
        rpc_id: Value,
        params: &Value,
    ) {
        let permission = map::permission(params);
        let tail = live.transcript.lock().unwrap().flush();
        self.acp_emit(agent_id, tail);
        let replaced = live.permission.lock().unwrap().replace(PendingPermission {
            rpc_id,
            option_ids: permission.option_ids,
        });
        if let Some(old) = replaced {
            let _ = live.conn.respond(old.rpc_id, cancelled_outcome()).await;
        }
        self.apply_hook_event(
            agent_id,
            HookEvent::PermissionRequest,
            live.current_session_id(),
        );
        self.set_feedback_request(agent_id, permission.request);
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
        let tail = live.transcript.lock().unwrap().flush();
        self.acp_emit(agent_id, tail);
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
            .is_some_and(|agent| agent.kind == AgentKind::Acp)
    }

    pub async fn acp_prompt(self: &Arc<Self>, agent_id: &AgentId, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            bail!("the prompt is empty");
        }
        let live = self.acp_ensure(agent_id)?;
        let sid = live.session_id().await?;
        if live.turn_open.swap(true, Ordering::SeqCst) {
            bail!("a turn is already running; wait for it or stop it");
        }
        self.checkpoint_for_prompt(agent_id).await;
        self.acp_title_from_prompt(agent_id, text);
        self.acp_push(agent_id, &live, AcpItem::User { text: text.into() });
        self.apply_hook_event(agent_id, HookEvent::UserPromptSubmit, Some(sid.clone()));
        let daemon = self.clone();
        let agent_id = agent_id.clone();
        let prompt = json!({"sessionId": sid, "prompt": [{"type": "text", "text": text}]});
        tokio::spawn(async move {
            let outcome = live.conn.request("session/prompt", prompt).await;
            daemon.acp_turn_ended(&agent_id, &live, sid, outcome);
        });
        Ok(())
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
        let title: String = first_line.chars().take(AUTO_TITLE_CHARS).collect();
        let _ = self.adopt_cli_title(agent_id, &title);
    }

    fn acp_turn_ended(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        sid: String,
        outcome: Result<Value, RpcError>,
    ) {
        let tail = live.transcript.lock().unwrap().flush();
        self.acp_emit(agent_id, tail);
        live.turn_open.store(false, Ordering::SeqCst);
        live.permission.lock().unwrap().take();
        let notice = match &outcome {
            Err(RpcError::Closed) => return,
            Err(error) => Some(format!("The turn failed: {error}")),
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
        self.apply_hook_event(agent_id, HookEvent::Stop, Some(sid));
    }

    pub async fn answer_feedback(self: &Arc<Self>, agent_id: &AgentId, option: u8) -> Result<()> {
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
        let Some(option_id) = pending.option_ids.get(option as usize).cloned() else {
            live.permission.lock().unwrap().replace(pending);
            bail!("no such option");
        };
        live.conn
            .respond(
                pending.rpc_id,
                json!({"outcome": {"outcome": "selected", "optionId": option_id}}),
            )
            .await?;
        self.apply_hook_event(
            agent_id,
            HookEvent::UserPromptSubmit,
            live.current_session_id(),
        );
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
        let _ = live
            .conn
            .notify("session/cancel", json!({"sessionId": sid}))
            .await;
        let pending = live.permission.lock().unwrap().take();
        if let Some(pending) = pending {
            let _ = live.conn.respond(pending.rpc_id, cancelled_outcome()).await;
        }
    }

    pub fn acp_transcript(&self, agent_id: &AgentId, after: u64) -> Result<Vec<AcpItemAt>> {
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

    fn acp_push(&self, agent_id: &AgentId, live: &AcpLive, item: AcpItem) {
        let items = live.transcript.lock().unwrap().push(item);
        self.acp_emit(agent_id, items);
    }

    fn acp_emit(&self, agent_id: &AgentId, items: Vec<AcpItemAt>) {
        for AcpItemAt { seq, item } in items {
            match serde_json::to_string(&item) {
                Ok(json) => {
                    if let Err(e) =
                        self.store
                            .put_acp_item(agent_id, seq, &json, ITEMS_KEPT_PER_AGENT)
                    {
                        tracing::warn!(agent = %agent_id, error = %e, "persist ACP item failed");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "encode ACP item failed"),
            }
            self.broadcast(ServerEvent::AcpItem {
                agent: agent_id.clone(),
                seq,
                item: Box::new(item),
            });
        }
    }

    fn try_broadcast_acp_agent(&self, agent_id: &AgentId) {
        let _ = self.broadcast_agent(agent_id);
    }
}

fn cancelled_outcome() -> Value {
    json!({"outcome": {"outcome": "cancelled"}})
}
