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

#[tokio::test]
async fn a_mention_reaches_only_the_owner_of_that_agent_and_counts_the_agent_chain() {
    let dir = tempfile::tempdir().unwrap();
    let (_hub, url) = start(dir.path()).await;
    let (_a, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (bob_id, bob) = person(&url, "bob", &["read", "post"]).await;
    ann.create_channel("PROJ-12").await.unwrap();
    ann.add_member("PROJ-12", &bob_id).await.unwrap();
    let a = ann.agent("agent-a").await.unwrap();
    let b = bob.agent("agent-b").await.unwrap();

    bob.post("PROJ-12", "@agent-a please look", None).await.unwrap();
    ann.post("PROJ-12", "@agent-a do it", None).await.unwrap();
    ann.post("PROJ-12", "nobody mentioned here", None).await.unwrap();
    let seen = ann.mentions(0).await.unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!((seen[0].agent.as_str(), seen[0].from_owner, seen[0].chain), (a.as_str(), false, 0));
    assert_eq!((seen[1].agent_name.as_str(), seen[1].from_owner), ("agent-a", true));
    assert!(bob.mentions(0).await.unwrap().is_empty());
    assert!(ann.mentions(seen[1].event.seq).await.unwrap().is_empty());

    bob.post("PROJ-12", "@agent-a from my agent", Some(&b)).await.unwrap();
    ann.post("PROJ-12", "@agent-b and back", Some(&a)).await.unwrap();
    bob.post("PROJ-12", "@agent-a again", Some(&b)).await.unwrap();
    let chains: Vec<u32> = ann.mentions(seen[1].event.seq).await.unwrap().iter().map(|m| m.chain).collect();
    assert_eq!(chains, [1, 3]);
    let back = bob.mentions(0).await.unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].chain, 2);

    ann.post("PROJ-12", "stop", None).await.unwrap();
    bob.post("PROJ-12", "@agent-a human again", None).await.unwrap();
    assert_eq!(ann.mentions(0).await.unwrap().last().unwrap().chain, 0);
}

#[tokio::test]
async fn presence_shows_who_runs_sessions_on_a_task_and_expires() {
    let dir = tempfile::tempdir().unwrap();
    let hub = berth_hub::serve_with_ttl(
        &dir.path().join("hub.db"),
        "127.0.0.1:0".parse().unwrap(),
        Some("admin-secret".into()),
        std::time::Duration::from_millis(400),
    )
    .await
    .unwrap();
    let url = format!("http://{}", hub.addr);
    let (_a, ann) = person(&url, "ann", &["read", "post", "create"]).await;
    let (bob_id, bob) = person(&url, "bob", &["read", "post"]).await;
    let (_c, carol) = person(&url, "carol", &["read", "post"]).await;
    ann.create_channel("PROJ-12").await.unwrap();
    ann.add_member("PROJ-12", &bob_id).await.unwrap();

    let on = |agents: &[&str]| {
        std::collections::HashMap::from([(
            "PROJ-12".to_string(),
            agents.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
        )])
    };
    ann.report_presence(&on(&["agent-1", "agent-2"])).await.unwrap();
    carol.report_presence(&on(&["agent-9"])).await.unwrap();

    let here = bob.presence("PROJ-12").await.unwrap();
    assert_eq!(here.len(), 1);
    assert_eq!(here[0].name, "ann");
    assert_eq!(here[0].agents, ["agent-1", "agent-2"]);
    assert!(carol.presence("PROJ-12").await.is_err());

    ann.report_presence(&on(&[])).await.unwrap();
    assert!(bob.presence("PROJ-12").await.unwrap().is_empty());

    ann.report_presence(&on(&["agent-1"])).await.unwrap();
    assert_eq!(bob.presence("PROJ-12").await.unwrap().len(), 1);
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    assert!(bob.presence("PROJ-12").await.unwrap().is_empty());
}
