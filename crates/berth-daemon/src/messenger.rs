use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use berth_core::platform::NoWindow;
use berth_core::{AgentId, AgentStatus, ServerEvent, WorktreeId};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::broadcast::error::RecvError;

use crate::config::Config;
use crate::registry::Daemon;

const SEND_TIMEOUT_SECS: &str = "15";
const NOTE_MAX_CHARS: usize = 300;
const TEST_TEXT: &str =
    "Berth chat bridge test: this chat gets a message when an Agent needs feedback or finishes.";

struct Post {
    route: &'static str,
    url: String,
    body: Value,
}

fn posts(config: &Config, text: &str) -> Vec<Post> {
    let mut out = Vec::new();
    if let (Some(token), Some(chat)) = (
        &config.messenger_telegram_token,
        &config.messenger_telegram_chat,
    ) {
        out.push(Post {
            route: "Telegram",
            url: format!("https://api.telegram.org/bot{token}/sendMessage"),
            body: json!({ "chat_id": chat, "text": text }),
        });
    }
    if let Some(url) = &config.messenger_webhook_url {
        out.push(Post {
            route: "Webhook",
            url: url.clone(),
            body: json!({ "text": text }),
        });
    }
    out
}

fn quote(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

async fn send(post: &Post) -> Result<()> {
    let mut child = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", SEND_TIMEOUT_SECS, "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_window()
        .spawn()
        .context("run curl")?;
    let config = format!(
        "url = \"{}\"\nheader = \"Content-Type: application/json\"\ndata = \"{}\"\n",
        quote(&post.url),
        quote(&post.body.to_string())
    );
    let mut stdin = child.stdin.take().context("curl stdin")?;
    stdin.write_all(config.as_bytes()).await?;
    drop(stdin);
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub async fn send_test() -> Result<()> {
    let sends: Vec<_> = posts(&Config::load(), TEST_TEXT)
        .into_iter()
        .map(|post| {
            tokio::spawn(async move {
                send(&post)
                    .await
                    .map_err(|error| format!("{}: {error}", post.route))
            })
        })
        .collect();
    if sends.is_empty() {
        bail!("no chat route is set: fill in the Telegram token and chat, or a webhook URL");
    }
    let mut failures = Vec::new();
    for send in sends {
        match send.await {
            Ok(Ok(())) => {}
            Ok(Err(message)) => failures.push(message),
            Err(error) => failures.push(error.to_string()),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        bail!("{}", failures.join("; "))
    }
}

fn turned_to(
    previous: Option<AgentStatus>,
    status: AgentStatus,
    unseen: bool,
) -> Option<&'static str> {
    match status {
        AgentStatus::NeedsFeedback if previous != Some(status) => Some("Needs feedback"),
        AgentStatus::Finished if previous == Some(AgentStatus::Running) && unseen => {
            Some("Finished")
        }
        _ => None,
    }
}

impl Daemon {
    fn agent_line(&self, agent: &AgentId, label: &str) -> Option<String> {
        let agent = self.store.get_agent(agent).ok()??;
        Some(format!(
            "{label}: {} ({})",
            agent.name,
            self.project_name(&agent.worktree_id)?
        ))
    }

    fn note_line(&self, worktree: &WorktreeId, note: &str) -> Option<String> {
        let branch = self.store.get_worktree(worktree).ok()??.branch;
        let note: String = note.chars().take(NOTE_MAX_CHARS).collect();
        Some(format!(
            "Task note on {} ({branch}): {note}",
            self.project_name(worktree)?
        ))
    }

    fn project_name(&self, worktree: &WorktreeId) -> Option<String> {
        let worktree = self.store.get_worktree(worktree).ok()??;
        Some(self.store.get_project(&worktree.project_id).ok()??.name)
    }

    pub async fn run_messenger_bridge(self: Arc<Self>) {
        let mut events = self.events.subscribe();
        let mut last: HashMap<AgentId, AgentStatus> = HashMap::new();
        loop {
            let event = tokio::select! {
                _ = self.shutdown.cancelled() => break,
                event = events.recv() => event,
            };
            let text = match event {
                Ok(ServerEvent::StatusChanged {
                    agent,
                    status,
                    unseen,
                    ..
                }) => turned_to(last.insert(agent.clone(), status), status, unseen)
                    .and_then(|label| self.agent_line(&agent, label)),
                Ok(ServerEvent::TaskNoteChanged { worktree, text }) if !text.is_empty() => {
                    self.note_line(&worktree, &text)
                }
                Ok(_) | Err(RecvError::Lagged(_)) => None,
                Err(RecvError::Closed) => break,
            };
            let Some(text) = text else { continue };
            for post in posts(&Config::load(), &text) {
                tokio::spawn(async move {
                    if let Err(e) = send(&post).await {
                        tracing::warn!(error = %e, "messenger post failed");
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_to_waiting_or_finished_is_worth_a_message_and_a_new_prompt_is_not() {
        use AgentStatus::*;
        assert_eq!(
            turned_to(Some(Running), NeedsFeedback, false),
            Some("Needs feedback")
        );
        assert_eq!(turned_to(Some(NeedsFeedback), NeedsFeedback, false), None);
        assert_eq!(turned_to(Some(Running), Finished, true), Some("Finished"));
        assert_eq!(turned_to(None, Finished, true), None);
        assert_eq!(turned_to(Some(Running), Finished, false), None);
        assert_eq!(turned_to(Some(Finished), Running, false), None);
    }

    #[test]
    fn telegram_and_the_webhook_each_get_their_own_request_and_unset_keys_send_nothing() {
        let mut config = Config::default();
        assert!(posts(&config, "hi").is_empty());
        config.messenger_telegram_token = Some("123:abc".into());
        assert!(
            posts(&config, "hi").is_empty(),
            "a token without a chat is off"
        );
        config.messenger_telegram_chat = Some(json!(-42));
        config.messenger_webhook_url = Some("https://chat.example/hook".into());
        let sent = posts(&config, "hi");
        assert_eq!(
            sent[0].url,
            "https://api.telegram.org/bot123:abc/sendMessage"
        );
        assert_eq!(sent[0].body, json!({ "chat_id": -42, "text": "hi" }));
        assert_eq!(sent[1].url, "https://chat.example/hook");
        assert_eq!(sent[1].body, json!({ "text": "hi" }));
        assert_eq!(quote(r#"a"b\c"#), r#"a\"b\\c"#);
    }
}
