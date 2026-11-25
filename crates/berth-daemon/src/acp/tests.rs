use super::mock::mock_agent_args;
use super::providers::AcpProvider;
use super::replay::daemon_in;
use super::wire::DaemonHost;
use crate::registry::Daemon;
use berth_core::acp::AcpItem;
use berth_core::{AgentId, AgentStatus, Backend, ServerEvent};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

const EVENT_WAIT: Duration = Duration::from_secs(30);
const TOOLS: usize = 300;

fn update(kind: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut update = json!({"sessionUpdate": kind});
    update
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    json!({"jsonrpc": "2.0", "method": "session/update",
        "params": {"sessionId": "mock-1", "update": update}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_finishes_only_after_the_answer_sent_before_its_reply() {
    let tmp = tempfile::tempdir().unwrap();
    let (daemon, worktree, _) = daemon_in(tmp.path()).await.unwrap();
    let mut events = daemon.events.subscribe();
    let mut script = vec![
        json!({"expect": "initialize", "result": {"protocolVersion": 1, "agentCapabilities": {}, "authMethods": []}}),
        json!({"expect": "session/new", "result": {"sessionId": "mock-1"}}),
        json!({"expect": "session/prompt"}),
    ];
    for n in 0..TOOLS {
        script.push(
            json!({"send": update("tool_call", json!({"toolCallId": format!("t{n}"),
            "title": "Read", "kind": "read", "status": "completed"}))}),
        );
    }
    script.push(json!({"send": update("agent_message_chunk", json!({"content": {"type": "text", "text": "done"}}))}));
    script.push(json!({"reply_to": "session/prompt", "result": {"stopReason": "end_turn"}}));
    let script_path = tmp.path().join("burst.json");
    std::fs::write(&script_path, json!(script).to_string()).unwrap();
    let provider = AcpProvider {
        id: "mock".into(),
        name: "Mock".into(),
        command: std::env::current_exe().unwrap().display().to_string(),
        args: mock_agent_args(&script_path),
        driver: berth_core::acp::ACP_DRIVER.into(),
        enabled: true,
    };
    let agent = AgentId::generate();
    let host = Arc::new(DaemonHost {
        agent: agent.clone(),
    });
    daemon
        .create_acp_agent_with(
            agent.clone(),
            worktree,
            "mock".into(),
            provider,
            false,
            host,
        )
        .unwrap();
    daemon.acp_prompt(&agent, "go").await.unwrap();
    tokio::time::timeout(EVENT_WAIT, async {
        loop {
            if let ServerEvent::StatusChanged {
                status: AgentStatus::Finished,
                ..
            } = events.recv().await.unwrap()
            {
                return;
            }
        }
    })
    .await
    .expect("the turn did not finish");

    let items: Vec<AcpItem> = daemon
        .acp_transcript(&agent, 0)
        .unwrap()
        .into_iter()
        .map(|i| i.item)
        .collect();
    assert_eq!(items.len(), TOOLS + 2, "{:?}", items.last());
    assert_eq!(
        items.last(),
        Some(&AcpItem::Assistant {
            text: "done".into()
        })
    );
    daemon.acp.kill(&agent);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_agent_moves_to_its_terminal_and_back_to_its_chat() {
    let tmp = tempfile::tempdir().unwrap();
    let (daemon, worktree, _) = daemon_in(tmp.path()).await.unwrap();
    let script = vec![
        json!({"expect": "initialize", "result": {"protocolVersion": 1, "agentCapabilities": {}, "authMethods": []}}),
        json!({"expect": "session/new", "result": {"sessionId": "mock-1"}}),
        json!({"expect": "session/prompt"}),
    ];
    let script_path = tmp.path().join("idle.json");
    std::fs::write(&script_path, json!(script).to_string()).unwrap();
    let provider = AcpProvider {
        id: "grok".into(),
        name: "Mock".into(),
        command: std::env::current_exe().unwrap().display().to_string(),
        args: mock_agent_args(&script_path),
        driver: berth_core::acp::ACP_DRIVER.into(),
        enabled: true,
    };
    daemon.cli_probes.lock().unwrap().insert(
        berth_core::AgentKind::Grok,
        (true, std::time::Instant::now()),
    );
    let agent = AgentId::generate();
    let host = Arc::new(DaemonHost {
        agent: agent.clone(),
    });
    daemon
        .create_acp_agent_with(
            agent.clone(),
            worktree,
            "mock".into(),
            provider,
            false,
            host,
        )
        .unwrap();
    let row = |daemon: &Daemon| daemon.store.get_agent(&agent).unwrap().unwrap();
    assert_eq!(row(&daemon).kind, berth_core::AgentKind::Grok);
    assert_eq!(row(&daemon).backend, Backend::Chat);
    assert!(daemon.acp.is_live(&agent));

    daemon
        .switch_backend(&agent, Backend::Terminal)
        .await
        .unwrap();
    assert_eq!(row(&daemon).backend, Backend::Terminal);
    assert_eq!(row(&daemon).acp_provider.as_deref(), Some("grok"));
    assert!(!daemon.acp.is_live(&agent));

    let mut events = daemon.events.subscribe();
    daemon.switch_backend(&agent, Backend::Chat).await.unwrap();
    assert_eq!(row(&daemon).backend, Backend::Chat);
    tokio::time::timeout(EVENT_WAIT, async {
        loop {
            if let ServerEvent::AcpItem { item, .. } = events.recv().await.unwrap() {
                if matches!(*item, AcpItem::Notice { .. }) {
                    return;
                }
            }
        }
    })
    .await
    .expect("no notice about the missing terminal turns");
}
