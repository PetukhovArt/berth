pub mod auto;
pub mod config;
pub mod detect;
pub mod handoff;
pub mod rules;

use crate::registry::Daemon;
use crate::status::HookEvent;
use anyhow::{bail, Context, Result};
use berth_core::protocol::SessionRef;
use berth_core::ProviderErrorKind;
use berth_core::{
    Agent, AgentId, AgentKind, AgentStatus, EntityId, ProviderError, RouteAction, RouteSuggestion,
    UsageWindow,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedError {
    pub kind: ProviderErrorKind,
    pub message: String,
    pub resets_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    TurnEnd,
    Screen,
}

type LoadedRules = (Option<SystemTime>, Arc<rules::RouterFile>);

#[derive(Default)]
pub struct ProviderErrors {
    errors: Mutex<HashMap<AgentId, (ProviderError, Source)>>,
    suggestions: Mutex<HashMap<AgentId, RouteSuggestion>>,
    rules: Mutex<Option<LoadedRules>>,
    auto: auto::AutoRoutes,
}

impl ProviderErrors {
    pub fn get(&self, id: &AgentId) -> Option<ProviderError> {
        self.errors.lock().unwrap().get(id).map(|(e, _)| e.clone())
    }

    fn suggestion(&self, id: &AgentId) -> Option<RouteSuggestion> {
        self.suggestions.lock().unwrap().get(id).cloned()
    }

    fn set_suggestion(&self, id: &AgentId, suggestion: Option<RouteSuggestion>) -> bool {
        let mut map = self.suggestions.lock().unwrap();
        if map.get(id) == suggestion.as_ref() {
            return false;
        }
        match suggestion {
            Some(s) => map.insert(id.clone(), s),
            None => map.remove(id),
        };
        true
    }

    fn set(
        &self,
        id: &AgentId,
        found: DetectedError,
        source: Source,
        window_reset: Option<i64>,
    ) -> bool {
        let mut map = self.errors.lock().unwrap();
        let since_ms = match map.get(id) {
            Some((old, _)) if old.kind == found.kind => {
                if old.message == found.message {
                    return false;
                }
                old.since_ms
            }
            _ => now_ms(),
        };
        let resets_at_ms = found.resets_at_ms.or(match found.kind {
            ProviderErrorKind::UsageLimit => window_reset,
            _ => None,
        });
        map.insert(
            id.clone(),
            (
                ProviderError {
                    kind: found.kind,
                    since_ms,
                    message: found.message,
                    resets_at_ms,
                },
                source,
            ),
        );
        true
    }

    fn clear(&self, id: &AgentId, only: Option<Source>) -> bool {
        let mut map = self.errors.lock().unwrap();
        match map.get(id) {
            Some((_, source)) if only.is_none_or(|o| o == *source) => {
                map.remove(id);
                true
            }
            _ => false,
        }
    }

    fn router_file(&self) -> Arc<rules::RouterFile> {
        let path = router_path();
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let mut cache = self.rules.lock().unwrap();
        if let Some((at, file)) = cache.as_ref() {
            if *at == mtime {
                return file.clone();
            }
        }
        let file = Arc::new(load_router_file(&path));
        if file.is_auto() {
            tracing::info!("router mode is auto");
        }
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        *cache = Some((mtime, file.clone()));
        file
    }
}

pub fn router_path() -> PathBuf {
    berth_core::paths::config_path().with_file_name("router.toml")
}

fn load_router_file(path: &std::path::Path) -> rules::RouterFile {
    match std::fs::read_to_string(path) {
        Ok(text) => rules::parse(&text).unwrap_or_else(|err| {
            tracing::warn!("ignoring {}: {err}", path.display());
            rules::RouterFile::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let file = rules::RouterFile::default();
            if let Err(err) = rules::render(&file)
                .map_err(anyhow::Error::msg)
                .and_then(|text| Ok(std::fs::write(path, text)?))
            {
                tracing::warn!("could not write {}: {err}", path.display());
            }
            file
        }
        Err(err) => {
            tracing::warn!("could not read {}: {err}", path.display());
            rules::RouterFile::default()
        }
    }
}

pub(crate) fn now_ms() -> i64 {
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
        ProviderErrorKind::Model => "model not available",
        ProviderErrorKind::Other => "API error",
    }
}

fn window_reset(windows: &[UsageWindow], kind: AgentKind) -> Option<i64> {
    windows
        .iter()
        .filter(|w| w.kind == kind && w.used_percent >= rules::FULL_PERCENT)
        .map(|w| w.resets_at_ms)
        .max()
}

fn may_route(status: AgentStatus, error: Option<&ProviderError>) -> bool {
    error.is_some() || matches!(status, AgentStatus::Finished | AgentStatus::NeedsFeedback)
}

fn stuck_long_enough(status: AgentStatus, error: Option<&ProviderError>, now_ms: i64) -> bool {
    status != AgentStatus::Running || error.is_some_and(|e| now_ms - e.since_ms >= auto::STUCK_MS)
}

impl Daemon {
    pub(crate) fn decorate_route(&self, agent: &mut Agent) {
        agent.provider_error = self.provider_errors.get(&agent.id);
        agent.route_suggestion = self.provider_errors.suggestion(&agent.id);
        agent.routed_from = self.store.routed_from(&agent.id).ok().flatten();
    }

    fn set_provider_error(&self, agent: &Agent, found: DetectedError, source: Source) {
        let reset = window_reset(&crate::usage_limits::cached(), agent.kind);
        if self.provider_errors.set(&agent.id, found, source, reset) {
            self.refresh_route_suggestion(agent);
            let _ = self.broadcast_agent(&agent.id);
        }
    }

    fn clear_provider_error(&self, id: &AgentId, only: Option<Source>) {
        if self.provider_errors.clear(id, only) {
            if let Ok(Some(agent)) = self.store.get_agent(id) {
                self.refresh_route_suggestion(&agent);
            }
            let _ = self.broadcast_agent(id);
        }
    }

    fn screen_error(&self, agent: &Agent) -> Option<DetectedError> {
        let session = self.session(&SessionRef::Agent(agent.id.clone()))?;
        let (_, bytes) = session.snapshot(None);
        detect::from_screen_text(agent.kind, &detect::screen_text(&bytes))
    }

    fn route_suggestion_for(&self, agent: &Agent) -> Option<RouteSuggestion> {
        if agent.archived || self.store.was_routed_on(&agent.id).unwrap_or(false) {
            return None;
        }
        let error = self.provider_errors.get(&agent.id);
        if !may_route(agent.status, error.as_ref()) {
            return None;
        }
        let file = self.provider_errors.router_file();
        let windows = crate::usage_limits::cached();
        let now = now_ms();
        rules::suggest(
            &file,
            &rules::Facts {
                kind: agent.kind,
                model: agent.model.as_deref(),
                status: agent.status,
                error: error.as_ref(),
                windows: &windows,
                now_ms: now,
            },
        )
        .or_else(|| {
            let origin_free = |kind| {
                windows
                    .iter()
                    .all(|w| w.kind != kind || w.used_percent < rules::FULL_PERCENT)
            };
            error
                .is_none()
                .then(|| {
                    self.provider_errors.auto.due_return(
                        &agent.id,
                        agent.kind,
                        agent.model.as_deref(),
                        now,
                    )
                })
                .flatten()
                .filter(|s| match &s.action {
                    RouteAction::ContinueIn { kind, .. } => origin_free(*kind),
                    RouteAction::SwitchModel { .. } => origin_free(agent.kind),
                })
        })
    }

    fn route_depth(&self, id: &AgentId) -> u32 {
        let mut depth = 0;
        let mut at = id.clone();
        while let Ok(Some(from)) = self.store.routed_from(&at) {
            depth += 1;
            if depth >= 64 {
                break;
            }
            at = from;
        }
        depth
    }

    fn auto_route(self: &Arc<Self>, agent: &Agent) {
        let file = self.provider_errors.router_file();
        if !file.is_auto() || agent.archived || self.store.was_routed_on(&agent.id).unwrap_or(false)
        {
            return;
        }
        let error = self.provider_errors.get(&agent.id);
        let now = now_ms();
        if !may_route(agent.status, error.as_ref())
            || !stuck_long_enough(agent.status, error.as_ref(), now)
        {
            return;
        }
        let windows = crate::usage_limits::cached();
        let Some(picked) = rules::pick(
            &file,
            &rules::Facts {
                kind: agent.kind,
                model: agent.model.as_deref(),
                status: agent.status,
                error: error.as_ref(),
                windows: &windows,
                now_ms: now,
            },
        ) else {
            return;
        };
        let attempt = auto::Attempt {
            agent: &agent.id,
            rule_id: &picked.rule.id,
            cooldown_ms: picked.cooldown_ms(),
            hops_before: self.route_depth(&agent.id),
            hop_cap: file.hop_cap(),
            now_ms: now,
        };
        if let Err(why) = self.provider_errors.auto.begin(&attempt) {
            tracing::debug!(agent = %agent.id, rule = %picked.rule.id, ?why, "auto route held back");
            return;
        }
        let action = picked.suggestion.action.clone();
        tracing::info!(agent = %agent.id, rule = %picked.rule.id, ?action, "auto route");
        let daemon = self.clone();
        let id = agent.id.clone();
        tokio::spawn(async move {
            let result = match action {
                RouteAction::SwitchModel { model } => {
                    daemon.switch_agent_model(&id, model).await.map(|_| None)
                }
                RouteAction::ContinueIn { kind, model } => daemon
                    .continue_agent_in(&id, kind, model)
                    .await
                    .map(|created| match created {
                        EntityId::Agent(new) => Some(new),
                        _ => None,
                    }),
            };
            match result {
                Ok(new) => daemon.provider_errors.auto.finish(&id, new.as_ref()),
                Err(e) => {
                    tracing::warn!(agent = %id, "auto route failed: {e:#}");
                    daemon.provider_errors.auto.finish(&id, None);
                }
            }
        });
    }

    fn limit_reset_of(&self, agent: &Agent, error: Option<&ProviderError>) -> Option<i64> {
        error.and_then(|e| e.resets_at_ms).or_else(|| {
            let windows = crate::usage_limits::cached();
            window_reset(&windows, agent.kind)
        })
    }

    fn refresh_route_suggestion(&self, agent: &Agent) -> bool {
        let suggestion = self.route_suggestion_for(agent);
        self.provider_errors.set_suggestion(&agent.id, suggestion)
    }

    pub fn route_hook_event(
        self: &Arc<Self>,
        id: &AgentId,
        event: &HookEvent,
        reported: Option<DetectedError>,
    ) {
        match event {
            HookEvent::UserPromptSubmit => {
                self.provider_errors.auto.turn_started(id);
                self.clear_provider_error(id, None)
            }
            HookEvent::SessionStart { .. } => self.clear_provider_error(id, None),
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
                    self.set_provider_error(&agent, found, Source::TurnEnd);
                }
                if let Ok(Some(agent)) = self.store.get_agent(id) {
                    if self.refresh_route_suggestion(&agent) {
                        let _ = self.broadcast_agent(id);
                    }
                    self.auto_route(&agent);
                }
            }
            _ => {}
        }
    }

    pub fn scan_provider_errors(self: &Arc<Self>) {
        let Ok((_, _, agents, _)) = self.store.load_tree() else {
            return;
        };
        for agent in agents.iter().filter(|a| !a.archived) {
            if agent.status == AgentStatus::Running {
                match self.screen_error(agent) {
                    Some(found) => {
                        if self.provider_errors.get(&agent.id).is_none() {
                            tracing::info!(agent = %agent.id, kind = ?found.kind, "provider error on screen");
                        }
                        self.set_provider_error(agent, found, Source::Screen)
                    }
                    None => self.clear_provider_error(&agent.id, Some(Source::Screen)),
                }
            }
            if self.refresh_route_suggestion(agent) {
                let _ = self.broadcast_agent(&agent.id);
            }
            self.auto_route(agent);
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
        let error = self.provider_errors.get(id);
        self.provider_errors.auto.drop_return(id);
        if let Some(resets_at_ms) = error.as_ref().and_then(|e| e.resets_at_ms) {
            self.provider_errors.auto.offer_return(
                id,
                auto::ReturnOffer {
                    kind: agent.kind,
                    model: agent.model.clone(),
                    resets_at_ms,
                },
            );
        }
        self.store.set_agent_model(id, model.as_deref())?;
        self.provider_errors.clear(id, None);
        self.provider_errors.set_suggestion(id, None);
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
        let error = self.provider_errors.get(id);
        let transcript = self.transcript_path(id);
        let conversation = transcript
            .as_deref()
            .map(handoff::conversation)
            .unwrap_or_default();
        let worktree = self.store.get_worktree(&old.worktree_id)?;
        let git = match &worktree {
            Some(w) => handoff::git_state(&w.path).await,
            None => None,
        };
        let task_note = self.store.task_note_for_agent(id).ok();
        let reason = match &error {
            Some(e) => kind_label(e.kind),
            None => "the user moved it to another agent",
        };
        let prompt = handoff::handoff_prompt(&handoff::Handoff {
            old: &old,
            task_note,
            conversation,
            git,
            transcript,
            error: error.as_ref(),
            reason,
        });
        let mut spec = self.sibling_spec(id, Some(kind), &prompt)?;
        if model.is_some() {
            spec.model = model;
        }
        tracing::info!(agent = %id, to = ?kind, "continue agent in another kind");
        let created = self.create_agent(spec).await?;
        if let EntityId::Agent(new_id) = &created {
            self.store.set_routed_from(new_id, id)?;
            let limited = error
                .as_ref()
                .is_none_or(|e| e.kind == ProviderErrorKind::UsageLimit);
            if let Some(resets_at_ms) = limited
                .then(|| self.limit_reset_of(&old, error.as_ref()))
                .flatten()
            {
                self.provider_errors.auto.offer_return(
                    new_id,
                    auto::ReturnOffer {
                        kind: old.kind,
                        model: old.model.clone(),
                        resets_at_ms,
                    },
                );
            }
            let _ = self.broadcast_agent(new_id);
        }
        self.provider_errors.set_suggestion(id, None);
        let _ = self.broadcast_agent(id);
        Ok(created)
    }

    pub fn router_config(&self) -> Result<(String, String)> {
        let path = router_path();
        let file = match std::fs::read_to_string(&path) {
            Ok(text) => rules::parse(&text)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("{} does not parse", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => rules::RouterFile::default(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok((config::to_json(&file)?, path.display().to_string()))
    }

    pub fn save_router_config(self: &Arc<Self>, json: &str) -> Result<(String, String)> {
        let (file, text) = config::from_json(json)?;
        let path = router_path();
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, &text).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
        *self.provider_errors.rules.lock().unwrap() = None;
        tracing::info!(rules = file.rules.len(), mode = %file.mode, "router.toml saved");
        self.scan_provider_errors();
        Ok((config::to_json(&file)?, path.display().to_string()))
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
            resets_at_ms: None,
        }
    }

    #[test]
    fn screen_errors_clear_only_themselves() {
        let errors = ProviderErrors::default();
        let id = AgentId("a".into());
        assert!(errors.set(
            &id,
            found(ProviderErrorKind::Overloaded, "x"),
            Source::TurnEnd,
            None
        ));
        assert!(!errors.set(
            &id,
            found(ProviderErrorKind::Overloaded, "x"),
            Source::TurnEnd,
            None
        ));
        assert!(!errors.clear(&id, Some(Source::Screen)));
        assert!(errors.get(&id).is_some());
        assert!(errors.clear(&id, None));
        assert!(errors.get(&id).is_none());
    }

    #[test]
    fn usage_limit_takes_the_full_window_reset() {
        let errors = ProviderErrors::default();
        let id = AgentId("a".into());
        errors.set(
            &id,
            found(ProviderErrorKind::UsageLimit, "You've hit your limit"),
            Source::TurnEnd,
            Some(42),
        );
        assert_eq!(errors.get(&id).unwrap().resets_at_ms, Some(42));
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
