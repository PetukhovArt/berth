use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};

use axum::extract::{ConnectInfo, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use berth_core::{Agent, AgentKind, Entity, EntityId, Worktree};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    client_ip, cookie_token, denied, guard, harden, plain, qr_svg, Auth, Ctx, Remote, Shared,
};
use crate::agent_sessions::{self, Harness};
use crate::berth::{self, Berth, WatchEvent};
use crate::connectors::types::DiffFile;
use crate::remote_devices::{self, Pairing, Scope};
use crate::sink::Sink;

const TAIL: u32 = 200;

#[derive(Default)]
struct Tree {
    loaded: bool,
    agents: HashMap<String, Agent>,
    worktrees: HashMap<String, Worktree>,
}

#[derive(Clone, Default)]
struct Cache(Arc<Mutex<Tree>>);

impl Sink<WatchEvent> for Cache {
    fn send(&self, event: WatchEvent) -> bool {
        let mut tree = guard(&self.0);
        match event {
            WatchEvent::Snapshot {
                agents, worktrees, ..
            } => {
                tree.loaded = true;
                tree.agents = agents.into_iter().map(|a| (a.id.0.clone(), a)).collect();
                tree.worktrees = worktrees.into_iter().map(|w| (w.id.0.clone(), w)).collect();
            }
            WatchEvent::Upserted {
                entity: Entity::Agent(agent),
            } => {
                tree.agents.insert(agent.id.0.clone(), *agent);
            }
            WatchEvent::Upserted {
                entity: Entity::Worktree(worktree),
            } => {
                tree.worktrees.insert(worktree.id.0.clone(), worktree);
            }
            WatchEvent::Removed {
                id: EntityId::Agent(id),
            } => {
                tree.agents.remove(id.as_str());
            }
            WatchEvent::Removed {
                id: EntityId::Worktree(id),
            } => {
                tree.worktrees.remove(id.as_str());
            }
            WatchEvent::Status { agent, status, .. } => {
                if let Some(agent) = tree.agents.get_mut(agent.as_str()) {
                    agent.status = status;
                }
            }
            _ => {}
        }
        true
    }
}

struct Link {
    berth: Berth,
    cache: Cache,
}

#[derive(Default)]
pub(super) struct Feed(OnceLock<Link>);

enum Read {
    Connecting,
    Gone,
    Found(Box<Agent>, Option<Worktree>),
}

impl Feed {
    fn link(&self) -> &Link {
        self.0.get_or_init(|| {
            let cache = Cache::default();
            let berth = Berth::spawn();
            berth::berth_watch(&berth, Box::new(cache.clone()));
            Link { berth, cache }
        })
    }

    fn lookup(&self, agent: &str) -> Read {
        let tree = guard(&self.link().cache.0);
        if !tree.loaded {
            return Read::Connecting;
        }
        match tree.agents.get(agent) {
            Some(found) => Read::Found(
                Box::new(found.clone()),
                tree.worktrees.get(found.worktree_id.as_str()).cloned(),
            ),
            None => Read::Gone,
        }
    }

    async fn messages(&self, agent: &Agent) -> Value {
        let harness = match agent.kind {
            AgentKind::Claude => Harness::Claude,
            AgentKind::Codex => Harness::Codex,
            AgentKind::Acp => return self.acp_items(agent).await,
            _ => return json!({ "transcript": [] }),
        };
        let Some(id) = agent.session_id.clone() else {
            return json!({ "transcript": [] });
        };
        let transcript = agent_sessions::agent_session_transcript(harness, id, Some(TAIL))
            .await
            .unwrap_or_default();
        json!({ "transcript": transcript })
    }

    async fn acp_items(&self, agent: &Agent) -> Value {
        let request = json!({ "AcpTranscript": { "agent": agent.id, "after": 0 } });
        let items = berth::berth_call(&self.link().berth, request)
            .await
            .ok()
            .and_then(|reply| reply.pointer("/AcpTranscript/items").cloned())
            .and_then(|items| match items {
                Value::Array(mut items) => {
                    let skip = items.len().saturating_sub(TAIL as usize);
                    Some(Value::Array(items.drain(skip..).collect()))
                }
                _ => None,
            })
            .unwrap_or_else(|| json!([]));
        json!({ "acp": items })
    }
}

async fn changed_files(worktree: Option<&Worktree>) -> Vec<DiffFile> {
    let Some(worktree) = worktree else {
        return Vec::new();
    };
    crate::changes::worktree_changes(worktree.path.to_string_lossy().into_owned(), None)
        .await
        .unwrap_or_default()
}

async fn changes(worktree: Option<&Worktree>) -> Value {
    changed_files(worktree)
        .await
        .into_iter()
        .map(|file| {
            json!({
                "path": file.new_path,
                "status": file.status,
                "additions": file.additions,
                "deletions": file.deletions,
            })
        })
        .collect()
}

struct Guest {
    agent: String,
    diffs: bool,
}

impl Shared {
    fn guest(&self, presented: &str) -> Option<Guest> {
        let device = self.identify(presented)?;
        match device.scope {
            Scope::Session(agent) => Some(Guest {
                agent,
                diffs: device.diffs,
            }),
            Scope::Full | Scope::ReadOnly => None,
        }
    }

    pub(super) fn guest_of(&self, presented: &str) -> Option<String> {
        self.guest(presented).map(|guest| guest.agent)
    }
}

impl Remote {
    pub fn share_session(
        &self,
        agent: String,
        name: String,
        diffs: bool,
    ) -> Result<Pairing, String> {
        let scope = Scope::Session(agent);
        let existing = self.0.with_devices(|devices| {
            devices
                .iter()
                .find(|device| device.scope == scope)
                .map(|device| device.token.clone())
        })?;
        let pairing = match existing {
            Some(token) => {
                let url = format!("{}?token={token}", self.base_url()?);
                Pairing {
                    qr_svg: qr_svg(&url),
                    url,
                }
            }
            None => self.add_device(name, scope.clone())?,
        };
        self.0.with_devices(|devices| {
            match devices.iter_mut().find(|device| device.scope == scope) {
                Some(device) if device.diffs != diffs => {
                    device.diffs = diffs;
                    remote_devices::save(devices)
                }
                _ => Ok(()),
            }
        })??;
        Ok(pairing)
    }

    pub fn unshare_session(&self, agent: String) -> Result<(), String> {
        let scope = Scope::Session(agent);
        self.0.with_devices(|devices| {
            devices.retain(|device| device.scope != scope);
            remote_devices::save(devices)
        })??;
        self.0.revoked.send_modify(|count| *count += 1);
        Ok(())
    }
}

pub(super) fn first_page(mut response: Response, cookie: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

fn admit(ctx: &Ctx, peer: SocketAddr, headers: &HeaderMap) -> Result<Guest, Box<Response>> {
    if !ctx.current() {
        return Err(Box::new(plain(StatusCode::SERVICE_UNAVAILABLE)));
    }
    let ip = client_ip(ctx, peer, headers);
    let Some(presented) = cookie_token(headers) else {
        return Err(Box::new(harden(denied())));
    };
    match ctx.shared.check(ip, &presented) {
        Auth::Granted => {}
        Auth::Denied => return Err(Box::new(harden(denied()))),
        Auth::Blocked => return Err(Box::new(plain(StatusCode::TOO_MANY_REQUESTS))),
    }
    ctx.shared
        .guest(&presented)
        .ok_or_else(|| Box::new(harden(plain(StatusCode::FORBIDDEN))))
}

pub(super) async fn feed_api(
    State(ctx): State<Ctx>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let guest = match admit(&ctx, peer, &headers) {
        Ok(guest) => guest,
        Err(response) => return *response,
    };
    let feed = &ctx.shared.feed;
    harden(match feed.lookup(&guest.agent) {
        Read::Connecting => plain(StatusCode::SERVICE_UNAVAILABLE),
        Read::Gone => plain(StatusCode::GONE),
        Read::Found(agent, worktree) => {
            let (messages, changes) =
                tokio::join!(feed.messages(&agent), changes(worktree.as_ref()));
            axum::Json(json!({
                "name": agent.name,
                "kind": agent.kind,
                "status": agent.status,
                "branch": worktree.as_ref().map(|worktree| worktree.branch.clone()),
                "messages": messages,
                "changes": changes,
                "diffs": guest.diffs,
            }))
            .into_response()
        }
    })
}

#[derive(Deserialize)]
pub(super) struct DiffQuery {
    path: String,
}

pub(super) async fn diff_api(
    State(ctx): State<Ctx>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<DiffQuery>,
) -> Response {
    let guest = match admit(&ctx, peer, &headers) {
        Ok(guest) => guest,
        Err(response) => return *response,
    };
    if !guest.diffs {
        return harden(plain(StatusCode::FORBIDDEN));
    }
    harden(match ctx.shared.feed.lookup(&guest.agent) {
        Read::Connecting => plain(StatusCode::SERVICE_UNAVAILABLE),
        Read::Gone => plain(StatusCode::GONE),
        Read::Found(_, worktree) => {
            match changed_files(worktree.as_ref())
                .await
                .into_iter()
                .find(|file| file.new_path == query.path)
            {
                Some(file) => axum::Json(file).into_response(),
                None => plain(StatusCode::NOT_FOUND),
            }
        }
    })
}
