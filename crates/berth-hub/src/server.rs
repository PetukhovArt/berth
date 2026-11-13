use crate::presence::{Presence, Present, PRESENCE_TTL};
use crate::store::{Member, Store, KIND_MESSAGE};
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

#[derive(Clone)]
struct Shared {
    store: Arc<Mutex<Store>>,
    presence: Arc<Mutex<Presence>>,
}

type Reply = Result<Json<Value>, (StatusCode, String)>;

pub struct Hub {
    pub addr: SocketAddr,
    pub admin_token: String,
    pub task: tokio::task::JoinHandle<()>,
}

pub async fn serve(db: &Path, bind: SocketAddr, admin_token: Option<String>) -> Result<Hub> {
    serve_with_ttl(db, bind, admin_token, PRESENCE_TTL).await
}

pub async fn serve_with_ttl(
    db: &Path,
    bind: SocketAddr,
    admin_token: Option<String>,
    presence_ttl: Duration,
) -> Result<Hub> {
    let store = Store::open(db)?;
    let admin_token = store.admin_token(admin_token)?;
    let state = Shared {
        store: Arc::new(Mutex::new(store)),
        presence: Arc::new(Mutex::new(Presence::new(presence_ttl))),
    };
    let app = Router::new()
        .route("/pair", post(pair))
        .route("/admin/pending", get(pending))
        .route("/admin/approve", post(approve))
        .route("/agents", post(agent))
        .route("/channels", get(channels).post(create_channel))
        .route("/channels/members", post(add_member))
        .route("/events", get(read).post(post_event))
        .route("/mentions", get(mentions))
        .route("/presence", get(presence).post(report_presence))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let addr = listener.local_addr()?;
    let task = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("hub stopped: {e}");
        }
    });
    Ok(Hub {
        addr,
        admin_token,
        task,
    })
}

fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
}

fn fail(status: StatusCode, e: impl std::fmt::Display) -> (StatusCode, String) {
    (status, e.to_string())
}

fn member(store: &Store, headers: &HeaderMap, scope: &str) -> Result<Member, (StatusCode, String)> {
    let m = store
        .member_by_token(bearer(headers))
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| fail(StatusCode::UNAUTHORIZED, "unknown or unapproved token"))?;
    if !m.can(scope) {
        return Err(fail(StatusCode::FORBIDDEN, format!("this device may not `{scope}`")));
    }
    Ok(m)
}

fn admin(store: &Store, headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    if store.is_admin(bearer(headers)) {
        Ok(())
    } else {
        Err(fail(StatusCode::UNAUTHORIZED, "admin token required"))
    }
}

fn denied(e: anyhow::Error) -> (StatusCode, String) {
    fail(StatusCode::FORBIDDEN, e)
}

#[derive(Deserialize)]
struct PairBody {
    name: String,
}

async fn pair(State(s): State<Shared>, Json(b): Json<PairBody>) -> Reply {
    let store = s.store.lock().unwrap();
    let (id, token) = store
        .admit_device(&b.name)
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "id": id, "token": token })))
}

async fn pending(State(s): State<Shared>, headers: HeaderMap) -> Reply {
    let store = s.store.lock().unwrap();
    admin(&store, &headers)?;
    let rows = store
        .pending()
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, name)| json!({ "id": id, "name": name }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct ApproveBody {
    id: String,
    scopes: Vec<String>,
}

async fn approve(State(s): State<Shared>, headers: HeaderMap, Json(b): Json<ApproveBody>) -> Reply {
    let store = s.store.lock().unwrap();
    admin(&store, &headers)?;
    store
        .approve_scopes(&b.id, &b.scopes)
        .map_err(|e| fail(StatusCode::NOT_FOUND, e))?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AgentBody {
    name: String,
}

async fn agent(State(s): State<Shared>, headers: HeaderMap, Json(b): Json<AgentBody>) -> Reply {
    let store = s.store.lock().unwrap();
    let owner = member(&store, &headers, "post")?;
    let id = store
        .ensure_agent(&owner, &b.name)
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "id": id })))
}

async fn channels(State(s): State<Shared>, headers: HeaderMap) -> Reply {
    let store = s.store.lock().unwrap();
    let m = member(&store, &headers, "read")?;
    let rows = store
        .channels(&m)
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
    let m = member(&store, &headers, "create")?;
    store.create_channel(&b.key, &m).map_err(denied)?;
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
    let m = member(&store, &headers, "create")?;
    store.add_member(&b.key, &m, &b.member).map_err(denied)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PostBody {
    channel: String,
    content: String,
    #[serde(default)]
    tags: Vec<Vec<String>>,
    #[serde(default)]
    as_agent: Option<String>,
}

async fn post_event(State(s): State<Shared>, headers: HeaderMap, Json(b): Json<PostBody>) -> Reply {
    let store = s.store.lock().unwrap();
    let m = member(&store, &headers, "post")?;
    let author = store.acting_member(&m, b.as_agent.as_deref()).map_err(denied)?;
    let mut tags = b.tags.clone();
    for tag in store.mention_tags(&b.channel, &b.content).map_err(denied)? {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    let event = store
        .post(&b.channel, &author, KIND_MESSAGE, &b.content, &tags)
        .map_err(denied)?;
    Ok(Json(json!(event)))
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
    let m = member(&store, &headers, "read")?;
    let rows = store
        .mentions(&m, q.after)
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
    let m = member(&store, &headers, "read")?;
    let reader = store.acting_member(&m, q.as_agent.as_deref()).map_err(denied)?;
    let as_reader = Member {
        id: reader,
        scopes: m.scopes.clone(),
    };
    let events = store.read_after(&q.channel, &as_reader, q.after).map_err(denied)?;
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
        let m = member(&store, &headers, "post")?;
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
        let m = member(&store, &headers, "read")?;
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
