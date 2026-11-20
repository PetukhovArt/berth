use crate::rights::{authorize, role_permissions, BuiltinRole, Permission, Principal, Refusal};
use crate::server::{Reply, Shared};
use crate::store::{has_member_column, now_ms, random_token, Store};
use crate::stream::{Ended, EndedWhy};
use anyhow::{anyhow, Result};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const AUDIT_PAGE: i64 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminChange {
    SaveRole {
        id: Option<String>,
        name: String,
        permissions: Vec<String>,
    },
    DeleteRole {
        id: String,
    },
    SetRole {
        member: String,
        role: String,
    },
    Revoke {
        member: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminView {
    pub catalog: Vec<String>,
    pub roles: Vec<RoleView>,
    pub members: Vec<MemberView>,
    pub audit: Vec<AuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleView {
    pub id: String,
    pub name: String,
    pub builtin: bool,
    pub permissions: Vec<String>,
    pub members: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberStatus {
    Pending,
    Approved,
    Revoked,
}

impl MemberStatus {
    fn parse(column: &str) -> Option<Self> {
        match column {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberView {
    pub id: String,
    pub name: String,
    pub status: MemberStatus,
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: i64,
    pub at: i64,
    pub actor: String,
    pub actor_name: String,
    pub action: AuditAction,
    pub target: String,
    pub detail: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    #[serde(rename = "role.saved")]
    RoleSaved,
    #[serde(rename = "role.deleted")]
    RoleDeleted,
    #[serde(rename = "member.role")]
    MemberRole,
    #[serde(rename = "member.revoked")]
    MemberRevoked,
    #[serde(rename = "hub.migrated")]
    HubMigrated,
}

impl AuditAction {
    const ALL: [AuditAction; 5] = [
        Self::RoleSaved,
        Self::RoleDeleted,
        Self::MemberRole,
        Self::MemberRevoked,
        Self::HubMigrated,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::RoleSaved => "role.saved",
            Self::RoleDeleted => "role.deleted",
            Self::MemberRole => "member.role",
            Self::MemberRevoked => "member.revoked",
            Self::HubMigrated => "hub.migrated",
        }
    }

    fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == key)
    }
}

const ROLE_TABLES: &str = "
    CREATE TABLE IF NOT EXISTS roles (
      id TEXT PRIMARY KEY, name TEXT NOT NULL, builtin INTEGER NOT NULL DEFAULT 0,
      created_at INTEGER NOT NULL);
    CREATE UNIQUE INDEX IF NOT EXISTS roles_name ON roles (lower(name));
    CREATE TABLE IF NOT EXISTS role_permissions (
      role TEXT NOT NULL, permission TEXT NOT NULL, PRIMARY KEY (role, permission));
    CREATE TABLE IF NOT EXISTS audit (
      seq INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL, actor TEXT NOT NULL,
      actor_name TEXT NOT NULL, action TEXT NOT NULL, target TEXT NOT NULL, detail TEXT NOT NULL);";

pub(crate) fn prepare_roles(db: &mut Connection, path: &Path) -> Result<()> {
    db.execute_batch(ROLE_TABLES)?;
    let from_scopes = has_member_column(db, "scopes")?;
    if from_scopes {
        back_up(db, path)?;
    }
    let tx = db.transaction()?;
    seed_builtin_roles(&tx)?;
    if from_scopes {
        drop_scopes(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

fn back_up(db: &Connection, path: &Path) -> Result<()> {
    let mut name = path.as_os_str().to_owned();
    name.push(".v1.bak");
    let backup = PathBuf::from(name);
    match std::fs::remove_file(&backup) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    let target = backup
        .to_str()
        .ok_or_else(|| anyhow!("the hub database path is not UTF-8"))?;
    db.execute("VACUUM INTO ?1", [target])?;
    Ok(())
}

fn seed_builtin_roles(db: &Connection) -> Result<()> {
    for role in BuiltinRole::ALL {
        db.execute(
            "INSERT INTO roles (id, name, builtin, created_at) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, builtin = 1",
            params![role.id(), role.name(), now_ms()],
        )?;
        write_permissions(db, role.id(), &role.permissions()?)?;
    }
    Ok(())
}

fn drop_scopes(db: &Connection) -> Result<()> {
    if !has_member_column(db, "role")? {
        db.execute_batch("ALTER TABLE members ADD COLUMN role TEXT;")?;
    }
    let promoted = db.execute(
        "UPDATE members SET role = ?1 WHERE kind = 'person' AND status = 'approved'",
        [BuiltinRole::Member.id()],
    )?;
    db.execute_batch("ALTER TABLE members DROP COLUMN scopes;")?;
    audit(
        db,
        ("hub", "hub"),
        &Applied {
            action: AuditAction::HubMigrated,
            target: "members".into(),
            detail: json!({ "before": "scopes", "after": BuiltinRole::Member.id(), "members": promoted }),
            ended: Vec::new(),
        },
    )
}

fn write_permissions(
    db: &Connection,
    role: &str,
    permissions: &BTreeSet<Permission>,
) -> Result<()> {
    db.execute("DELETE FROM role_permissions WHERE role = ?1", [role])?;
    for p in permissions {
        db.execute(
            "INSERT INTO role_permissions (role, permission) VALUES (?1, ?2)",
            params![role, p.key()],
        )?;
    }
    Ok(())
}

struct Applied {
    action: AuditAction,
    target: String,
    detail: Value,
    ended: Vec<Ended>,
}

fn audit(db: &Connection, (actor, actor_name): (&str, &str), applied: &Applied) -> Result<()> {
    db.execute(
        "INSERT INTO audit (at, actor, actor_name, action, target, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            now_ms(),
            actor,
            actor_name,
            applied.action.key(),
            applied.target,
            applied.detail.to_string()
        ],
    )?;
    Ok(())
}

#[derive(Serialize, PartialEq)]
struct RoleState {
    name: String,
    permissions: BTreeSet<Permission>,
}

fn role_state(db: &Connection, id: &str) -> Result<Option<RoleState>> {
    let Some(name) = db
        .query_row("SELECT name FROM roles WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
    else {
        return Ok(None);
    };
    Ok(Some(RoleState {
        name,
        permissions: role_permissions(db, id)?,
    }))
}

fn holders(db: &Connection, role: &str) -> Result<Vec<String>> {
    let mut st = db.prepare(
        "SELECT id FROM members WHERE kind = 'person' AND status = 'approved' AND role = ?1",
    )?;
    let ids = st.query_map([role], |r| r.get(0))?;
    Ok(ids.collect::<rusqlite::Result<_>>()?)
}

fn device(db: &Connection, id: &str) -> Result<Option<(String, Option<String>)>> {
    Ok(db
        .query_row(
            "SELECT status, role FROM members WHERE id = ?1 AND kind = 'person'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn builtin_refusal(role: BuiltinRole) -> Refusal {
    Refusal::Conflict(format!(
        "the built-in role `{}` cannot be changed",
        role.name()
    ))
}

fn save_role(
    db: &Connection,
    id: Option<String>,
    name: &str,
    keys: &[String],
) -> Result<Option<Applied>, Refusal> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Refusal::Invalid("a role needs a name".into()));
    }
    let permissions = keys
        .iter()
        .map(|k| {
            Permission::parse(k)
                .ok_or_else(|| Refusal::Invalid(format!("unknown permission `{k}`")))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if let Some(builtin) = id.as_deref().and_then(BuiltinRole::from_id) {
        return Err(builtin_refusal(builtin));
    }
    let before = match &id {
        Some(id) => {
            Some(role_state(db, id)?.ok_or_else(|| Refusal::NotFound(format!("no role `{id}`")))?)
        }
        None => None,
    };
    let id = id.unwrap_or_else(|| format!("r-{}", &random_token()[..12]));
    let taken: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE lower(name) = lower(?1) AND id <> ?2)",
        params![name, id],
        |r| r.get(0),
    )?;
    if taken {
        return Err(Refusal::Conflict(format!("a role named `{name}` exists")));
    }
    let after = RoleState {
        name: name.to_string(),
        permissions,
    };
    if before.as_ref() == Some(&after) {
        return Ok(None);
    }
    db.execute(
        "INSERT INTO roles (id, name, builtin, created_at) VALUES (?1, ?2, 0, ?3)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name",
        params![id, after.name, now_ms()],
    )?;
    write_permissions(db, &id, &after.permissions)?;
    let ended = holders(db, &id)?
        .into_iter()
        .map(|member| Ended {
            member,
            why: EndedWhy::RightsChanged,
        })
        .collect();
    Ok(Some(Applied {
        action: AuditAction::RoleSaved,
        detail: json!({ "before": before, "after": after }),
        target: id,
        ended,
    }))
}

fn delete_role(db: &Connection, id: &str) -> Result<Option<Applied>, Refusal> {
    if let Some(builtin) = BuiltinRole::from_id(id) {
        return Err(builtin_refusal(builtin));
    }
    let before = role_state(db, id)?.ok_or_else(|| Refusal::NotFound(format!("no role `{id}`")))?;
    let held = holders(db, id)?.len();
    if held > 0 {
        return Err(Refusal::Conflict(format!(
            "the role `{}` is still held by {held} members: move them first",
            before.name
        )));
    }
    db.execute("UPDATE members SET role = NULL WHERE role = ?1", [id])?;
    db.execute("DELETE FROM role_permissions WHERE role = ?1", [id])?;
    db.execute("DELETE FROM roles WHERE id = ?1", [id])?;
    Ok(Some(Applied {
        action: AuditAction::RoleDeleted,
        target: id.to_string(),
        detail: json!({ "before": before, "after": null }),
        ended: Vec::new(),
    }))
}

fn set_role(db: &Connection, member: &str, role: &str) -> Result<Option<Applied>, Refusal> {
    let role_exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE id = ?1)",
        [role],
        |r| r.get(0),
    )?;
    if !role_exists {
        return Err(Refusal::NotFound(format!("no role `{role}`")));
    }
    let (status, held) =
        device(db, member)?.ok_or_else(|| Refusal::NotFound("no such device".into()))?;
    if status == "revoked" {
        return Err(Refusal::Conflict(
            "this device was revoked: pair again".into(),
        ));
    }
    if status == "approved" && held.as_deref() == Some(role) {
        return Ok(None);
    }
    db.execute(
        "UPDATE members SET status = 'approved', role = ?2 WHERE id = ?1",
        params![member, role],
    )?;
    Ok(Some(Applied {
        action: AuditAction::MemberRole,
        target: member.to_string(),
        detail: json!({
            "before": { "status": status, "role": held },
            "after": { "status": "approved", "role": role },
        }),
        ended: vec![Ended {
            member: member.to_string(),
            why: EndedWhy::RightsChanged,
        }],
    }))
}

fn revoke(db: &Connection, member: &str) -> Result<Option<Applied>, Refusal> {
    let (status, role) =
        device(db, member)?.ok_or_else(|| Refusal::NotFound("no such device".into()))?;
    if status == "revoked" {
        return Ok(None);
    }
    db.execute(
        "UPDATE members SET status = 'revoked' WHERE id = ?1",
        [member],
    )?;
    db.execute(
        "UPDATE members SET status = 'revoked' WHERE owner = ?1 AND kind = 'agent'",
        [member],
    )?;
    Ok(Some(Applied {
        action: AuditAction::MemberRevoked,
        target: member.to_string(),
        detail: json!({
            "before": { "status": status, "role": role },
            "after": { "status": "revoked", "role": role },
        }),
        ended: vec![Ended {
            member: member.to_string(),
            why: EndedWhy::Revoked,
        }],
    }))
}

impl Store {
    pub(crate) fn apply(
        &mut self,
        actor: &Principal,
        change: AdminChange,
    ) -> Result<Vec<Ended>, Refusal> {
        let tx = self.db.transaction()?;
        let applied = match change {
            AdminChange::SaveRole {
                id,
                name,
                permissions,
            } => save_role(&tx, id, &name, &permissions)?,
            AdminChange::DeleteRole { id } => delete_role(&tx, &id)?,
            AdminChange::SetRole { member, role } => set_role(&tx, &member, &role)?,
            AdminChange::Revoke { member } => revoke(&tx, &member)?,
        };
        let Some(applied) = applied else {
            return Ok(Vec::new());
        };
        audit(&tx, actor.actor(), &applied)?;
        tx.commit()?;
        Ok(applied.ended)
    }

    pub(crate) fn admin_view(&self) -> Result<AdminView> {
        Ok(AdminView {
            catalog: Permission::ALL
                .iter()
                .map(|p| p.key().to_string())
                .collect(),
            roles: self.role_views()?,
            members: self.member_views()?,
            audit: self.audit_page()?,
        })
    }

    fn role_views(&self) -> Result<Vec<RoleView>> {
        let mut st = self.db.prepare(
            "SELECT r.id, r.name, r.builtin,
                    (SELECT COUNT(*) FROM members m
                      WHERE m.role = r.id AND m.kind = 'person' AND m.status = 'approved')
             FROM roles r",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, u32>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut roles = Vec::new();
        for (id, name, builtin, members) in rows {
            roles.push(RoleView {
                permissions: role_permissions(&self.db, &id)?
                    .into_iter()
                    .map(|p| p.key().to_string())
                    .collect(),
                id,
                name,
                builtin,
                members,
            });
        }
        roles.sort_by_key(|r| {
            (
                BuiltinRole::ALL
                    .iter()
                    .position(|b| b.id() == r.id)
                    .unwrap_or(BuiltinRole::ALL.len()),
                r.name.to_lowercase(),
            )
        });
        Ok(roles)
    }

    fn member_views(&self) -> Result<Vec<MemberView>> {
        let mut st = self.db.prepare(
            "SELECT id, name, status, role FROM members WHERE kind = 'person' ORDER BY created_at",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, name, status, role)| {
                Some(MemberView {
                    id,
                    name,
                    status: MemberStatus::parse(&status)?,
                    role,
                })
            })
            .collect())
    }

    fn audit_page(&self) -> Result<Vec<AuditEntry>> {
        let mut st = self.db.prepare(
            "SELECT seq, at, actor, actor_name, action, target, detail
             FROM audit ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = st
            .query_map([AUDIT_PAGE], |r| {
                Ok((
                    (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?),
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|((seq, at, actor, actor_name), action, target, detail)| {
                Some(AuditEntry {
                    seq,
                    at,
                    actor,
                    actor_name,
                    action: AuditAction::parse(&action)?,
                    target,
                    detail: serde_json::from_str(&detail).unwrap_or(Value::String(detail)),
                })
            })
            .collect())
    }
}

pub(crate) async fn view(State(s): State<Shared>, headers: HeaderMap) -> Reply {
    let store = s.store.lock().unwrap();
    authorize(&store, &headers, Permission::AdminManage)?;
    let view = store.admin_view().map_err(Refusal::Internal)?;
    Ok(Json(json!(view)))
}

pub(crate) async fn change(
    State(s): State<Shared>,
    headers: HeaderMap,
    Json(change): Json<AdminChange>,
) -> Reply {
    let (ended, view) = {
        let mut store = s.store.lock().unwrap();
        let actor = authorize(&store, &headers, Permission::AdminManage)?;
        let ended = store.apply(&actor, change)?;
        (ended, store.admin_view().map_err(Refusal::Internal)?)
    };
    for e in ended {
        s.end(e);
    }
    Ok(Json(json!(view)))
}
