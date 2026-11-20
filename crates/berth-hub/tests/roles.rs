use berth_hub::client::HubClient;
use berth_hub::{
    serve, AdminChange, AdminView, AuditAction, Ceiling, Hub, MemberStatus, Next, Permission,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

async fn start(db: &Path, admin_token: Option<&str>) -> (Hub, String) {
    let hub = serve(
        db,
        "127.0.0.1:0".parse().unwrap(),
        admin_token.map(String::from),
        None,
    )
    .await
    .unwrap();
    let url = format!("http://{}", hub.addr);
    (hub, url)
}

fn admin(url: &str) -> HubClient {
    HubClient::new(url, "admin-secret").unwrap()
}

async fn set_role(url: &str, member: &str, role: &str) {
    admin(url)
        .admin_change(&AdminChange::SetRole {
            member: member.into(),
            role: role.into(),
        })
        .await
        .unwrap();
}

async fn person(url: &str, name: &str, role: &str) -> (String, HubClient) {
    let (id, token) = HubClient::pair(url, name).await.unwrap();
    set_role(url, &id, role).await;
    (id, HubClient::new(url, &token).unwrap())
}

fn save_role(id: Option<&str>, name: &str, permissions: &[&str]) -> AdminChange {
    AdminChange::SaveRole {
        id: id.map(String::from),
        name: name.into(),
        permissions: permissions.iter().map(|p| p.to_string()).collect(),
    }
}

fn role_id(view: &AdminView, name: &str) -> String {
    view.roles
        .iter()
        .find(|r| r.name == name)
        .unwrap()
        .id
        .clone()
}

async fn next(subscription: &mut berth_hub::Subscription) -> Next {
    tokio::time::timeout(Duration::from_secs(5), subscription.next())
        .await
        .expect("the stream stays open")
        .unwrap()
}

#[tokio::test]
async fn a_guest_is_refused_a_channel_until_the_admin_makes_it_a_member() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let (gus_id, gus) = person(&url, "gus", "guest").await;

    let refused = gus.create_channel("PROJ-1").await.unwrap_err().to_string();
    assert!(refused.contains("(403)"), "{refused}");
    assert!(
        refused.contains("`Guest`") && refused.contains("`channels.create`"),
        "{refused}"
    );

    set_role(&url, &gus_id, "member").await;
    gus.create_channel("PROJ-1").await.unwrap();
    assert_eq!(gus.me().await.unwrap().role.unwrap().name, "Member");
}

#[tokio::test]
async fn a_person_with_the_admin_role_manages_the_hub_with_its_own_token() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let (ann_id, ann) = person(&url, "ann", "admin").await;
    let (bob_id, bob) = person(&url, "bob", "member").await;

    let refused = bob.admin_view().await.unwrap_err().to_string();
    assert!(
        refused.contains("(403)") && refused.contains("`admin.manage`"),
        "{refused}"
    );
    assert!(admin(&url)
        .me()
        .await
        .unwrap_err()
        .to_string()
        .contains("(403)"));

    assert!(ann.me().await.unwrap().holds(Permission::AdminManage));
    let view = ann.admin_view().await.unwrap();
    assert!(view.members.iter().any(|m| m.id == bob_id));
    let after = ann
        .admin_change(&AdminChange::SetRole {
            member: bob_id.clone(),
            role: "guest".into(),
        })
        .await
        .unwrap();
    let bob_row = after.members.iter().find(|m| m.id == bob_id).unwrap();
    assert_eq!(bob_row.role.as_deref(), Some("guest"));
    assert_eq!(
        (
            after.audit[0].actor.as_str(),
            after.audit[0].actor_name.as_str()
        ),
        (ann_id.as_str(), "ann")
    );
}

#[tokio::test]
async fn custom_roles_are_saved_and_built_in_or_held_roles_are_protected() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let admin = admin(&url);

    let view = admin
        .admin_change(&save_role(None, "Reviewer", &["channels.read"]))
        .await
        .unwrap();
    let reviewer = view.roles.iter().find(|r| r.name == "Reviewer").unwrap();
    assert!(!reviewer.builtin);
    assert_eq!(reviewer.permissions, ["channels.read"]);
    let reviewer = reviewer.id.clone();

    let failure = |e: anyhow::Error| e.to_string();
    let unknown = admin
        .admin_change(&save_role(None, "Odd", &["channels.fly"]))
        .await
        .map_err(failure);
    assert!(unknown.unwrap_err().contains("(400)"));
    let taken = admin
        .admin_change(&save_role(None, "reviewer", &[]))
        .await
        .map_err(failure);
    assert!(taken.unwrap_err().contains("(409)"));
    let builtin_saved = admin
        .admin_change(&save_role(Some("member"), "Member", &[]))
        .await
        .map_err(failure);
    assert!(builtin_saved.unwrap_err().contains("(409)"));
    let builtin_deleted = admin
        .admin_change(&AdminChange::DeleteRole { id: "guest".into() })
        .await
        .map_err(failure);
    assert!(builtin_deleted.unwrap_err().contains("(409)"));

    let (rita_id, rita) = person(&url, "rita", &reviewer).await;
    let refused = rita.create_channel("PROJ-1").await.unwrap_err().to_string();
    assert!(refused.contains("`Reviewer`"), "{refused}");
    let held = admin
        .admin_change(&AdminChange::DeleteRole {
            id: reviewer.clone(),
        })
        .await
        .map_err(failure);
    assert!(held.unwrap_err().contains("(409)"));

    set_role(&url, &rita_id, "member").await;
    let view = admin
        .admin_change(&AdminChange::DeleteRole { id: reviewer })
        .await
        .unwrap();
    assert!(view.roles.iter().all(|r| r.name != "Reviewer"));
}

#[tokio::test]
async fn every_applied_change_writes_one_audit_row_and_a_no_op_writes_none() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let admin = admin(&url);
    let (bob, _) = HubClient::pair(&url, "bob").await.unwrap();
    let mut rows = admin.admin_view().await.unwrap().audit.len();
    let mut expect = |view: &AdminView, added: usize| {
        assert_eq!(view.audit.len(), rows + added);
        rows = view.audit.len();
    };

    let set_member = AdminChange::SetRole {
        member: bob.clone(),
        role: "member".into(),
    };
    let view = admin.admin_change(&set_member).await.unwrap();
    expect(&view, 1);
    assert_eq!(
        (
            view.audit[0].action,
            view.audit[0].target.as_str(),
            view.audit[0].actor.as_str()
        ),
        (AuditAction::MemberRole, bob.as_str(), "operator")
    );
    expect(&admin.admin_change(&set_member).await.unwrap(), 0);

    let view = admin
        .admin_change(&save_role(None, "Reviewer", &["channels.read"]))
        .await
        .unwrap();
    expect(&view, 1);
    let reviewer = role_id(&view, "Reviewer");
    let same = save_role(Some(&reviewer), "Reviewer", &["channels.read"]);
    expect(&admin.admin_change(&same).await.unwrap(), 0);
    let wider = save_role(
        Some(&reviewer),
        "Reviewer",
        &["channels.read", "channels.post"],
    );
    expect(&admin.admin_change(&wider).await.unwrap(), 1);

    assert!(admin
        .admin_change(&AdminChange::DeleteRole {
            id: "member".into()
        })
        .await
        .is_err());
    expect(&admin.admin_view().await.unwrap(), 0);

    let revoke = AdminChange::Revoke {
        member: bob.clone(),
    };
    expect(&admin.admin_change(&revoke).await.unwrap(), 1);
    expect(&admin.admin_change(&revoke).await.unwrap(), 0);
    let deleted = AdminChange::DeleteRole { id: reviewer };
    expect(&admin.admin_change(&deleted).await.unwrap(), 1);
}

#[tokio::test]
async fn a_hub_db_with_scopes_is_migrated_once_and_its_approved_devices_become_members() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hub.db");
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(
            "CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT NOT NULL);
             CREATE TABLE members (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL,
               owner TEXT, token TEXT NOT NULL, status TEXT NOT NULL,
               scopes TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL, display TEXT);
             CREATE TABLE channels (key TEXT PRIMARY KEY, created_at INTEGER NOT NULL);
             CREATE TABLE channel_members (
               channel TEXT NOT NULL, member TEXT NOT NULL, last_read INTEGER NOT NULL DEFAULT 0,
               PRIMARY KEY (channel, member));
             CREATE TABLE events (
               seq INTEGER PRIMARY KEY AUTOINCREMENT, kind INTEGER NOT NULL, author TEXT NOT NULL,
               channel TEXT NOT NULL, tags TEXT NOT NULL, content TEXT NOT NULL,
               created_at INTEGER NOT NULL);
             INSERT INTO meta (k, v) VALUES ('admin_token', 'admin-secret');
             INSERT INTO members (id, name, kind, token, status, scopes, created_at) VALUES
               ('m-reader', 'ann', 'person', 'tok-reader', 'approved', 'read', 1),
               ('m-pending', 'eve', 'person', 'tok-pending', 'pending', '', 2),
               ('m-revoked', 'mal', 'person', 'tok-revoked', 'revoked', 'read,post,create', 3);",
        )
        .unwrap();

    let (hub, url) = start(&db, None).await;
    assert_eq!(hub.admin_token, "admin-secret");
    let ann = HubClient::new(&url, "tok-reader").unwrap();
    assert_eq!(ann.me().await.unwrap().role.unwrap().id, "member");
    ann.create_channel("PROJ-1").await.unwrap();
    ann.post("PROJ-1", "now I can post", None).await.unwrap();
    for refused in ["tok-pending", "tok-revoked"] {
        let e = HubClient::new(&url, refused)
            .unwrap()
            .channels()
            .await
            .unwrap_err();
        assert!(e.to_string().contains("(401)"), "{e}");
    }

    let view = admin(&url).admin_view().await.unwrap();
    let members: Vec<_> = view
        .members
        .iter()
        .map(|m| (m.id.as_str(), m.status, m.role.as_deref()))
        .collect();
    assert_eq!(
        members,
        [
            ("m-reader", MemberStatus::Approved, Some("member")),
            ("m-pending", MemberStatus::Pending, None),
            ("m-revoked", MemberStatus::Revoked, None),
        ]
    );
    let migrated = |v: &AdminView| {
        v.audit
            .iter()
            .filter(|a| a.action == AuditAction::HubMigrated)
            .count()
    };
    assert_eq!(migrated(&view), 1);

    let backup = dir.path().join("hub.db.v1.bak");
    let scopes_in_backup: bool = rusqlite::Connection::open(&backup)
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('members') WHERE name = 'scopes')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(scopes_in_backup);
    let backup_bytes = std::fs::read(&backup).unwrap();

    let (_again, again_url) = start(&db, None).await;
    let reopened = admin(&again_url).admin_view().await.unwrap();
    assert_eq!(reopened, view);
    assert_eq!(std::fs::read(&backup).unwrap(), backup_bytes);
}

#[tokio::test]
async fn a_rights_change_closes_the_stream_with_rights_changed_and_a_revoke_closes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let admin = admin(&url);
    let (bob_id, bob) = person(&url, "bob", "member").await;

    let mut live = bob.subscribe(None).await.unwrap();
    set_role(&url, &bob_id, "guest").await;
    assert_eq!(next(&mut live).await, Next::RightsChanged);

    let view = admin
        .admin_change(&save_role(None, "Reviewer", &["channels.read"]))
        .await
        .unwrap();
    let reviewer = role_id(&view, "Reviewer");
    set_role(&url, &bob_id, &reviewer).await;
    let mut live = bob.subscribe(None).await.unwrap();
    admin
        .admin_change(&save_role(
            Some(&reviewer),
            "Reviewer",
            &["channels.read", "channels.post"],
        ))
        .await
        .unwrap();
    assert_eq!(next(&mut live).await, Next::RightsChanged);

    let mut live = bob.subscribe(None).await.unwrap();
    admin
        .admin_change(&AdminChange::Revoke { member: bob_id })
        .await
        .unwrap();
    assert_eq!(next(&mut live).await, Next::Closed);
}

#[tokio::test]
async fn the_instruct_ceiling_allows_authors_whose_role_instructs_and_who_share_a_channel() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let (_ann_id, ann) = person(&url, "ann", "member").await;
    let (gus_id, gus) = person(&url, "gus", "guest").await;
    let (bob_id, bob) = person(&url, "bob", "member").await;
    let (stranger_id, _) = person(&url, "stranger", "member").await;
    ann.create_channel("PROJ-1").await.unwrap();
    ann.add_member("PROJ-1", &gus_id).await.unwrap();
    ann.add_member("PROJ-1", &bob_id).await.unwrap();
    let bobs_agent = bob.agent("agent-b").await.unwrap();

    let asked = [
        gus_id.clone(),
        bob_id.clone(),
        bobs_agent.clone(),
        stranger_id,
    ];
    assert_eq!(
        ann.ceiling(&asked).await.unwrap(),
        Ceiling::Only(BTreeSet::from([bob_id, bobs_agent]))
    );
    assert!(gus.ceiling(&asked).await.is_err());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let old_hub = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, axum::Router::new()).await });
    let unlimited = HubClient::new(&old_hub, "token")
        .unwrap()
        .ceiling(&asked)
        .await
        .unwrap();
    assert_eq!(unlimited, Ceiling::Unlimited);
}

#[tokio::test]
async fn a_member_demoted_to_guest_can_no_longer_post_as_its_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let (bob_id, bob) = person(&url, "bob", "member").await;
    bob.create_channel("PROJ-1").await.unwrap();
    let agent = bob.agent("agent-b").await.unwrap();
    bob.post("PROJ-1", "as my agent", Some(&agent))
        .await
        .unwrap();

    set_role(&url, &bob_id, "guest").await;
    let refused = bob
        .post("PROJ-1", "still?", Some(&agent))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("(403)") && refused.contains("`agents.join`"),
        "{refused}"
    );
    bob.post("PROJ-1", "as myself", None).await.unwrap();
}

#[tokio::test]
async fn the_catalog_json_matches_the_permission_enum_and_seeds_the_built_in_roles() {
    let catalog: BTreeMap<String, Value> =
        serde_json::from_str(include_str!("../permissions.json")).unwrap();
    let keys: BTreeSet<&str> = catalog.keys().map(String::as_str).collect();
    let enumerated: BTreeSet<&str> = Permission::ALL.iter().map(|p| p.key()).collect();
    assert_eq!(keys, enumerated);

    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(&dir.path().join("hub.db"), Some("admin-secret")).await;
    let view = admin(&url).admin_view().await.unwrap();
    let builtin: Vec<&str> = view
        .roles
        .iter()
        .filter(|r| r.builtin)
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(builtin, ["admin", "member", "guest"]);
    for role in view.roles.iter().filter(|r| r.builtin) {
        let expected: BTreeSet<&str> = catalog
            .iter()
            .filter(|(_, entry)| {
                role.id == "admin"
                    || entry["builtin"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|r| r == role.id.as_str())
            })
            .map(|(key, _)| key.as_str())
            .collect();
        let seeded: BTreeSet<&str> = role.permissions.iter().map(String::as_str).collect();
        assert_eq!(seeded, expected, "{}", role.id);
    }
}
