use berth_hub::client::HubClient;
use berth_hub::serve;

async fn person(url: &str, name: &str) -> (String, HubClient) {
    let (id, token) = HubClient::pair(url, name).await.unwrap();
    let admin = HubClient::new(url, "admin-secret").unwrap();
    admin
        .approve(&id, &["read", "post", "create"])
        .await
        .unwrap();
    (id, HubClient::new(url, &token).unwrap())
}

#[tokio::test]
async fn a_subscriber_gets_new_events_live_and_catches_up_after_a_reconnect() {
    let dir = tempfile::tempdir().unwrap();
    let hub = serve(
        &dir.path().join("hub.db"),
        "127.0.0.1:0".parse().unwrap(),
        Some("admin-secret".into()),
        None,
    )
    .await
    .unwrap();
    let url = format!("http://{}", hub.addr);
    let (_a, ann) = person(&url, "ann").await;
    let (bob_id, bob) = person(&url, "bob").await;
    ann.create_channel("PROJ-1").await.unwrap();
    ann.create_channel("SECRET").await.unwrap();
    ann.add_member("PROJ-1", &bob_id).await.unwrap();
    let before = ann.post("PROJ-1", "before", None).await.unwrap();

    let mut live = bob.subscribe(None).await.unwrap();
    ann.post("SECRET", "not for bob", None).await.unwrap();
    let first = ann.post("PROJ-1", "one", None).await.unwrap();
    let got = live.next().await.unwrap().unwrap();
    assert_eq!((got.seq, got.content.as_str()), (first.seq, "one"));
    drop(live);

    ann.post("PROJ-1", "two", None).await.unwrap();
    ann.post("SECRET", "still not", None).await.unwrap();
    let mut again = bob.subscribe(Some(before.seq)).await.unwrap();
    let mut seen = Vec::new();
    for _ in 0..2 {
        seen.push(again.next().await.unwrap().unwrap().content);
    }
    ann.post("PROJ-1", "three", None).await.unwrap();
    seen.push(again.next().await.unwrap().unwrap().content);
    assert_eq!(seen, ["one", "two", "three"]);

    let stranger = HubClient::new(&url, "guess").unwrap();
    assert!(stranger.subscribe(None).await.is_err());
}
