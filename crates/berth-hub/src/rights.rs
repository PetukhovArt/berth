use crate::store::Store;
use anyhow::Result;
use axum::http::{HeaderMap, StatusCode};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use subtle::ConstantTimeEq;

const CATALOG: &str = include_str!("../permissions.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "&'static str", try_from = "String")]
pub enum Permission {
    ChannelsRead,
    ChannelsPost,
    ChannelsCreate,
    ChannelsInvite,
    AgentsJoin,
    AgentsInstruct,
    ContextWrite,
    AdminManage,
}

impl Permission {
    pub const ALL: [Permission; 8] = [
        Self::ChannelsRead,
        Self::ChannelsPost,
        Self::ChannelsCreate,
        Self::ChannelsInvite,
        Self::AgentsJoin,
        Self::AgentsInstruct,
        Self::ContextWrite,
        Self::AdminManage,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::ChannelsRead => "channels.read",
            Self::ChannelsPost => "channels.post",
            Self::ChannelsCreate => "channels.create",
            Self::ChannelsInvite => "channels.invite",
            Self::AgentsJoin => "agents.join",
            Self::AgentsInstruct => "agents.instruct",
            Self::ContextWrite => "context.write",
            Self::AdminManage => "admin.manage",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }
}

impl From<Permission> for &'static str {
    fn from(p: Permission) -> Self {
        p.key()
    }
}

impl TryFrom<String> for Permission {
    type Error = String;

    fn try_from(key: String) -> Result<Self, String> {
        Self::parse(&key).ok_or_else(|| format!("unknown permission `{key}`"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinRole {
    Admin,
    Member,
    Guest,
}

#[derive(Deserialize)]
struct CatalogEntry {
    builtin: Vec<String>,
}

impl BuiltinRole {
    pub(crate) const ALL: [BuiltinRole; 3] = [Self::Admin, Self::Member, Self::Guest];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
            Self::Guest => "guest",
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Admin => "Admin",
            Self::Member => "Member",
            Self::Guest => "Guest",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.id() == id)
    }

    pub(crate) fn permissions(self) -> Result<BTreeSet<Permission>> {
        if self == Self::Admin {
            return Ok(Permission::ALL.into_iter().collect());
        }
        let catalog: BTreeMap<String, CatalogEntry> = serde_json::from_str(CATALOG)?;
        Ok(catalog
            .into_iter()
            .filter(|(_, entry)| entry.builtin.iter().any(|r| r == self.id()))
            .filter_map(|(key, _)| Permission::parse(&key))
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub role: Option<RoleRef>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl Me {
    pub fn holds(&self, p: Permission) -> bool {
        self.permissions.iter().any(|k| k == p.key())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Member {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) role: RoleRef,
    permissions: BTreeSet<Permission>,
}

impl Member {
    pub(crate) fn holds(&self, p: Permission) -> bool {
        self.permissions.contains(&p)
    }

    pub(crate) fn acting_for(
        &self,
        store: &Store,
        as_agent: Option<&str>,
    ) -> Result<String, Refusal> {
        let Some(agent) = as_agent else {
            return Ok(self.id.clone());
        };
        if !self.holds(Permission::AgentsJoin) {
            return Err(Refusal::Missing {
                role: Some(self.role.name.clone()),
                permission: Permission::AgentsJoin,
            });
        }
        match store.agent_owner(agent)? {
            Some(owner) if owner == self.id => Ok(agent.to_string()),
            _ => Err(Refusal::Denied("not your Agent".into())),
        }
    }

    pub(crate) fn me(&self) -> Me {
        Me {
            id: self.id.clone(),
            name: self.name.clone(),
            role: Some(self.role.clone()),
            permissions: self
                .permissions
                .iter()
                .map(|p| p.key().to_string())
                .collect(),
        }
    }
}

pub(crate) enum Principal {
    Root,
    Member(Member),
}

const ROOT_ACTOR: (&str, &str) = ("operator", "admin token");

impl Principal {
    pub(crate) fn require(&self, p: Permission) -> Result<(), Refusal> {
        let (holds, role) = match self {
            Self::Root => (p == Permission::AdminManage, None),
            Self::Member(m) => (m.holds(p), Some(m.role.name.clone())),
        };
        if holds {
            Ok(())
        } else {
            Err(Refusal::Missing {
                role,
                permission: p,
            })
        }
    }

    pub(crate) fn into_member(self) -> Result<Member, Refusal> {
        match self {
            Self::Root => Err(Refusal::Denied("the admin token is not a member".into())),
            Self::Member(m) => Ok(m),
        }
    }

    pub(crate) fn actor(&self) -> (&str, &str) {
        match self {
            Self::Root => ROOT_ACTOR,
            Self::Member(m) => (&m.id, &m.name),
        }
    }
}

pub(crate) enum Need {
    Signed,
    Holds(Permission),
}

impl From<Permission> for Need {
    fn from(p: Permission) -> Self {
        Need::Holds(p)
    }
}

#[derive(Debug)]
pub(crate) enum Refusal {
    Unauthenticated,
    Missing {
        role: Option<String>,
        permission: Permission,
    },
    Denied(String),
    Invalid(String),
    NotFound(String),
    Conflict(String),
    Internal(anyhow::Error),
}

impl From<Refusal> for (StatusCode, String) {
    fn from(r: Refusal) -> Self {
        match r {
            Refusal::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unknown or unapproved token".into(),
            ),
            Refusal::Missing {
                role: Some(role),
                permission,
            } => (
                StatusCode::FORBIDDEN,
                format!("your role `{role}` does not grant `{}`", permission.key()),
            ),
            Refusal::Missing {
                role: None,
                permission,
            } => (
                StatusCode::FORBIDDEN,
                format!("the admin token does not grant `{}`", permission.key()),
            ),
            Refusal::Denied(m) => (StatusCode::FORBIDDEN, m),
            Refusal::Invalid(m) => (StatusCode::BAD_REQUEST, m),
            Refusal::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Refusal::Conflict(m) => (StatusCode::CONFLICT, m),
            Refusal::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    }
}

impl From<anyhow::Error> for Refusal {
    fn from(e: anyhow::Error) -> Self {
        Refusal::Internal(e)
    }
}

impl From<rusqlite::Error> for Refusal {
    fn from(e: rusqlite::Error) -> Self {
        Refusal::Internal(e.into())
    }
}

fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
}

pub(crate) fn authorize(
    store: &Store,
    headers: &HeaderMap,
    need: impl Into<Need>,
) -> Result<Principal, Refusal> {
    let presented = bearer(headers);
    let who = if store.is_admin_token(presented) {
        Principal::Root
    } else {
        Principal::Member(
            store
                .member_by_token(presented)?
                .ok_or(Refusal::Unauthenticated)?,
        )
    };
    if let Need::Holds(p) = need.into() {
        who.require(p)?;
    }
    Ok(who)
}

impl Store {
    fn member_by_token(&self, presented: &str) -> Result<Option<Member>> {
        let mut st = self.db.prepare(
            "SELECT m.id, m.name, m.token, r.id, r.name FROM members m JOIN roles r ON r.id = m.role
             WHERE m.kind = 'person' AND m.status = 'approved'",
        )?;
        let mut rows = st.query([])?;
        let mut found = None;
        while let Some(r) = rows.next()? {
            let token: String = r.get(2)?;
            if bool::from(token.as_bytes().ct_eq(presented.as_bytes())) {
                found = Some((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    RoleRef {
                        id: r.get(3)?,
                        name: r.get(4)?,
                    },
                ));
            }
        }
        let Some((id, name, role)) = found else {
            return Ok(None);
        };
        let permissions = role_permissions(&self.db, &role.id)?;
        Ok(Some(Member {
            id,
            name,
            role,
            permissions,
        }))
    }
}

pub(crate) fn role_permissions(db: &Connection, role: &str) -> Result<BTreeSet<Permission>> {
    let mut st = db.prepare("SELECT permission FROM role_permissions WHERE role = ?1")?;
    let keys = st.query_map(params![role], |r| r.get::<_, String>(0))?;
    let mut held = BTreeSet::new();
    for key in keys {
        if let Some(p) = Permission::parse(&key?) {
            held.insert(p);
        }
    }
    Ok(held)
}
