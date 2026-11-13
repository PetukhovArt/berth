pub mod detect;

use crate::registry::Daemon;
use crate::status::HookEvent;
use anyhow::{bail, Context, Result};
use berth_core::protocol::SessionRef;
use berth_core::ProviderErrorKind;
use berth_core::{Agent, AgentId, AgentKind, AgentStatus, EntityId, ProviderError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedError {
    pub kind: ProviderErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    TurnEnd,
    Screen,
}

#[derive(Default)]
pub struct ProviderErrors(Mutex<HashMap<AgentId, (ProviderError, Source)>>);

impl ProviderErrors {
    pub fn get(&self, id: &AgentId) -> Option<ProviderError> {
        self.0.lock().unwrap().get(id).map(|(e, _)| e.clone())
    }

    fn set(&self, id: &AgentId, found: DetectedError, source: Source) -> bool {
        let mut map = self.0.lock().unwrap();
        let since_ms = match map.get(id) {
            Some((old, _)) if old.kind == found.kind => {
                if old.message == found.message {
                    return false;
                }
                old.since_ms
            }
            _ => now_ms(),
        };
        map.insert(
            id.clone(),
            (
                ProviderError {
                    kind: found.kind,
                    since_ms,
                    message: found.message,
                },
                source,
            ),
        );
        true
    }

    fn clear(&self, id: &AgentId, only: Option<Source>) -> bool {
        let mut map = self.0.lock().unwrap();
        match map.get(id) {
            Some((_, source)) if only.is_none_or(|o| o == *source) => {
                map.remove(id);
                true
            }
            _ => false,
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn kind_label(kind: ProviderErrorKind) -> &'static str {
    match kind {
        ProviderErrorKind::Overloaded => "provider overloaded",
        ProviderErrorKind::Server => "provider server error",
        ProviderErrorKind::Network => "network error",
        ProviderErrorKind::UsageLimit => "usage limit",
        ProviderErrorKind::Auth => "authentication error",
        ProviderErrorKind::Other => "API error",
    }
}

pub fn handoff_prompt(
    old: &Agent,
    transcript: Option<&std::path::Path>,
    error: Option<&ProviderError>,
) -> String {
    let reason = match error {
        Some(e) => format!(" It stopped on a {} ({}).", kind_label(e.kind), e.message),
        None => String::new(),
    };
    let conversation = match (transcript, old.session_id.as_deref()) {
        (Some(path), _) => format!(" Its conversation is at {}.", path.display()),
        (None, Some(sid)) => format!(
            " Its conversation is {} session {sid}.",
            old.kind.cli_program()
        ),
        (None, None) => String::new(),
    };
    format!(
        "[berth] Continue the task the {} agent \"{}\" was working on in this worktree.{reason}{conversation} \
Recover the task and its plan from the last turns of that conversation, run git status and git diff \
to see what is already done, then carry on from where it stopped.",
        old.kind.cli_program(),
        old.name,
    )
}

impl Daemon {
    fn set_provider_error(&self, id: &AgentId, found: DetectedError, source: Source) {
        if self.provider_errors.set(id, found, source) {
            let _ = self.broadcast_agent(id);
        }
    }

    fn clear_provider_error(&self, id: &AgentId, only: Option<Source>) {
        if self.provider_errors.clear(id, only) {
            let _ = self.broadcast_agent(id);
        }
    }

    fn screen_error(&self, agent: &Agent) -> Option<DetectedError> {
        let session = self.session(&SessionRef::Agent(agent.id.clone()))?;
        let (_, bytes) = session.snapshot(None);
        detect::from_screen_text(agent.kind, &detect::screen_text(&bytes))
    }

    pub fn route_hook_event(
        &self,
        id: &AgentId,
        event: &HookEvent,
        reported: Option<DetectedError>,
    ) {
        match event {
            HookEvent::UserPromptSubmit | HookEvent::SessionStart { .. } => {
                self.clear_provider_error(id, None)
            }
            HookEvent::Stop => {
                let Ok(Some(agent)) = self.store.get_agent(id) else {
                    return;
                };
                let found = reported
                    .or_else(|| {
                        (agent.kind == AgentKind::Claude)
                            .then(|| self.transcript_path(id))
                            .flatten()
                            .and_then(|p| detect::from_transcript(&p))
                    })
                    .or_else(|| self.screen_error(&agent));
                if let Some(found) = found {
                    tracing::info!(agent = %id, kind = ?found.kind, "provider error at turn end");
                    self.set_provider_error(id, found, Source::TurnEnd);
                }
            }
            _ => {}
        }
    }

    pub fn scan_provider_errors(&self) {
        let Ok((_, _, agents, _)) = self.store.load_tree() else {
            return;
        };
        for agent in agents.iter().filter(|a| !a.archived) {
            if agent.status != AgentStatus::Running {
                continue;
            }
            match self.screen_error(agent) {
                Some(found) => {
                    if self.provider_errors.get(&agent.id).is_none() {
                        tracing::info!(agent = %agent.id, kind = ?found.kind, "provider error on screen");
                    }
                    self.set_provider_error(&agent.id, found, Source::Screen)
                }
                None => self.clear_provider_error(&agent.id, Some(Source::Screen)),
            }
        }
    }

    pub async fn switch_agent_model(
        self: &Arc<Self>,
        id: &AgentId,
        model: Option<String>,
    ) -> Result<()> {
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if agent.session_id.is_none() {
            bail!("the agent has no conversation to resume yet");
        }
        let model = clean_model(model)?;
        self.store.set_agent_model(id, model.as_deref())?;
        self.provider_errors.clear(id, None);
        tracing::info!(agent = %id, model = ?model, "switch agent model");
        self.restart_agent(id).await
    }

    pub async fn continue_agent_in(
        self: &Arc<Self>,
        id: &AgentId,
        kind: AgentKind,
        model: Option<String>,
    ) -> Result<EntityId> {
        let old = self.store.get_agent(id)?.context("agent not found")?;
        let model = clean_model(model)?;
        let prompt = handoff_prompt(
            &old,
            self.transcript_path(id).as_deref(),
            self.provider_errors.get(id).as_ref(),
        );
        let mut spec = self.sibling_spec(id, Some(kind), &prompt)?;
        if model.is_some() {
            spec.model = model;
        }
        tracing::info!(agent = %id, to = ?kind, "continue agent in another kind");
        self.create_agent(spec).await
    }
}

fn clean_model(model: Option<String>) -> Result<Option<String>> {
    let model = model
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    if let Some(m) = &model {
        if m.chars().any(|c| c.is_whitespace() || c.is_control()) || m.starts_with('-') {
            bail!("invalid model name");
        }
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(kind: ProviderErrorKind, message: &str) -> DetectedError {
        DetectedError {
            kind,
            message: message.into(),
        }
    }

    #[test]
    fn screen_errors_clear_only_themselves() {
        let errors = ProviderErrors::default();
        let id = AgentId("a".into());
        assert!(errors.set(
            &id,
            found(ProviderErrorKind::Overloaded, "x"),
            Source::TurnEnd
        ));
        assert!(!errors.set(
            &id,
            found(ProviderErrorKind::Overloaded, "x"),
            Source::TurnEnd
        ));
        assert!(!errors.clear(&id, Some(Source::Screen)));
        assert!(errors.get(&id).is_some());
        assert!(errors.clear(&id, None));
        assert!(errors.get(&id).is_none());
    }

    #[test]
    fn model_names_cannot_smuggle_flags() {
        assert!(clean_model(Some("--yolo".into())).is_err());
        assert!(clean_model(Some("a b".into())).is_err());
        assert_eq!(clean_model(Some("  ".into())).unwrap(), None);
        assert_eq!(
            clean_model(Some("sonnet".into())).unwrap().as_deref(),
            Some("sonnet")
        );
    }
}
