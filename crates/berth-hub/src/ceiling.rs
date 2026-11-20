use crate::rights::{authorize, Permission, Refusal};
use crate::server::{Reply, Shared};
use crate::store::Store;
use anyhow::Result;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ceiling {
    Unlimited,
    Only(BTreeSet<String>),
}

impl Ceiling {
    pub fn allows(&self, member: &str) -> bool {
        match self {
            Ceiling::Unlimited => true,
            Ceiling::Only(ids) => ids.contains(member),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CeilingBody {
    pub(crate) members: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CeilingReply {
    pub(crate) allowed: BTreeSet<String>,
}

pub(crate) async fn allowed(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(b): Json<CeilingBody>,
) -> Reply {
    let store = s.store.lock().unwrap();
    let caller = authorize(&store, &headers, Permission::AgentsJoin)?.into_member()?;
    let allowed = store
        .instruct_ceiling(&caller.id, &b.members)
        .map_err(Refusal::Internal)?;
    Ok(Json(json!(CeilingReply { allowed })))
}

impl Store {
    fn instruct_ceiling(&self, caller: &str, ids: &[String]) -> Result<BTreeSet<String>> {
        let mut allowed = BTreeSet::new();
        for id in ids {
            let person = self.agent_owner(id)?.unwrap_or_else(|| id.clone());
            if self.may_instruct(&person)? && self.shares_channel(caller, &person)? {
                allowed.insert(id.clone());
            }
        }
        Ok(allowed)
    }

    fn may_instruct(&self, person: &str) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM members m JOIN role_permissions rp ON rp.role = m.role
              WHERE m.id = ?1 AND m.kind = 'person' AND m.status = 'approved' AND rp.permission = ?2)",
            params![person, Permission::AgentsInstruct.key()],
            |r| r.get(0),
        )?)
    }

    fn shares_channel(&self, a: &str, b: &str) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM channel_members x JOIN channel_members y ON y.channel = x.channel
              WHERE (x.member = ?1 OR x.member IN
                      (SELECT id FROM members WHERE kind = 'agent' AND status = 'agent' AND owner = ?1))
                AND (y.member = ?2 OR y.member IN
                      (SELECT id FROM members WHERE kind = 'agent' AND status = 'agent' AND owner = ?2)))",
            params![a, b],
            |r| r.get(0),
        )?)
    }
}
