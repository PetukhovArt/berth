use super::conn::RpcError;
use super::{AcpLive, Pending, ACP_PROTOCOL_VERSION};
use crate::registry::Daemon;
use crate::status::HookEvent;
use berth_core::acp::{AcpItem, AcpItemAt};
use berth_core::protocol::{FeedbackKind, FeedbackRequest};
use berth_core::{AgentId, AgentStatus};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

const AUTH_REQUIRED_CODE: i64 = -32000;
const STDERR_LINES_IN_NOTICE: usize = 8;

#[derive(Debug, Clone, Default)]
pub(super) struct Capabilities {
    resume: bool,
    load: bool,
    auth_methods: Vec<(String, String)>,
}

impl Capabilities {
    fn from_initialize(result: &Value) -> Self {
        let caps = result.get("agentCapabilities");
        let resume = caps
            .and_then(|c| c.get("sessionCapabilities"))
            .and_then(|s| s.get("resume"))
            .is_some_and(|r| !r.is_null() && r != &Value::Bool(false));
        let load = caps
            .and_then(|c| c.get("loadSession"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let auth_methods = result
            .get("authMethods")
            .and_then(Value::as_array)
            .map(|methods| {
                methods
                    .iter()
                    .filter_map(|m| {
                        let id = m.get("id").and_then(Value::as_str)?;
                        let name = m.get("name").and_then(Value::as_str).unwrap_or(id);
                        Some((id.to_string(), name.to_string()))
                    })
                    .take(crate::feedback::MAX_OPTIONS)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            resume,
            load,
            auth_methods,
        }
    }
}

enum Opened {
    Session(String),
    NeedsSignIn,
}

impl Daemon {
    pub(super) async fn acp_handshake(
        self: Arc<Self>,
        agent_id: AgentId,
        live: Arc<AcpLive>,
        cwd: PathBuf,
        previous_session: Option<String>,
    ) {
        let initialized = live
            .conn
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
            .await;
        let opened = match initialized {
            Ok(result) => {
                *live.capabilities.lock().unwrap() = Capabilities::from_initialize(&result);
                self.acp_open_session(&agent_id, &live, &cwd, previous_session.as_deref())
                    .await
            }
            Err(error) => Err(error),
        };
        self.acp_settle_session(agent_id, live, cwd, previous_session, opened);
    }

    fn acp_settle_session(
        &self,
        agent_id: AgentId,
        live: Arc<AcpLive>,
        cwd: PathBuf,
        previous_session: Option<String>,
        opened: Result<Opened, RpcError>,
    ) {
        match opened {
            Ok(Opened::Session(sid)) => {
                self.apply_hook_event(
                    &agent_id,
                    HookEvent::SessionStart {
                        source: Some("acp".into()),
                    },
                    Some(sid.clone()),
                );
                live.ready.send_replace(Some(Ok(sid)));
            }
            Ok(Opened::NeedsSignIn) => {
                self.acp_ask_sign_in(&agent_id, &live, cwd, previous_session)
            }
            Err(error) => {
                let mut message = format!("{} did not start: {error}", live.provider_name);
                let tail = live.conn.stderr_tail();
                tracing::warn!(agent = %agent_id, stderr = ?tail, "{message}");
                let shown = &tail[tail.len().saturating_sub(STDERR_LINES_IN_NOTICE)..];
                if !shown.is_empty() {
                    message.push_str("\n\n");
                    message.push_str(&shown.join("\n"));
                }
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

    async fn acp_open_session(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        cwd: &Path,
        previous_session: Option<&str>,
    ) -> Result<Opened, RpcError> {
        let caps = live.capabilities.lock().unwrap().clone();
        let params = |sid: &str| json!({"sessionId": sid, "cwd": cwd, "mcpServers": []});
        if let Some(old) = previous_session {
            if caps.resume {
                match live.conn.request("session/resume", params(old)).await {
                    Ok(_) => return Ok(Opened::Session(old.to_string())),
                    Err(e) if needs_sign_in(&e, &caps) => return Ok(Opened::NeedsSignIn),
                    Err(RpcError::Closed) => return Err(RpcError::Closed),
                    Err(e) => {
                        tracing::debug!(agent = %agent_id, error = %e, "session/resume failed")
                    }
                }
            }
            if caps.load {
                live.replay_until.store(u64::MAX, Ordering::SeqCst);
                let loaded = live.conn.request("session/load", params(old)).await;
                live.replay_until
                    .store(live.conn.notifications_seen(), Ordering::SeqCst);
                match loaded {
                    Ok(_) => return Ok(Opened::Session(old.to_string())),
                    Err(e) if needs_sign_in(&e, &caps) => return Ok(Opened::NeedsSignIn),
                    Err(RpcError::Closed) => return Err(RpcError::Closed),
                    Err(e) => {
                        tracing::debug!(agent = %agent_id, error = %e, "session/load failed")
                    }
                }
            }
        }
        let created = match live
            .conn
            .request("session/new", json!({"cwd": cwd, "mcpServers": []}))
            .await
        {
            Ok(created) => created,
            Err(e) if needs_sign_in(&e, &caps) => return Ok(Opened::NeedsSignIn),
            Err(e) => return Err(e),
        };
        let sid = created
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(RpcError::Remote {
                code: 0,
                message: "session/new returned no sessionId".into(),
            })?;
        if previous_session.is_some_and(|old| old != sid) {
            self.acp_push(
                agent_id,
                live,
                AcpItem::Notice {
                    text: "The previous conversation could not be resumed; this is a new session."
                        .into(),
                },
            );
        }
        Ok(Opened::Session(sid))
    }

    fn acp_ask_sign_in(
        &self,
        agent_id: &AgentId,
        live: &AcpLive,
        cwd: PathBuf,
        previous_session: Option<String>,
    ) {
        let methods = live.capabilities.lock().unwrap().auth_methods.clone();
        let (method_ids, names): (Vec<String>, Vec<String>) = methods.into_iter().unzip();
        let message = format!("{} needs you to sign in.", live.provider_name);
        live.ready.send_replace(Some(Err(format!(
            "{message} Answer the sign-in card first."
        ))));
        *live.permission.lock().unwrap() = Some(Pending::SignIn {
            method_ids,
            cwd,
            previous_session,
        });
        self.acp_push(
            agent_id,
            live,
            AcpItem::Notice {
                text: message.clone(),
            },
        );
        self.apply_hook_event(agent_id, HookEvent::PermissionRequest, None);
        self.set_feedback_request(
            agent_id,
            FeedbackRequest {
                kind: FeedbackKind::Question,
                title: format!("Sign in to {}", live.provider_name),
                detail: Some(
                    "The agent opens its own sign-in flow, usually in the browser.".into(),
                ),
                options: names,
            },
        );
    }

    pub(super) async fn acp_sign_in(
        self: Arc<Self>,
        agent_id: AgentId,
        live: Arc<AcpLive>,
        method_id: String,
        cwd: PathBuf,
        previous_session: Option<String>,
    ) {
        let opened = match live
            .conn
            .request("authenticate", json!({"methodId": method_id}))
            .await
        {
            Ok(_) => {
                self.acp_open_session(&agent_id, &live, &cwd, previous_session.as_deref())
                    .await
            }
            Err(RpcError::Closed) => Err(RpcError::Closed),
            Err(e) => {
                self.acp_push(
                    &agent_id,
                    &live,
                    AcpItem::Notice {
                        text: format!("Sign-in failed: {e}"),
                    },
                );
                Ok(Opened::NeedsSignIn)
            }
        };
        self.acp_settle_session(agent_id, live, cwd, previous_session, opened);
    }

    pub(crate) fn reap_idle_acp(&self, timeout: Duration) {
        let now = Instant::now();
        let idle: Vec<AgentId> = self
            .acp
            .live
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, live)| {
                !live.turn_open.load(Ordering::SeqCst)
                    && live.permission.lock().unwrap().is_none()
                    && now.duration_since(*live.last_active.lock().unwrap()) >= timeout
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in idle {
            let busy = self
                .store
                .get_agent(&id)
                .ok()
                .flatten()
                .is_none_or(|agent| {
                    matches!(
                        agent.status,
                        AgentStatus::Running | AgentStatus::NeedsFeedback
                    )
                });
            if busy {
                continue;
            }
            tracing::info!(agent = %id, "reaping idle ACP agent");
            self.acp.kill(&id);
        }
    }

    pub(crate) fn live_acp_agents(&self) -> Vec<AgentId> {
        self.acp.live.lock().unwrap().keys().cloned().collect()
    }

    pub(crate) fn restore_acp_agent(self: &Arc<Self>, agent_id: &AgentId) -> anyhow::Result<()> {
        self.acp_ensure(agent_id).map(|_| ())
    }

    pub(crate) fn acp_notice(&self, agent_id: &AgentId, text: String) {
        if let Some(live) = self.acp.get(agent_id) {
            self.acp_push(agent_id, &live, AcpItem::Notice { text });
            return;
        }
        let seq = self.store.last_acp_seq(agent_id).unwrap_or(0) + 1;
        self.acp_emit(
            agent_id,
            vec![AcpItemAt {
                seq,
                item: AcpItem::Notice { text },
            }],
        );
    }

    pub(crate) fn acp_note_checkpoint_restored(&self, agent_id: &AgentId, number: u32) {
        if !self.is_acp_agent(agent_id) {
            return;
        }
        self.acp_notice(
            agent_id,
            format!(
                "Files were restored to checkpoint {number}. The agent's conversation was not rewound, so it may still refer to the later changes."
            ),
        );
    }

    pub(crate) fn acp_note_trigger_failed(&self, agent_id: &AgentId, error: &anyhow::Error) {
        self.acp_notice(
            agent_id,
            format!("A prompt trigger fired but its prompt was not sent: {error:#}"),
        );
    }
}

fn needs_sign_in(error: &RpcError, caps: &Capabilities) -> bool {
    !caps.auth_methods.is_empty()
        && matches!(error, RpcError::Remote { code, message }
            if *code == AUTH_REQUIRED_CODE || message.to_ascii_lowercase().contains("auth"))
}
