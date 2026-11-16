use berth_hub::client::HubClient;
use berth_hub::{serve_with_ttl, KIND_NOTE};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

async fn person(url: &str, name: &str, scopes: &[&str]) -> (String, HubClient) {
    let (id, token) = HubClient::pair(url, name).await.unwrap();
    let admin = HubClient::new(url, "admin-secret").unwrap();
    admin.approve(&id, scopes).await.unwrap();
    (id, HubClient::new(url, &token).unwrap())
}

fn touching(agent: &str, modules: &[&str]) -> HashMap<String, BTreeMap<String, Vec<String>>> {
    HashMap::from([(
        "PROJ-12".to_string(),
        BTreeMap::from([(
            agent.to_string(),
            modules.iter().map(|m| m.to_string()).collect(),
        )]),
    )])
}

#[tokio::test]
async fn the_latest_note_of_a_channel_is_the_one_in_force_and_an_empty_note_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let hub = serve_with_ttl(
        &dir.path().join("hub.db"),
        "127.0.0.1:0".parse().unwrap(),
        Some("admin-secret".into()),
        Duration::from_secs(45),
    )
    .await
    .unwrap();
    let url = format!("http://{}", hub.addr);
    let (_a, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (bob_id, bob) = person(&url, "bob", &["read", "post"]).await;
    let (_c, carol) = person(&url, "carol", &["read", "post"]).await;
    ann.create_channel("PROJ-12").await.unwrap();
    ann.add_member("PROJ-12", &bob_id).await.unwrap();

    ann.post("PROJ-12", "just talk", None).await.unwrap();
    ann.post_note("PROJ-12", "use the v2 API", None)
        .await
        .unwrap();
    let note = bob
        .post_note("PROJ-12", "  v2 API, and keep the old route  ", None)
        .await
        .unwrap();
    assert_eq!(note.kind, KIND_NOTE);

    let in_force = ann.notes().await.unwrap();
    assert_eq!(in_force.len(), 1);
    assert_eq!(in_force[0].content, "v2 API, and keep the old route");
    assert_eq!(in_force[0].author_name, "bob");
    assert!(carol.notes().await.unwrap().is_empty());
    assert!(carol.post_note("PROJ-12", "hijack", None).await.is_err());

    bob.post_note("PROJ-12", "", None).await.unwrap();
    assert!(ann.notes().await.unwrap().is_empty());

    let too_long = "x".repeat(berth_hub::NOTE_MAX_BYTES + 1);
    assert!(ann.post_note("PROJ-12", &too_long, None).await.is_err());
}

#[tokio::test]
async fn touched_modules_of_other_members_are_shown_and_expire() {
    let dir = tempfile::tempdir().unwrap();
    let hub = serve_with_ttl(
        &dir.path().join("hub.db"),
        "127.0.0.1:0".parse().unwrap(),
        Some("admin-secret".into()),
        Duration::from_millis(400),
    )
    .await
    .unwrap();
    let url = format!("http://{}", hub.addr);
    let (_a, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (bob_id, bob) = person(&url, "bob", &["read", "post"]).await;
    let (_c, carol) = person(&url, "carol", &["read", "post"]).await;
    ann.create_channel("PROJ-12").await.unwrap();
    ann.add_member("PROJ-12", &bob_id).await.unwrap();

    ann.report_touched(&touching("agent-1", &["crates/api", "web/src"]))
        .await
        .unwrap();
    bob.report_touched(&touching("agent-2", &["crates/api"]))
        .await
        .unwrap();
    carol
        .report_touched(&touching("agent-9", &["crates/api"]))
        .await
        .unwrap();

    let seen_by_bob = bob.touched("PROJ-12").await.unwrap();
    assert_eq!(seen_by_bob.len(), 1);
    assert_eq!(seen_by_bob[0].name, "ann");
    assert_eq!(seen_by_bob[0].agent, "agent-1");
    assert_eq!(seen_by_bob[0].modules, ["crates/api", "web/src"]);
    assert!(carol.touched("PROJ-12").await.is_err());

    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(bob.touched("PROJ-12").await.unwrap().is_empty());
}
