mod answer;
mod inbound;
pub(crate) mod route;
mod speech;
mod state;
mod telegram;
pub(crate) mod transcribers;

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use berth_core::platform::NoWindow;
use berth_core::{AgentId, AgentStatus, ServerEvent, WorktreeId};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::broadcast::error::RecvError;

use crate::config::Config;
use crate::registry::Daemon;
use speech::Scratch;
use state::{Awaiting, Bridge};
use telegram::Link;

const SEND_TIMEOUT_SECS: u64 = 15;
const NOTE_MAX_CHARS: usize = 300;
const ANSWER_SETTLE: Duration = Duration::from_millis(1500);
const TEST_TEXT: &str =
    "Berth chat bridge test: this chat gets a message when an Agent needs feedback or finishes.";

struct Post {
    route: &'static str,
    url: String,
    body: Value,
}

const TELEGRAM: &str = "Telegram";

fn posts(config: &Config, text: &str) -> Vec<Post> {
    let mut out = Vec::new();
    if let (Some(token), Some(chat)) = (
        &config.messenger_telegram_token,
        &config.messenger_telegram_chat,
    ) {
        out.push(Post {
            route: TELEGRAM,
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

pub(crate) fn curl_line(key: &str, value: &str) -> String {
    format!("{key} = \"{}\"\n", quote(value))
}

pub(crate) async fn curl(config: &str, max_secs: u64) -> Result<Vec<u8>> {
    let max_time = max_secs.to_string();
    let mut child = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", max_time.as_str(), "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_window()
        .spawn()
        .context("run curl")?;
    let mut stdin = child.stdin.take().context("curl stdin")?;
    stdin.write_all(config.as_bytes()).await?;
    drop(stdin);
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(output.stdout)
}

async fn send(post: &Post) -> Result<Vec<u8>> {
    let config = curl_line("url", &post.url)
        + &curl_line("header", "Content-Type: application/json")
        + &curl_line("data", &post.body.to_string());
    curl(&config, SEND_TIMEOUT_SECS).await
}

pub async fn send_test() -> Result<()> {
    let sends: Vec<_> = posts(&Config::load(), TEST_TEXT)
        .into_iter()
        .map(|post| {
            tokio::spawn(async move {
                send(&post)
                    .await
                    .map(drop)
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

fn sent_message_id(body: &[u8]) -> Option<i64> {
    serde_json::from_slice::<Value>(body).ok()?["result"]["message_id"].as_i64()
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

    async fn send_answer(
        self: Arc<Self>,
        bridge: Arc<Bridge>,
        agent: AgentId,
        status: AgentStatus,
        awaiting: Awaiting,
    ) {
        tokio::time::sleep(ANSWER_SETTLE).await;
        let Some(link) = Link::from_config(&Config::load()) else {
            return;
        };
        let last = match status {
            AgentStatus::Finished => self
                .store
                .get_agent(&agent)
                .ok()
                .flatten()
                .and_then(|row| self.last_answer(&row)),
            _ => None,
        };
        let label = self.agent_label(&agent);
        let text = match (status, &last) {
            (AgentStatus::NeedsFeedback, _) => format!(
                "{}\nReply to this message to answer.",
                self.feedback_note(&agent)
            ),
            (_, Some(last)) => format!("{label} finished:\n{}", answer::summary(last)),
            (_, None) => format!("{label} finished. Open berth to read the answer."),
        };
        match link.send_text(&text, None).await {
            Ok(id) => bridge.remember(id, &agent),
            Err(e) => tracing::warn!(error = %e, "chat bridge could not send an answer"),
        }
        if let (true, Some(last)) = (awaiting.voice, &last) {
            self.send_spoken(&link, &bridge, &agent, last).await;
        }
    }

    async fn send_spoken(&self, link: &Link, bridge: &Bridge, agent: &AgentId, last: &str) {
        let spoken = answer::spoken(last);
        if spoken.is_empty() {
            return;
        }
        let scratch = match Scratch::new(&format!("reply-{agent}")) {
            Ok(scratch) => scratch,
            Err(e) => {
                tracing::warn!(error = %e, "chat bridge could not make a scratch dir");
                return;
            }
        };
        let Some(upload) = speech::speak(&spoken, &scratch).await else {
            return;
        };
        match link.send_upload(&upload, None).await {
            Ok(id) => bridge.remember(id, agent),
            Err(e) => tracing::warn!(error = %e, "chat bridge could not send a voice answer"),
        }
    }

    pub async fn run_messenger_bridge(self: Arc<Self>) {
        let bridge = Arc::new(Bridge::default());
        tokio::spawn(self.clone().run_chat_inbound(bridge.clone()));
        let mut events = self.events.subscribe();
        let mut last: HashMap<AgentId, AgentStatus> = HashMap::new();
        loop {
            let event = tokio::select! {
                _ = self.shutdown.cancelled() => break,
                event = events.recv() => event,
            };
            let (text, about, answered) = match event {
                Ok(ServerEvent::StatusChanged {
                    agent,
                    status,
                    unseen,
                    ..
                }) => {
                    let previous = last.insert(agent.clone(), status);
                    let answered = self.follow_up(&bridge, &agent, previous, status);
                    let text = turned_to(previous, status, unseen)
                        .and_then(|label| self.agent_line(&agent, label));
                    (text, Some(agent), answered)
                }
                Ok(ServerEvent::TaskNoteChanged { worktree, text }) if !text.is_empty() => {
                    (self.note_line(&worktree, &text), None, false)
                }
                Ok(_) | Err(RecvError::Lagged(_)) => (None, None, false),
                Err(RecvError::Closed) => break,
            };
            let Some(text) = text else { continue };
            for post in posts(&Config::load(), &text) {
                let telegram = post.route == TELEGRAM;
                if telegram && answered {
                    continue;
                }
                let bridge = bridge.clone();
                let about = about.clone();
                tokio::spawn(async move {
                    match send(&post).await {
                        Ok(body) => {
                            if let (true, Some(agent), Some(id)) =
                                (telegram, about, sent_message_id(&body))
                            {
                                bridge.remember(id, &agent);
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "messenger post failed"),
                    }
                });
            }
        }
    }

    fn follow_up(
        self: &Arc<Self>,
        bridge: &Arc<Bridge>,
        agent: &AgentId,
        previous: Option<AgentStatus>,
        status: AgentStatus,
    ) -> bool {
        if previous == Some(status)
            || !matches!(status, AgentStatus::Finished | AgentStatus::NeedsFeedback)
        {
            return false;
        }
        let awaiting = bridge.awaiting(agent);
        if status == AgentStatus::Finished {
            bridge.answered(agent);
        }
        let daemon = self.clone();
        let bridge = bridge.clone();
        let agent = agent.clone();
        tokio::spawn(async move {
            if let Some(awaiting) = awaiting {
                daemon
                    .clone()
                    .send_answer(bridge.clone(), agent.clone(), status, awaiting)
                    .await;
            }
            if status == AgentStatus::Finished {
                daemon.deliver_queued(&bridge, &agent).await;
            }
        });
        awaiting.is_some()
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
