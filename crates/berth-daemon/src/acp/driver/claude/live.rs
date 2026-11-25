use crate::acp::driver::open;
use crate::acp::providers::AcpProvider;
use crate::acp::wire::{DaemonHost, WireIn};
use berth_core::AgentId;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const DIR_VAR: &str = "LIVE_CLAUDE_DIR";
const TURN_LIMIT: Duration = Duration::from_secs(180);

#[tokio::test]
#[ignore = "drives the real claude CLI; set LIVE_CLAUDE_DIR to a scratch git repo"]
async fn a_permission_round_trip_reaches_the_real_claude() {
    let dir = std::path::PathBuf::from(std::env::var(DIR_VAR).expect(DIR_VAR));
    let provider = AcpProvider {
        id: "claude".into(),
        name: "Claude".into(),
        command: "claude".into(),
        args: vec!["--setting-sources".into(), "project".into()],
        driver: "claude".into(),
        enabled: true,
    };
    let host = Arc::new(DaemonHost {
        agent: AgentId::generate(),
    });
    let mut wire = open(&provider, &dir, host).unwrap();
    let send = |value: Value| wire.outbound.send(value.to_string()).unwrap();
    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1}}),
    );
    send(
        json!({"jsonrpc": "2.0", "id": 2, "method": "session/new", "params": {"cwd": dir, "mcpServers": []}}),
    );
    send(
        json!({"jsonrpc": "2.0", "id": 3, "method": "session/set_config_option", "params": {"configId": "model", "value": "haiku"}}),
    );
    send(
        json!({"jsonrpc": "2.0", "id": 4, "method": "session/prompt", "params": {"sessionId": "-", "prompt": [{
            "type": "text",
            "text": "Use the Write tool to create a file named live.txt containing the single word hi. Then reply done."
        }]}}),
    );
    let mut allowed = 0;
    let finished = tokio::time::timeout(TURN_LIMIT, async {
        while let Some(read) = wire.inbound.recv().await {
            let WireIn::Line { text, .. } = read else {
                break;
            };
            let message: Value = serde_json::from_str(&text).unwrap();
            let shown: String = text.chars().take(160).collect();
            println!("<< {shown}");
            match message["method"].as_str() {
                Some("session/request_permission") => {
                    allowed += 1;
                    let reply = json!({"jsonrpc": "2.0", "id": message["id"], "result": {"outcome": {"outcome": "selected", "optionId": "allow_once"}}});
                    println!(">> allow_once");
                    wire.outbound.send(reply.to_string()).unwrap();
                }
                None if message["id"] == 4 => return Some(message),
                _ => {}
            }
        }
        None
    })
    .await
    .unwrap();
    let reply = finished.expect("the turn ended without a prompt reply");
    println!("prompt reply: {}", reply["result"]);
    assert_eq!(reply["result"]["stopReason"], "end_turn");
    assert_eq!(allowed, 1);
    assert!(dir.join("live.txt").is_file());
}
