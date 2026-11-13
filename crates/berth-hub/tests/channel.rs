use berth_hub::client::HubClient;
use berth_hub::{serve, Hub};

async fn start(dir: &std::path::Path) -> (Hub, String) {
    let hub = serve(&dir.join("hub.db"), "127.0.0.1:0".parse().unwrap(), Some("admin-secret".into()))
        .await
        .unwrap();
    let url = format!("http://{}", hub.addr);
    (hub, url)
}

async fn person(url: &str, name: &str, scopes: &[&str]) -> (String, HubClient) {
    let (id, token) = HubClient::pair(url, name).await.unwrap();
    let admin = HubClient::new(url, "admin-secret").unwrap();
    admin.approve(&id, scopes).await.unwrap();
    (id, HubClient::new(url, &token).unwrap())
}

#[tokio::test]
async fn two_people_post_and_read_in_one_channel() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(dir.path()).await;
    let (_ann_id, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (bob_id, bob) = person(&url, "bob", &["read", "post"]).await;

    ann.create_channel("PROJ-12").await.unwrap();
    assert!(bob.post("PROJ-12", "too early", None).await.is_err());
    ann.add_member("PROJ-12", &bob_id).await.unwrap();

    ann.post("PROJ-12", "contract: POST /orders", None).await.unwrap();
    let reply = bob.post("PROJ-12", "ack", None).await.unwrap();
    assert_eq!(reply.author_name, "bob");
    assert_eq!(reply.kind, 1);

    let seen = ann.read("PROJ-12", 0, None).await.unwrap();
    let texts: Vec<_> = seen.iter().map(|e| (e.author_name.as_str(), e.content.as_str())).collect();
    assert_eq!(texts, [("ann", "contract: POST /orders"), ("bob", "ack")]);
    assert!(seen[0].seq < seen[1].seq);

    let after = bob.read("PROJ-12", seen[0].seq, None).await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].content, "ack");
}

#[tokio::test]
async fn unread_counts_follow_the_read_mark() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(dir.path()).await;
    let (_a, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (bob_id, bob) = person(&url, "bob", &["read", "post"]).await;
    ann.create_channel("c").await.unwrap();
    ann.add_member("c", &bob_id).await.unwrap();
    ann.post("c", "one", None).await.unwrap();
    assert_eq!(bob.channels().await.unwrap()[0].unread, 1);
    bob.read("c", 0, None).await.unwrap();
    assert_eq!(bob.channels().await.unwrap()[0].unread, 0);
}

#[tokio::test]
async fn an_agent_posts_as_a_member_marked_with_its_owner() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(dir.path()).await;
    let (_a, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (_b, bob) = person(&url, "bob", &["read", "post", "create"]).await;
    ann.create_channel("PROJ-12").await.unwrap();

    let agent = ann.agent("agent-7").await.unwrap();
    let event = ann.post("PROJ-12", "status: tests green", Some(&agent)).await.unwrap();
    assert_eq!(event.author, agent);
    assert_eq!(event.author_kind, "agent");

    assert!(bob.post("PROJ-12", "hijack", Some(&agent)).await.is_err());
    assert!(bob.read("PROJ-12", 0, None).await.is_err());
}

#[tokio::test]
async fn requests_without_approval_or_scope_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(dir.path()).await;
    let (_id, token) = HubClient::pair(&url, "eve").await.unwrap();
    let pending = HubClient::new(&url, &token).unwrap();
    assert!(pending.channels().await.is_err());
    assert!(HubClient::new(&url, "guess").unwrap().channels().await.is_err());
    assert!(pending.pending().await.is_err());

    let (_ann, reader) = person(&url, "ann", &["read"]).await;
    assert!(reader.create_channel("x").await.is_err());
}
