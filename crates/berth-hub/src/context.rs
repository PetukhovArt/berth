use crate::server::{denied, fail, member, Reply, Shared};
use crate::store::KIND_NOTE;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

pub const NOTE_MAX_BYTES: usize = 16 * 1024;
const MODULES_MAX: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Touching {
    pub member: String,
    pub name: String,
    pub agent: String,
    pub modules: Vec<String>,
}

type ModulesByAgent = BTreeMap<String, Vec<String>>;

pub struct Touched {
    ttl: Duration,
    seen: HashMap<(String, String), (ModulesByAgent, Instant)>,
}

impl Touched {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            seen: HashMap::new(),
        }
    }

    pub fn report(&mut self, member: &str, channels: HashMap<String, ModulesByAgent>) {
        self.seen.retain(|(_, m), _| m != member);
        let now = Instant::now();
        for (channel, mut agents) in channels {
            agents.retain(|_, modules| !modules.is_empty());
            for modules in agents.values_mut() {
                modules.truncate(MODULES_MAX);
            }
            if !agents.is_empty() {
                self.seen
                    .insert((channel, member.to_string()), (agents, now));
            }
        }
    }

    pub fn others(&mut self, channel: &str, caller: &str) -> Vec<(String, String, Vec<String>)> {
        let ttl = self.ttl;
        self.seen.retain(|_, (_, at)| at.elapsed() < ttl);
        let mut found: Vec<_> = self
            .seen
            .iter()
            .filter(|((c, m), _)| c == channel && m != caller)
            .flat_map(|((_, m), (agents, _))| {
                agents
                    .iter()
                    .map(|(agent, modules)| (m.clone(), agent.clone(), modules.clone()))
            })
            .collect();
        found.sort();
        found
    }
}

#[derive(Deserialize)]
pub(crate) struct NoteBody {
    channel: String,
    content: String,
    #[serde(default)]
    as_agent: Option<String>,
}

pub(crate) async fn post_note(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<NoteBody>,
) -> Reply {
    let content = b.content.trim();
    if content.len() > NOTE_MAX_BYTES {
        return Err(fail(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("a note is at most {NOTE_MAX_BYTES} bytes"),
        ));
    }
    let store = s.store.lock().unwrap();
    let m = member(&store, &headers, "post")?;
    let author = store
        .acting_member(&m, b.as_agent.as_deref())
        .map_err(denied)?;
    let event = store
        .post(&b.channel, &author, KIND_NOTE, content, &[])
        .map_err(denied)?;
    Ok(Json(json!(event)))
}

pub(crate) async fn notes(State(s): State<Shared>, headers: HeaderMap) -> Reply {
    let store = s.store.lock().unwrap();
    let m = member(&store, &headers, "read")?;
    let rows = store
        .latest_notes(&m)
        .map_err(|e| fail(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
pub(crate) struct ReportBody {
    channels: HashMap<String, ModulesByAgent>,
}

pub(crate) async fn report_touched(
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
    s.touched.lock().unwrap().report(&id, known);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub(crate) struct TouchedQuery {
    channel: String,
}

pub(crate) async fn touched(
    State(s): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<TouchedQuery>,
) -> Reply {
    let store = s.store.lock().unwrap();
    let m = member(&store, &headers, "read")?;
    if !store.is_member(&q.channel, &m.id).map_err(denied)? {
        return Err(denied(anyhow::anyhow!("not a member of this channel")));
    }
    let rows: Vec<Touching> = s
        .touched
        .lock()
        .unwrap()
        .others(&q.channel, &m.id)
        .into_iter()
        .map(|(member, agent, modules)| Touching {
            name: store.member_name(&member).unwrap_or_default(),
            member,
            agent,
            modules,
        })
        .collect();
    Ok(Json(json!(rows)))
}
