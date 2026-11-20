use berth_hub::client::HubClient;
use berth_hub::{serve, AdminChange, EVENT_TAG, KIND_ANSWER, KIND_REQUEST, KIND_SETTLED};

async fn person(url: &str, name: &str) -> (String, HubClient) {
    let (id, token) = HubClient::pair(url, name).await.unwrap();
    let admin = HubClient::new(url, "admin-secret").unwrap();
    admin
        .admin_change(&AdminChange::SetRole {
            member: id.clone(),
            role: "member".into(),
        })
        .await
        .unwrap();
    (id, HubClient::new(url, &token).unwrap())
}

#[tokio::test]
async fn a_request_is_posted_by_an_agent_answered_by_a_person_and_settled_by_the_agent() {
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
    let (ann_id, ann) = person(&url, "ann").await;
    let (bob_id, bob) = person(&url, "bob").await;
    ann.create_channel("PROJ-12").await.unwrap();
    ann.add_member("PROJ-12", &bob_id).await.unwrap();
    let agent = ann.agent("agent-1").await.unwrap();
    assert_eq!(ann.me().await.unwrap().id, ann_id);

    assert!(ann
        .publish("PROJ-12", KIND_REQUEST, "Bash", &[], None)
        .await
        .is_err());
    let request = ann
        .publish(
            "PROJ-12",
            KIND_REQUEST,
            "Bash @agent-1",
            &[vec!["request".into(), "permission".into()]],
            Some(&agent),
        )
        .await
        .unwrap();
    assert_eq!(request.kind, KIND_REQUEST);
    assert_eq!(request.author, agent);
    assert_eq!(request.tags, [vec!["request", "permission"]]);
    assert!(ann.mentions(0).await.unwrap().is_empty());

    let refers = vec![vec![EVENT_TAG.to_string(), request.seq.to_string()]];
    assert!(ann
        .publish("PROJ-12", KIND_ANSWER, "allow", &refers, Some(&agent))
        .await
        .is_err());
    let from_bob = bob
        .publish("PROJ-12", KIND_ANSWER, "allow", &refers, None)
        .await
        .unwrap();
    assert_eq!(from_bob.author, bob_id);
    ann.publish("PROJ-12", KIND_SETTLED, "local", &refers, Some(&agent))
        .await
        .unwrap();
    assert!(bob.publish("PROJ-12", 9, "?", &[], None).await.is_err());

    let bobs_agent = bob.agent("agent-2").await.unwrap();
    bob.post("PROJ-12", "@agent-1 next", Some(&bobs_agent))
        .await
        .unwrap();
    let mention = ann.mentions(request.seq).await.unwrap();
    assert_eq!(mention.len(), 1);
    assert_eq!(mention[0].chain, 1);
}
