use super::mock::mock_agent_args;
use super::providers::AcpProvider;
use crate::hooks::HookEnv;
use crate::registry::Daemon;
use crate::store::Store;
use berth_core::acp::AcpItem;
use berth_core::{AgentStatus, EntityId, ServerEvent};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

const EVENT_WAIT: Duration = Duration::from_secs(30);

fn git(repo: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
}

fn update(kind: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut update = json!({"sessionUpdate": kind});
    update
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    json!({"jsonrpc": "2.0", "method": "session/update",
        "params": {"sessionId": "mock-1", "update": update}})
}

fn script() -> serde_json::Value {
    json!([
        {"expect": "initialize", "result": {"protocolVersion": 1, "agentCapabilities": {}, "authMethods": []}},
        {"expect": "session/new", "result": {"sessionId": "mock-1"}},
        {"expect": "session/prompt"},
        {"send": update("agent_message_chunk", json!({"content": {"type": "text", "text": "Let me "}}))},
        {"send": update("agent_message_chunk", json!({"content": {"type": "text", "text": "check."}}))},
        {"send": update("tool_call", json!({"toolCallId": "t1", "title": "Run tests", "kind": "execute",
            "status": "pending", "rawInput": {"command": "cargo test"}}))},
        {"send": {"jsonrpc": "2.0", "id": 99, "method": "session/request_permission", "params": {
            "sessionId": "mock-1", "toolCall": {"toolCallId": "t1", "title": "Run tests",
                "rawInput": {"command": "cargo test"}},
            "options": [
                {"optionId": "allow-once", "name": "Allow once", "kind": "allow_once"},
                {"optionId": "reject-once", "name": "Reject", "kind": "reject_once"}
            ]}}},
        {"await_response": true},
        {"send": update("tool_call_update", json!({"toolCallId": "t1", "status": "completed",
            "content": [{"type": "content", "content": {"type": "text", "text": "ok"}}]}))},
        {"send": update("plan", json!({"entries": [
            {"content": "Run tests", "priority": "high", "status": "completed"}]}))},
        {"reply_to": "session/prompt", "result": {"stopReason": "end_turn"}}
    ])
}

async fn next_event(
    events: &mut broadcast::Receiver<ServerEvent>,
    mut wanted: impl FnMut(&ServerEvent) -> bool,
    statuses: &mut Vec<AgentStatus>,
) -> ServerEvent {
    tokio::time::timeout(EVENT_WAIT, async {
        loop {
            let event = events.recv().await.unwrap();
            if let ServerEvent::StatusChanged { status, .. } = &event {
                statuses.push(*status);
            }
            if wanted(&event) {
                return event;
            }
        }
    })
    .await
    .expect("event did not arrive")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_runs_through_a_permission_answer_to_finished() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = berth_core::paths::canonical_or_raw(tmp.path()).join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);
    let script_path = tmp.path().join("script.json");
    std::fs::write(&script_path, script().to_string()).unwrap();

    let daemon = Daemon::new(
        Arc::new(Store::open_in_memory().unwrap()),
        HookEnv {
            port: 0,
            token: String::new(),
        },
    );
    let EntityId::Project(project) = daemon.add_project(&repo, None, false, None).await.unwrap()
    else {
        panic!("expected a project id");
    };
    let (_, worktrees, _, _) = daemon.store.load_tree().unwrap();
    let worktree = worktrees.iter().find(|w| w.project_id == project).unwrap();
    let mut events = daemon.events.subscribe();

    let provider = AcpProvider {
        id: "mock".into(),
        name: "Mock".into(),
        command: std::env::current_exe().unwrap().display().to_string(),
        args: mock_agent_args(&script_path),
        enabled: true,
    };
    let EntityId::Agent(agent) = daemon
        .create_acp_agent_with(worktree.id.clone(), "mock".into(), provider, false)
        .unwrap()
    else {
        panic!("expected an agent id");
    };
    assert_eq!(
        daemon.store.get_agent(&agent).unwrap().unwrap().status,
        AgentStatus::Fresh
    );
    assert!(daemon.is_alive(&berth_core::SessionRef::Agent(agent.clone())));

    let mut statuses = Vec::new();
    daemon.acp_prompt(&agent, "run the tests").await.unwrap();
    assert_eq!(
        daemon
            .store
            .get_agent(&agent)
            .unwrap()
            .unwrap()
            .session_id
            .as_deref(),
        Some("mock-1")
    );
    let ServerEvent::FeedbackRequestChanged {
        request: Some(request),
        ..
    } = next_event(
        &mut events,
        |e| {
            matches!(
                e,
                ServerEvent::FeedbackRequestChanged {
                    request: Some(_),
                    ..
                }
            )
        },
        &mut statuses,
    )
    .await
    else {
        unreachable!()
    };
    assert_eq!(request.title, "Run tests");
    assert_eq!(request.detail.as_deref(), Some("cargo test"));
    assert_eq!(request.options, ["Allow once", "Reject"]);
    assert!(daemon.answer_feedback(&agent, 5).await.is_err());

    daemon.answer_feedback(&agent, 0).await.unwrap();
    next_event(
        &mut events,
        |e| {
            matches!(
                e,
                ServerEvent::StatusChanged {
                    status: AgentStatus::Finished,
                    ..
                }
            )
        },
        &mut statuses,
    )
    .await;
    assert_eq!(
        statuses,
        [
            AgentStatus::Running,
            AgentStatus::NeedsFeedback,
            AgentStatus::Running,
            AgentStatus::Finished
        ]
    );

    let items: Vec<AcpItem> = daemon
        .acp_transcript(&agent, 0)
        .unwrap()
        .into_iter()
        .map(|i| i.item)
        .collect();
    assert_eq!(
        items[0],
        AcpItem::User {
            text: "run the tests".into()
        }
    );
    assert_eq!(
        items[1],
        AcpItem::Assistant {
            text: "Let me check.".into()
        }
    );
    let AcpItem::Tool(tool) = &items[2] else {
        panic!("expected a tool item, got {:?}", items[2])
    };
    assert_eq!(
        (tool.status.as_str(), tool.output.as_deref()),
        ("completed", Some("ok"))
    );
    assert!(matches!(&items[3], AcpItem::Plan { entries } if entries.len() == 1));
    assert_eq!(items.len(), 4);
    assert_eq!(daemon.list_checkpoints(&agent).await.unwrap().len(), 1);

    assert!(daemon.acp_prompt(&agent, "  ").await.is_err());
    daemon.stop_agent_or_its_acp_turn(&agent).await.unwrap();
    assert!(!daemon.is_alive(&berth_core::SessionRef::Agent(agent.clone())));
}

async fn mock_daemon(tmp: &Path) -> (Arc<Daemon>, berth_core::WorktreeId, std::path::PathBuf) {
    let repo = berth_core::paths::canonical_or_raw(tmp).join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);
    let daemon = Daemon::new(
        Arc::new(Store::open_in_memory().unwrap()),
        HookEnv {
            port: 0,
            token: String::new(),
        },
    );
    let EntityId::Project(project) = daemon.add_project(&repo, None, false, None).await.unwrap()
    else {
        panic!("expected a project id");
    };
    let (_, worktrees, _, _) = daemon.store.load_tree().unwrap();
    let worktree = worktrees.iter().find(|w| w.project_id == project).unwrap();
    (daemon, worktree.id.clone(), repo)
}

fn mock_provider(tmp: &Path, name: &str, script: serde_json::Value) -> AcpProvider {
    let script_path = tmp.join(format!("{name}.json"));
    std::fs::write(&script_path, script.to_string()).unwrap();
    AcpProvider {
        id: "mock".into(),
        name: "Mock".into(),
        command: std::env::current_exe().unwrap().display().to_string(),
        args: mock_agent_args(&script_path),
        enabled: true,
    }
}

fn turn(reply: &str) -> Vec<serde_json::Value> {
    vec![
        json!({"expect": "session/prompt"}),
        json!({"send": update("agent_message_chunk", json!({"content": {"type": "text", "text": reply}}))}),
        json!({"reply_to": "session/prompt", "result": {"stopReason": "end_turn"}}),
    ]
}

async fn until_finished(events: &mut broadcast::Receiver<ServerEvent>) {
    next_event(
        events,
        |e| {
            matches!(
                e,
                ServerEvent::StatusChanged {
                    status: AgentStatus::Finished,
                    ..
                }
            )
        },
        &mut Vec::new(),
    )
    .await;
}

fn items_of(daemon: &Daemon, agent: &berth_core::AgentId) -> Vec<AcpItem> {
    daemon
        .acp_transcript(agent, 0)
        .unwrap()
        .into_iter()
        .map(|i| i.item)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_respawned_agent_loads_its_session_without_duplicating_history() {
    let tmp = tempfile::tempdir().unwrap();
    let (daemon, worktree, repo) = mock_daemon(tmp.path()).await;
    let mut events = daemon.events.subscribe();
    let mut first = vec![
        json!({"expect": "initialize", "result": {"protocolVersion": 1,
            "agentCapabilities": {"loadSession": true}, "authMethods": []}}),
        json!({"expect": "session/new", "result": {"sessionId": "mock-1"}}),
    ];
    first.extend(turn("first answer"));
    let provider = mock_provider(tmp.path(), "first", json!(first));
    let EntityId::Agent(agent) = daemon
        .create_acp_agent_with(worktree, "mock".into(), provider, false)
        .unwrap()
    else {
        panic!("expected an agent id");
    };
    daemon.acp_prompt(&agent, "one").await.unwrap();
    until_finished(&mut events).await;
    daemon.acp.kill(&agent);

    let mut second = vec![
        json!({"expect": "initialize", "result": {"protocolVersion": 1,
            "agentCapabilities": {"loadSession": true}, "authMethods": []}}),
        json!({"expect": "session/load"}),
        json!({"send": update("user_message_chunk", json!({"content": {"type": "text", "text": "one"}}))}),
        json!({"send": update("agent_message_chunk", json!({"content": {"type": "text", "text": "first answer"}}))}),
        json!({"reply_to": "session/load", "result": {}}),
    ];
    second.extend(turn("second answer"));
    let provider = mock_provider(tmp.path(), "second", json!(second));
    let program = provider.program().unwrap();
    daemon
        .acp_start(&agent, &repo, &provider, &program, Some("mock-1".into()))
        .unwrap();
    daemon.acp_prompt(&agent, "two").await.unwrap();
    until_finished(&mut events).await;

    assert_eq!(
        items_of(&daemon, &agent),
        [
            AcpItem::User { text: "one".into() },
            AcpItem::Assistant {
                text: "first answer".into()
            },
            AcpItem::User { text: "two".into() },
            AcpItem::Assistant {
                text: "second answer".into()
            },
        ]
    );
    daemon.acp.kill(&agent);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_agent_that_needs_sign_in_asks_and_starts_after_the_answer() {
    let tmp = tempfile::tempdir().unwrap();
    let (daemon, worktree, _) = mock_daemon(tmp.path()).await;
    let mut events = daemon.events.subscribe();
    let mut script = vec![
        json!({"expect": "initialize", "result": {"protocolVersion": 1, "agentCapabilities": {},
            "authMethods": [{"id": "agent_login", "name": "Log in with the browser"}]}}),
        json!({"expect": "session/new", "error": {"code": -32000, "message": "Authentication required"}}),
        json!({"expect": "authenticate", "result": {}}),
        json!({"expect": "session/new", "result": {"sessionId": "mock-2"}}),
    ];
    script.extend(turn("signed in"));
    let provider = mock_provider(tmp.path(), "auth", json!(script));
    let EntityId::Agent(agent) = daemon
        .create_acp_agent_with(worktree, "mock".into(), provider, false)
        .unwrap()
    else {
        panic!("expected an agent id");
    };
    let ServerEvent::FeedbackRequestChanged {
        request: Some(request),
        ..
    } = next_event(
        &mut events,
        |e| {
            matches!(
                e,
                ServerEvent::FeedbackRequestChanged {
                    request: Some(_),
                    ..
                }
            )
        },
        &mut Vec::new(),
    )
    .await
    else {
        unreachable!()
    };
    assert_eq!(request.title, "Sign in to Mock");
    assert_eq!(request.options, ["Log in with the browser"]);
    assert!(daemon.acp_prompt(&agent, "too early").await.is_err());

    daemon.answer_feedback(&agent, 0).await.unwrap();
    daemon.acp_prompt(&agent, "hello").await.unwrap();
    until_finished(&mut events).await;
    assert_eq!(
        daemon
            .store
            .get_agent(&agent)
            .unwrap()
            .unwrap()
            .session_id
            .as_deref(),
        Some("mock-2")
    );
    daemon.acp.kill(&agent);
}
