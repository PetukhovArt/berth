use crate::client::{handshake, runtime, try_connect, ONE_SHOT_REQ_ID};
use anyhow::{bail, Result};
use berth_core::codec::{read_frame, write_frame};
use berth_core::{AgentId, ClientRequest, ServerEvent};
use berth_hub::client::HubClient;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;

fn own_agent() -> Option<AgentId> {
    berth_core::env::non_empty(berth_core::env::AGENT_ID)
        .or_else(|| berth_core::env::non_empty(berth_core::legacy::AGENT_ID))
        .map(AgentId)
}

async fn call(request: ClientRequest) -> Result<ServerEvent> {
    let Ok(stream) = try_connect().await else {
        bail!("no berth daemon is running");
    };
    let mut conn = handshake(stream).await?;
    write_frame(&mut conn.stream, &request).await?;
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::Error {
                req_id: Some(ONE_SHOT_REQ_ID),
                message,
            }) => bail!("{message}"),
            Some(event @ (ServerEvent::Channels { .. } | ServerEvent::ChannelEvents { .. })) => {
                return Ok(event)
            }
            Some(_) => continue,
            None => bail!("the daemon closed the connection before it answered"),
        }
    }
}

pub fn run_list() -> Result<()> {
    runtime()?.block_on(async {
        let ServerEvent::Channels { channels, .. } = call(ClientRequest::ListChannels {
            req_id: ONE_SHOT_REQ_ID,
        })
        .await?
        else {
            bail!("unexpected reply");
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "channels": channels }))?
        );
        Ok(())
    })
}

pub fn run_read(channel: String, after: i64) -> Result<()> {
    runtime()?.block_on(async {
        let ServerEvent::ChannelEvents { events, .. } = call(ClientRequest::ReadChannel {
            req_id: ONE_SHOT_REQ_ID,
            channel,
            after,
            agent: own_agent(),
        })
        .await?
        else {
            bail!("unexpected reply");
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "events": events }))?
        );
        Ok(())
    })
}

pub fn run_post(channel: String, content: String) -> Result<()> {
    runtime()?.block_on(async {
        let ServerEvent::ChannelEvents { events, .. } = call(ClientRequest::PostToChannel {
            req_id: ONE_SHOT_REQ_ID,
            channel,
            content,
            agent: own_agent(),
        })
        .await?
        else {
            bail!("unexpected reply");
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "event": events.first() }))?
        );
        Ok(())
    })
}

pub fn run_hub_serve(
    bind: SocketAddr,
    db: Option<PathBuf>,
    admin_token: Option<String>,
) -> Result<()> {
    let db = db.unwrap_or_else(|| berth_core::paths::data_dir().join("hub.db"));
    runtime()?.block_on(async {
        let hub = berth_hub::serve(&db, bind, admin_token).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "url": format!("http://{}", hub.addr),
                "admin_token": hub.admin_token,
            }))?
        );
        hub.task.await?;
        Ok(())
    })
}

pub fn run_hub_pair(url: String, name: String) -> Result<()> {
    runtime()?.block_on(async {
        let (id, token) = HubClient::pair(&url, &name).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "id": id, "token": token }))?
        );
        Ok(())
    })
}

pub fn run_hub_approve(
    url: String,
    admin_token: String,
    id: String,
    scopes: Vec<String>,
) -> Result<()> {
    runtime()?.block_on(async {
        let hub = HubClient::new(&url, &admin_token)?;
        let scopes: Vec<&str> = scopes.iter().map(String::as_str).collect();
        hub.approve(&id, &scopes).await?;
        let pending: Value = hub.pending().await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "ok": true, "still_pending": pending }))?
        );
        Ok(())
    })
}
