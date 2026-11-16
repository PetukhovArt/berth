use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use berth_core::{AgentId, SessionRef};
use serde_json::Value;

use super::route::{self, Body, Candidate, Inbound, Route};
use super::speech::{self, Scratch};
use super::state::{Bridge, Queued};
use super::telegram::Link;
use super::transcribers::{self, Failure};
use crate::config::Config;
use crate::registry::Daemon;
use crate::triggers::PROMPT_MAX_BYTES;

const POLL_SECS: u64 = 25;
const OFF_RECHECK: Duration = Duration::from_secs(15);
const ERROR_BACKOFF: Duration = Duration::from_secs(30);
const STALE_SECS: i64 = 15 * 60;
const MAX_VOICE_SECS: u64 = 180;
const VOICE_WAIT: Duration = Duration::from_secs(60 * 60);
const VOICE_RECHECK: Duration = Duration::from_secs(30);

const HELP: &str = "Send text or a voice note and it goes to your Agent as a prompt. \
Reply to one of my messages to pick its session; otherwise it goes to the session that most \
recently started waiting for input. Only messages from the owner id set in berth Settings > \
Chat bridge are taken.";
const NO_FFMPEG: &str = "Voice notes need ffmpeg on the PATH of the computer berth runs on. \
Nothing was sent; type the message instead.";
const APP_CLOSED: &str = "No berth desktop app with its speech engine is open, and it holds the \
speech model. This voice note waits until one opens (up to an hour); text messages work now.";
const NOT_SUPPORTED: &str = "I take text and voice notes only.";

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn prompt_text(text: &str) -> String {
    let mut clean: String = text
        .trim()
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    if clean.len() > PROMPT_MAX_BYTES {
        let mut end = PROMPT_MAX_BYTES;
        while !clean.is_char_boundary(end) {
            end -= 1;
        }
        clean.truncate(end);
    }
    clean
}

struct Inbox {
    link: Link,
    owner: Value,
}

fn inbox(config: &Config) -> Option<Inbox> {
    let owner = config.messenger_telegram_owner.clone()?;
    route::telegram_id(&owner)?;
    Some(Inbox {
        link: Link::from_config(config)?,
        owner,
    })
}

impl Daemon {
    async fn pause(&self, duration: Duration) -> bool {
        tokio::select! {
            _ = self.shutdown.cancelled() => true,
            _ = tokio::time::sleep(duration) => false,
        }
    }

    pub(crate) async fn run_chat_inbound(self: Arc<Self>, bridge: Arc<Bridge>) {
        let mut offset = 0i64;
        let mut token = String::new();
        loop {
            let Some(inbox) = inbox(&Config::load()) else {
                if self.pause(OFF_RECHECK).await {
                    break;
                }
                continue;
            };
            if inbox.link.bot.token() != token {
                token = inbox.link.bot.token().to_owned();
                offset = 0;
            }
            let polled = tokio::select! {
                _ = self.shutdown.cancelled() => break,
                polled = inbox.link.bot.updates(offset, POLL_SECS) => polled,
            };
            let updates = match polled {
                Ok(updates) => updates,
                Err(e) => {
                    tracing::warn!(error = %e, "chat bridge could not read Telegram");
                    if self.pause(ERROR_BACKOFF).await {
                        break;
                    }
                    continue;
                }
            };
            for update in updates {
                if let Some(id) = update["update_id"].as_i64() {
                    offset = offset.max(id + 1);
                }
                let Some(message) = Inbound::parse(&update, inbox.link.bot.id()) else {
                    continue;
                };
                if !route::from_owner(Some(&inbox.owner), Some(&inbox.link.chat), &message) {
                    tracing::info!(
                        from = ?message.from,
                        chat = message.chat_id,
                        "chat bridge ignored a Telegram message that is not from the owner"
                    );
                    continue;
                }
                self.take_message(&bridge, &inbox.link, message).await;
            }
        }
    }

    async fn take_message(self: &Arc<Self>, bridge: &Arc<Bridge>, link: &Link, message: Inbound) {
        let waited = now_secs() - message.date;
        if message.date > 0 && waited > STALE_SECS {
            let note = format!(
                "This message waited {} minutes while berth was offline, so nothing was sent. \
                 Send it again if it still applies.",
                waited / 60
            );
            reply(link, &message, &note).await;
            return;
        }
        match message.body.clone() {
            Body::Text(text) if matches!(text.trim(), "/start" | "/help") => {
                reply(link, &message, HELP).await;
            }
            Body::Text(text) => {
                self.route_prompt(bridge, link, &message, &text, None).await;
            }
            Body::Voice { seconds, .. } if seconds > MAX_VOICE_SECS => {
                let note = format!(
                    "Voice notes up to {} minutes are taken; this one was not sent.",
                    MAX_VOICE_SECS / 60
                );
                reply(link, &message, &note).await;
            }
            Body::Voice { file_id, .. } => {
                let daemon = self.clone();
                let bridge = bridge.clone();
                let link = link.clone();
                tokio::spawn(async move {
                    daemon.take_voice(&bridge, &link, &message, &file_id).await;
                });
            }
            Body::Other => reply(link, &message, NOT_SUPPORTED).await,
        }
    }

    async fn take_voice(
        self: &Arc<Self>,
        bridge: &Arc<Bridge>,
        link: &Link,
        message: &Inbound,
        file_id: &str,
    ) {
        let heard = match self.hear(link, message, file_id).await {
            Ok(Some(text)) => text,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(error = %e, "chat bridge could not read a voice note");
                reply(
                    link,
                    message,
                    &format!("I could not read that voice note: {e:#}"),
                )
                .await;
                return;
            }
        };
        if heard.trim().is_empty() {
            reply(
                link,
                message,
                "I heard no words in that voice note; nothing was sent.",
            )
            .await;
            return;
        }
        self.route_prompt(bridge, link, message, &heard, Some(&heard))
            .await;
    }

    async fn hear(&self, link: &Link, message: &Inbound, file_id: &str) -> Result<Option<String>> {
        let Some(ffmpeg) = speech::ffmpeg() else {
            reply(link, message, NO_FFMPEG).await;
            return Ok(None);
        };
        let scratch = Scratch::new(&message.message_id.to_string())?;
        let note = scratch.path("note.oga");
        let pcm = scratch.path("note.pcm");
        link.bot.download(file_id, &note).await?;
        speech::decode(&ffmpeg, &note, &pcm).await?;
        let deadline = tokio::time::Instant::now() + VOICE_WAIT;
        let mut told = false;
        loop {
            match transcribers::transcribe(&pcm).await {
                Ok(text) => return Ok(Some(text)),
                Err(Failure::Failed(error)) => anyhow::bail!("{error}"),
                Err(Failure::NoApp) => {}
            }
            if !told {
                reply(link, message, APP_CLOSED).await;
                told = true;
            }
            if tokio::time::Instant::now() >= deadline || self.shutdown.is_cancelled() {
                reply(
                    link,
                    message,
                    "No berth desktop app opened within an hour, so this voice note was dropped.",
                )
                .await;
                return Ok(None);
            }
            transcribers::offered(VOICE_RECHECK).await;
        }
    }

    fn candidates(&self) -> Vec<Candidate> {
        let Ok((_, _, agents, _)) = self.store.load_tree() else {
            return Vec::new();
        };
        let feedback = self.feedback.all();
        agents
            .into_iter()
            .map(|agent| Candidate {
                options: feedback
                    .iter()
                    .find(|(id, _)| *id == agent.id)
                    .map(|(_, request)| request.options.len()),
                id: agent.id,
                status: agent.status,
                archived: agent.archived,
                changed_at: agent.status_changed_at,
            })
            .collect()
    }

    pub(crate) fn agent_label(&self, agent: &AgentId) -> String {
        let Some(row) = self.store.get_agent(agent).ok().flatten() else {
            return agent.to_string();
        };
        match self.project_name(&row.worktree_id) {
            Some(project) => format!("{} ({project})", row.name),
            None => row.name,
        }
    }

    pub(crate) async fn deliver_prompt(
        self: &Arc<Self>,
        agent: &AgentId,
        text: &str,
    ) -> Result<()> {
        if self.is_acp_agent(agent) {
            self.acp_prompt(agent, text).await
        } else {
            self.type_into_agent(agent, text).await
        }
    }

    async fn answer_option(self: &Arc<Self>, agent: &AgentId, option: u8) -> Result<()> {
        if self.is_acp_agent(agent) {
            return self.answer_feedback(agent, option).await;
        }
        let session = self
            .session(&SessionRef::Agent(agent.clone()))
            .context("the session is not running")?;
        session.write_input((u16::from(option) + 1).to_string().as_bytes())
    }

    async fn route_prompt(
        self: &Arc<Self>,
        bridge: &Arc<Bridge>,
        link: &Link,
        message: &Inbound,
        text: &str,
        heard: Option<&str>,
    ) {
        let text = prompt_text(text);
        if text.is_empty() {
            return;
        }
        let voice = heard.is_some();
        let heard = heard
            .map(|heard| format!("Heard: \"{}\"\n", heard.trim()))
            .unwrap_or_default();
        let candidates = self.candidates();
        let decision = route::route(
            &text,
            message.reply_to_bot,
            |sent| bridge.agent_of(sent),
            &candidates,
        );
        let (answer, about) = match decision {
            Route::Deliver {
                agent,
                queued: true,
            } => {
                let place = bridge.queue(&agent, Queued { text, voice });
                let note = format!(
                    "{heard}Queued for {}: it is busy, and this goes in when its turn ends \
                     ({place} waiting).",
                    self.agent_label(&agent)
                );
                (note, Some(agent))
            }
            Route::Deliver { agent, .. } => {
                let label = self.agent_label(&agent);
                let note = match self.deliver_prompt(&agent, &text).await {
                    Ok(()) => {
                        bridge.expect_answer(&agent, voice);
                        format!("{heard}Sent to {label}.")
                    }
                    Err(e) => format!("{heard}Could not send to {label}: {e:#}"),
                };
                (note, Some(agent))
            }
            Route::Answer { agent, option } => {
                let label = self.agent_label(&agent);
                let note = match self.answer_option(&agent, option).await {
                    Ok(()) => {
                        bridge.expect_answer(&agent, voice);
                        format!("{heard}Answered option {} for {label}.", option + 1)
                    }
                    Err(e) => format!("{heard}Could not answer {label}: {e:#}"),
                };
                (note, Some(agent))
            }
            Route::NeedsAnswer { agent } => {
                let note = format!(
                    "{heard}{}\nNothing was sent: reply with an option number.",
                    self.feedback_note(&agent)
                );
                (note, Some(agent))
            }
            Route::Gone => (
                format!("{heard}That session is archived or gone; nothing was sent."),
                None,
            ),
            Route::Forgotten => (
                format!(
                    "{heard}I no longer know which session that message was about (berth \
                     restarted since). Nothing was sent: send it again without replying, or \
                     reply to a newer message."
                ),
                None,
            ),
            Route::NoTarget => (
                format!(
                    "{heard}No session waits for input, so nothing was sent. Reply to one of \
                     my messages to queue it for a busy session."
                ),
                None,
            ),
        };
        if let Some(sent) = reply(link, message, &answer).await {
            if let Some(agent) = about {
                bridge.remember(sent, &agent);
            }
        }
    }

    pub(crate) fn feedback_note(&self, agent: &AgentId) -> String {
        let label = self.agent_label(agent);
        let Some((_, request)) = self.feedback.all().into_iter().find(|(id, _)| id == agent) else {
            return format!("{label} needs feedback.");
        };
        let mut note = format!("{label} needs feedback: {}", request.title);
        if let Some(detail) = request.detail.as_deref().filter(|d| !d.trim().is_empty()) {
            let detail: String = detail.chars().take(200).collect();
            note.push_str(&format!("\n{detail}"));
        }
        for (index, option) in request.options.iter().enumerate() {
            note.push_str(&format!("\n{}. {option}", index + 1));
        }
        note
    }

    pub(crate) async fn deliver_queued(self: &Arc<Self>, bridge: &Arc<Bridge>, agent: &AgentId) {
        let Some(next) = bridge.next_queued(agent) else {
            return;
        };
        match self.deliver_prompt(agent, &next.text).await {
            Ok(()) => bridge.expect_answer(agent, next.voice),
            Err(e) => {
                tracing::warn!(agent = %agent, error = %e, "chat bridge could not deliver a queued prompt");
                if let Some(link) = Link::from_config(&Config::load()) {
                    let note = format!(
                        "Could not send the queued message to {}: {e:#}",
                        self.agent_label(agent)
                    );
                    let _ = link.send_text(&note, None).await;
                }
            }
        }
    }
}

async fn reply(link: &Link, message: &Inbound, text: &str) -> Option<i64> {
    match link.send_text(text, Some(message.message_id)).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!(error = %e, "chat bridge could not answer on Telegram");
            None
        }
    }
}
