use crate::client::{handshake, runtime, try_connect, ONE_SHOT_REQ_ID};
use anyhow::{bail, Context, Result};
use berth_core::codec::{read_frame, write_frame};
use berth_core::{
    AgentId, AgentKind, ClientRequest, NewTrigger, ServerEvent, TriggerId, TriggerTarget,
    TriggerWhen, WorktreeId,
};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct TriggerTargetArgs {
    pub agent: Option<String>,
    pub worktree: Option<String>,
    pub kind: Option<AgentKind>,
}

pub fn parse_span(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let split = s.len().saturating_sub(1);
    let (digits, unit) = s.split_at(split);
    let n: u64 = digits
        .parse()
        .map_err(|_| format!("`{s}` is not a span like 30m, 2h or 1d"))?;
    let unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return Err(format!("`{s}` is not a span like 30m, 2h or 1d")),
    };
    Ok(Duration::from_secs(n * unit))
}

fn target(args: TriggerTargetArgs) -> Result<TriggerTarget> {
    match (args.agent, args.worktree) {
        (Some(_), Some(_)) => bail!("name either --agent or --worktree, not both"),
        (Some(agent), None) => Ok(TriggerTarget::Agent(AgentId(agent))),
        (None, Some(worktree)) => Ok(TriggerTarget::NewAgent {
            worktree: WorktreeId(worktree),
            kind: args.kind.unwrap_or_default(),
        }),
        (None, None) => {
            let agent = berth_core::env::non_empty(berth_core::env::AGENT_ID)
                .context("name the target with --agent <id> or --worktree <id>")?;
            Ok(TriggerTarget::Agent(AgentId(agent)))
        }
    }
}

fn name_of(prompt: &str, name: Option<String>) -> String {
    name.unwrap_or_else(|| prompt.chars().take(40).collect())
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
            Some(
                event @ (ServerEvent::Triggers { .. }
                | ServerEvent::TriggerCreated { .. }
                | ServerEvent::Ack { .. }),
            ) => return Ok(event),
            Some(_) => continue,
            None => bail!("the daemon closed the connection before it answered"),
        }
    }
}

async fn create(trigger: NewTrigger) -> Result<()> {
    let event = call(ClientRequest::CreateTrigger {
        req_id: ONE_SHOT_REQ_ID,
        trigger,
    })
    .await?;
    let ServerEvent::TriggerCreated {
        trigger, secret, ..
    } = event
    else {
        bail!("unexpected reply");
    };
    let mut out = json!({ "id": trigger.id.0, "name": trigger.name });
    if let Some(secret) = secret {
        out["secret"] = Value::String(secret);
        out["path"] = Value::String(format!("/hooks/{}", trigger.id.0));
        out["note"] = Value::String(
            "POST to that path with the header `X-Berth-Secret: <secret>` (optionally \
             `X-Berth-Delivery: <id>` to drop repeats). The secret is shown once. The listener is \
             off until `\"webhooks\": true` is set in the config file."
                .into(),
        );
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

pub fn run_schedule(
    prompt: String,
    name: Option<String>,
    delay: Duration,
    every: Option<Duration>,
    args: TriggerTargetArgs,
) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
    let trigger = NewTrigger {
        name: name_of(&prompt, name),
        when: TriggerWhen::At {
            run_at: now + delay.as_millis() as i64,
            repeat_secs: every.map(|d| d.as_secs()),
        },
        target: target(args)?,
        prompt,
    };
    runtime()?.block_on(create(trigger))
}

pub fn run_webhook(prompt: String, name: Option<String>, args: TriggerTargetArgs) -> Result<()> {
    let trigger = NewTrigger {
        name: name_of(&prompt, name),
        when: TriggerWhen::Webhook,
        target: target(args)?,
        prompt,
    };
    runtime()?.block_on(create(trigger))
}

pub fn run_triggers(remove: Option<String>) -> Result<()> {
    runtime()?.block_on(async {
        if let Some(id) = remove {
            call(ClientRequest::DeleteTrigger {
                req_id: ONE_SHOT_REQ_ID,
                id: TriggerId(id),
            })
            .await?;
            return Ok(());
        }
        let ServerEvent::Triggers { triggers, .. } = call(ClientRequest::ListTriggers {
            req_id: ONE_SHOT_REQ_ID,
        })
        .await?
        else {
            bail!("unexpected reply");
        };
        let rows: Vec<Value> = triggers
            .iter()
            .map(|t| {
                json!({
                    "id": t.id.0,
                    "name": t.name,
                    "when": serde_json::to_value(&t.when).unwrap_or(Value::Null),
                    "target": serde_json::to_value(&t.target).unwrap_or(Value::Null),
                    "last_fired_at": t.last_fired_at,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_parse() {
        assert_eq!(parse_span("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_span("1d").unwrap(), Duration::from_secs(86_400));
        assert!(parse_span("soon").is_err());
        assert!(parse_span("").is_err());
    }
}
