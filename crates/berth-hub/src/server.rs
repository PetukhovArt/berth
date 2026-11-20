use crate::context::{self, Touched};
use crate::presence::{Presence, Present, PRESENCE_TTL};
use crate::rights::{authorize, Need, Permission};
use crate::store::{Event, Store, KIND_ANSWER, KIND_MESSAGE, KIND_REQUEST, KIND_SETTLED};
use crate::stream::{Ended, EndedWhy};
use crate::tls::{acceptor, TlsFiles, TlsListener};
use anyhow::Result;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const APPENDED_BACKLOG: usize = 256;
const ENDED_BACKLOG: usize = 16;

#[derive(Clone)]
pub(crate) struct Shared {
    pub(crate) store: Arc<Mutex<Store>>,
    presence: Arc<Mutex<Presence>>,
    pub(crate) touched: Arc<Mutex<Touched>>,
    pub(crate) appended: tokio::sync::broadcast::Sender<Event>,
    pub(crate) ended: tokio::sync::broadcast::Sender<Ended>,
}

impl Shared {
    pub(crate) fn end(&self, ended: Ended) {
        if ended.why == EndedWhy::Revoked {
            self.presence
                .lock()
                .unwrap()
                .report(&ended.member, HashMap::new());
        }
        let _ = self.ended.send(ended);
    }
}

pub(crate) type Reply = Result<Json<Value>, (StatusCode, String)>;

pub struct Hub {
    pub addr: SocketAddr,
    pub admin_token: String,
    pub task: tokio::task::JoinHandle<()>,
}

pub async fn serve(
    db: &Path,
    bind: SocketAddr,
    admin_token: Option<String>,
    tls: Option<TlsFiles>,
) -> Result<Hub> {
    start(db, bind, admin_token, PRESENCE_TTL, tls).await
}

pub async fn serve_with_ttl(
    db: &Path,
    bind: SocketAddr,
    admin_token: Option<String>,
    presence_ttl: Duration,
) -> Result<Hub> {
    start(db, bind, admin_token, presence_ttl, None).await
}

async fn start(
    db: &Path,
    bind: SocketAddr,
    admin_token: Option<String>,
    presence_ttl: Duration,
    tls: Option<TlsFiles>,
) -> Result<Hub> {
    let acceptor = tls.as_ref().map(acceptor).transpose()?;
    let store = Store::open(db)?;
    let admin_token = store.admin_token(admin_token)?;
    let state = Shared {
        store: Arc::new(Mutex::new(store)),
        presence: Arc::new(Mutex::new(Presence::new(presence_ttl))),
        touched: Arc::new(Mutex::new(Touched::new(presence_ttl))),
        appended: tokio::sync::broadcast::channel(APPENDED_BACKLOG).0,
        ended: tokio::sync::broadcast::channel(ENDED_BACKLOG).0,
    };
    let app = Router::new()
        .route("/pair", post(pair))
        .route("/admin", get(crate::admin::view))
        .route("/admin/change", post(crate::admin::change))
        .route("/instruct/ceiling", post(crate::ceiling::allowed))
        .route("/agents", post(agent))
        .route("/me", get(me))
        .route("/channels", get(channels).post(create_channel))
        .route("/channels/members", post(add_member))
        .route("/events", get(read).post(post_event))
        .route("/mentions", get(mentions))
        .route("/presence", get(presence).post(report_presence))
        .route("/notes", get(context::notes).post(context::post_note))
        .route(
            "/touched",
            get(context::touched).post(context::report_touched),
        )
        .route("/stream", get(crate::stream::stream))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let addr = listener.local_addr()?;
    let task = match acceptor {
        Some(acceptor) => {
            let listener = TlsListener::new(listener, acceptor)?;
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("hub stopped: {e}");
                }
            })
        }
        None => tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("hub stopped: {e}");
            }
        }),
    };
    Ok(Hub {
        addr,
        admin_token,
        task,
    })
}

pub(crate) fn fail(status: StatusCode, e: impl std::fmt::Display) -> (StatusCode, String) {
    (status, e.to_string())
}

pub(crate) fn denied(e: anyhow::Error) -> (StatusCode, String) {
    fail(StatusCode::FORBIDDEN, e)
}

#[derive(Deserialize)]
struct PairBody {
    name: String,
}

async fn pair(State(s): State<Shared>, Json(b): Json<PairBody>) -> Reply {
    let (id, token, admins) = {
        let store = s.store.lock().unwrap();
        let (id, token) = store
            .admit_device(&b.name)
            .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        let admins = store
            .admins()
            .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        (id, token, admins)
    };
    for member in admins {
        s.end(Ended {
            member,
            why: EndedWhy::RightsChanged,
        });
    }
    Ok(Json(json!({ "id": id, "token": token })))
}

#[derive(Deserialize)]
struct AgentBody {
    name: String,
    #[serde(default)]
    display: Option<String>,
}

async fn agent(State(s): State<Shared>, headers: HeaderMap, Json(b): Json<AgentBody>) -> Reply {
    let store = s.store.lock().unwrap();
    let owner = authorize(&store, &headers, Permission::AgentsJoin)?.into_member()?;
    let id = store
        .ensure_agent(&owner.id, &b.name, b.display.as_deref())
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "id": id })))
}

async fn channels(State(s): State<Shared>, headers: HeaderMap) -> Reply {
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Permission::ChannelsRead)?.into_member()?;
    let rows = store
        .channels(&m.id)
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
struct ChannelBody {
    key: String,
}

async fn create_channel(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<ChannelBody>,
) -> Reply {
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Permission::ChannelsCreate)?.into_member()?;
    store.create_channel(&b.key, &m.id).map_err(denied)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AddMemberBody {
    key: String,
    member: String,
}

async fn add_member(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<AddMemberBody>,
) -> Reply {
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Permission::ChannelsInvite)?.into_member()?;
    store.add_member(&b.key, &m.id, &b.member).map_err(denied)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PostBody {
    channel: String,
    content: String,
    #[serde(default = "message_kind")]
    kind: u32,
    #[serde(default)]
    tags: Vec<Vec<String>>,
    #[serde(default)]
    as_agent: Option<String>,
}

fn message_kind() -> u32 {
    KIND_MESSAGE
}

fn kind_allowed(kind: u32, as_agent: bool) -> bool {
    match kind {
        KIND_MESSAGE => true,
        KIND_REQUEST | KIND_SETTLED => as_agent,
        KIND_ANSWER => !as_agent,
        _ => false,
    }
}

async fn post_event(State(s): State<Shared>, headers: HeaderMap, Json(b): Json<PostBody>) -> Reply {
    if !kind_allowed(b.kind, b.as_agent.is_some()) {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            format!("event kind {} is not allowed here", b.kind),
        ));
    }
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Permission::ChannelsPost)?.into_member()?;
    let author = m.acting_for(&store, b.as_agent.as_deref())?;
    let mut tags = b.tags.clone();
    if b.kind == KIND_MESSAGE {
        for tag in store
            .mention_tags(&b.channel, &author, &b.content)
            .map_err(denied)?
        {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    let event = store
        .post(&b.channel, &author, b.kind, &b.content, &tags)
        .map_err(denied)?;
    let _ = s.appended.send(event.clone());
    Ok(Json(json!(event)))
}

async fn me(State(s): State<Shared>, headers: HeaderMap) -> Reply {
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Need::Signed)?.into_member()?;
    Ok(Json(json!(m.me())))
}

#[derive(Deserialize)]
struct MentionsQuery {
    #[serde(default)]
    after: i64,
}

async fn mentions(
    State(s): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<MentionsQuery>,
) -> Reply {
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Permission::ChannelsRead)?.into_member()?;
    if !m.holds(Permission::AgentsJoin) {
        return Ok(Json(json!([])));
    }
    let rows = store
        .mentions(&m.id, q.after)
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
struct ReadQuery {
    channel: String,
    #[serde(default)]
    after: i64,
    #[serde(default)]
    as_agent: Option<String>,
}

async fn read(State(s): State<Shared>, headers: HeaderMap, Query(q): Query<ReadQuery>) -> Reply {
    let store = s.store.lock().unwrap();
    let m = authorize(&store, &headers, Permission::ChannelsRead)?.into_member()?;
    let reader = m.acting_for(&store, q.as_agent.as_deref())?;
    let events = store
        .read_after(&q.channel, &reader, q.after)
        .map_err(denied)?;
    Ok(Json(json!(events)))
}

#[derive(Deserialize)]
struct ReportBody {
    channels: HashMap<String, Vec<String>>,
}

async fn report_presence(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<ReportBody>,
) -> Reply {
    let (id, known) = {
        let store = s.store.lock().unwrap();
        let m = authorize(&store, &headers, Permission::ChannelsRead)?.into_member()?;
        let mut known = HashMap::new();
        for (key, agents) in b.channels {
            if store.is_member(&key, &m.id).map_err(denied)? {
                known.insert(key, agents);
            }
        }
        (m.id, known)
    };
    s.presence.lock().unwrap().report(&id, known);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PresenceQuery {
    channel: String,
}

async fn presence(
    State(s): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<PresenceQuery>,
) -> Reply {
    let here: Vec<Present> = {
        let store = s.store.lock().unwrap();
        let m = authorize(&store, &headers, Permission::ChannelsRead)?.into_member()?;
        if !store.is_member(&q.channel, &m.id).map_err(denied)? {
            return Err(denied(anyhow::anyhow!("not a member of this channel")));
        }
        let ids = s.presence.lock().unwrap().on(&q.channel);
        ids.into_iter()
            .map(|(member, agents)| Present {
                name: store.member_name(&member).unwrap_or_default(),
                member,
                agents,
            })
            .collect()
    };
    Ok(Json(json!(here)))
}
