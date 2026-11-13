use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use berth_core::{
    NewTrigger, PromptTrigger, SessionRef, TriggerId, TriggerTarget, TriggerWhen,
};
use subtle::ConstantTimeEq;

use crate::config::Config;
use crate::pty::{DEFAULT_COLS, DEFAULT_ROWS};
use crate::registry::{CreateAgentSpec, Daemon};

pub const PROMPT_MAX_BYTES: usize = 16 * 1024;
pub const NAME_MAX_CHARS: usize = 80;
pub const MIN_REPEAT_SECS: u64 = 60;
const SCHEDULER_TICK: Duration = Duration::from_secs(5);
const BOOT_GRACE: Duration = Duration::from_secs(6);
const PASTE_GAP: Duration = Duration::from_millis(150);
const MIN_WEBHOOK_GAP: Duration = Duration::from_secs(5);
const DELIVERY_MEMORY: usize = 512;
const WEBHOOK_BODY_LIMIT: usize = 64 * 1024;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn generate_secret() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn next_run(run_at: i64, repeat_secs: Option<u64>, now: i64) -> Option<i64> {
    let period = i64::try_from(repeat_secs?).ok()?.checked_mul(1000)?;
    let periods_behind = (now - run_at).max(0) / period + 1;
    Some(run_at + periods_behind * period)
}

fn paste(prompt: &str) -> Vec<u8> {
    format!("\x1b[200~{prompt}\x1b[201~").into_bytes()
}

impl Daemon {
    pub fn create_trigger(&self, new: NewTrigger) -> Result<(PromptTrigger, Option<String>)> {
        let name = new.name.trim().to_string();
        ensure!(!name.is_empty(), "name is empty");
        ensure!(
            name.chars().count() <= NAME_MAX_CHARS,
            "name is longer than {NAME_MAX_CHARS} characters"
        );
        let prompt = new.prompt.trim().to_string();
        ensure!(!prompt.is_empty(), "prompt is empty");
        ensure!(
            prompt.len() <= PROMPT_MAX_BYTES,
            "prompt is longer than {PROMPT_MAX_BYTES} bytes"
        );
        ensure!(
            !prompt.contains(['\0', '\x1b']),
            "prompt cannot contain control escapes"
        );
        if let TriggerWhen::At {
            run_at,
            repeat_secs,
        } = &new.when
        {
            ensure!(*run_at > 0, "run time is not set");
            if let Some(secs) = repeat_secs {
                ensure!(
                    *secs >= MIN_REPEAT_SECS,
                    "a repeat shorter than {MIN_REPEAT_SECS} seconds is refused"
                );
            }
        }
        match &new.target {
            TriggerTarget::Agent(id) => {
                let agent = self.store.get_agent(id)?.context("agent not found")?;
                ensure!(!agent.archived, "agent is archived");
            }
            TriggerTarget::NewAgent { worktree, .. } => {
                self.store
                    .get_worktree(worktree)?
                    .context("worktree not found")?;
            }
        }
        let secret = matches!(new.when, TriggerWhen::Webhook).then(generate_secret);
        let trigger = PromptTrigger {
            id: TriggerId(ulid::Ulid::generate().to_string()),
            name,
            when: new.when,
            target: new.target,
            prompt,
            last_fired_at: None,
        };
        self.store.insert_trigger(&trigger, secret.as_deref())?;
        Ok((trigger, secret))
    }

    pub fn delete_trigger(&self, id: &TriggerId) -> Result<()> {
        ensure!(self.store.delete_trigger(id)?, "trigger not found");
        Ok(())
    }

    pub(crate) fn claim_due_triggers(&self, now: i64) -> Vec<PromptTrigger> {
        let due = match self.store.due_triggers(now) {
            Ok(due) => due,
            Err(e) => {
                tracing::warn!(error = %e, "reading due triggers failed");
                return Vec::new();
            }
        };
        due.into_iter()
            .filter(|trigger| {
                let TriggerWhen::At {
                    run_at,
                    repeat_secs,
                } = &trigger.when
                else {
                    return false;
                };
                let next = next_run(*run_at, *repeat_secs, now);
                self.store
                    .claim_due_trigger(&trigger.id, now, next)
                    .unwrap_or(false)
            })
            .collect()
    }

    pub async fn run_scheduler(self: Arc<Self>) {
        let mut interval = tokio::time::interval(SCHEDULER_TICK);
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = interval.tick() => {}
            }
            for trigger in self.claim_due_triggers(now_ms()) {
                self.spawn_fire(trigger);
            }
        }
    }

    fn spawn_fire(self: &Arc<Self>, trigger: PromptTrigger) {
        let daemon = self.clone();
        tokio::spawn(async move {
            if let Err(e) = daemon.fire_trigger(&trigger).await {
                tracing::warn!(trigger = %trigger.id.0, error = %e, "prompt trigger failed");
            }
        });
    }

    async fn fire_trigger(self: &Arc<Self>, trigger: &PromptTrigger) -> Result<()> {
        match &trigger.target {
            TriggerTarget::Agent(id) => {
                let sref = SessionRef::Agent(id.clone());
                let was_alive = self.is_alive(&sref);
                let session = self.ensure_session(&sref, DEFAULT_COLS, DEFAULT_ROWS)?;
                if !was_alive {
                    tokio::time::sleep(BOOT_GRACE).await;
                }
                session.write_input(&paste(&trigger.prompt))?;
                tokio::time::sleep(PASTE_GAP).await;
                session.write_input(b"\r")
            }
            TriggerTarget::NewAgent { worktree, kind } => {
                let (_, _, agents, _) = self.store.load_tree()?;
                let taken = agents
                    .iter()
                    .filter(|a| &a.worktree_id == worktree)
                    .map(|a| a.name.clone())
                    .collect::<Vec<_>>();
                self.create_agent(CreateAgentSpec {
                    worktree: worktree.clone(),
                    name: crate::sibling::sibling_name(&taken),
                    kind: *kind,
                    model: None,
                    effort: None,
                    auto_title: true,
                    cloud_prompt: None,
                    starting_prompt: Some(trigger.prompt.clone()),
                    pr_url: None,
                })
                .await
                .map(|_| ())
            }
        }
    }
}

struct WebhookState {
    daemon: Arc<Daemon>,
    seen: Mutex<(HashSet<(String, String)>, VecDeque<(String, String)>)>,
    last_fired: Mutex<HashMap<String, Instant>>,
}

impl WebhookState {
    fn receive(&self, id: &str, presented: &str, delivery: Option<&str>) -> (StatusCode, &'static str) {
        let found = self
            .daemon
            .store
            .trigger_with_secret(&TriggerId(id.to_string()))
            .ok()
            .flatten();
        let expected = found
            .as_ref()
            .map_or_else(|| "0".repeat(64), |(_, secret)| secret.clone());
        let secret_ok: bool = presented.as_bytes().ct_eq(expected.as_bytes()).into();
        let Some((trigger, _)) = found.filter(|_| secret_ok) else {
            return (StatusCode::UNAUTHORIZED, "unauthorized");
        };
        if let Some(delivery) = delivery {
            let key = (id.to_string(), delivery.to_string());
            if self.seen.lock().unwrap().0.contains(&key) {
                return (StatusCode::OK, "duplicate");
            }
        }
        {
            let mut last = self.last_fired.lock().unwrap();
            if last
                .get(id)
                .is_some_and(|at| at.elapsed() < MIN_WEBHOOK_GAP)
            {
                return (StatusCode::TOO_MANY_REQUESTS, "too many requests");
            }
            last.insert(id.to_string(), Instant::now());
        }
        if let Some(delivery) = delivery {
            let key = (id.to_string(), delivery.to_string());
            let mut seen = self.seen.lock().unwrap();
            if seen.0.insert(key.clone()) {
                seen.1.push_back(key);
                while seen.1.len() > DELIVERY_MEMORY {
                    if let Some(old) = seen.1.pop_front() {
                        seen.0.remove(&old);
                    }
                }
            }
        }
        let _ = self.daemon.store.mark_trigger_fired(&trigger.id, now_ms());
        self.daemon.spawn_fire(trigger);
        (StatusCode::ACCEPTED, "accepted")
    }
}

fn presented_secret(headers: &HeaderMap) -> String {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    header("x-berth-secret")
        .or_else(|| header("authorization").and_then(|v| v.strip_prefix("Bearer ")))
        .unwrap_or_default()
        .trim()
        .to_string()
}

async fn receive_webhook(
    State(state): State<Arc<WebhookState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> (StatusCode, &'static str) {
    let delivery = headers
        .get("x-berth-delivery")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty() && v.len() <= 128);
    state.receive(&id, &presented_secret(&headers), delivery)
}

pub async fn start_webhooks(daemon: Arc<Daemon>, config: &Config) -> Result<()> {
    if !config.webhooks {
        return Ok(());
    }
    let host = if config.webhook_expose {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let listener = tokio::net::TcpListener::bind((host, config.webhook_port))
        .await
        .with_context(|| format!("webhook listener on {host}:{}", config.webhook_port))?;
    tracing::info!(addr = %listener.local_addr()?, "webhook listener up");
    let shutdown = daemon.shutdown.clone();
    let state = Arc::new(WebhookState {
        daemon,
        seen: Mutex::default(),
        last_fired: Mutex::default(),
    });
    let app = Router::new()
        .route("/hooks/{id}", post(receive_webhook))
        .layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT))
        .with_state(state);
    tokio::spawn(async move {
        let served = axum::serve(listener, app)
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await;
        if let Err(e) = served {
            tracing::error!(error = %e, "webhook listener died");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use berth_core::{AgentId, AgentKind, Project, ProjectId, Worktree, WorktreeId};

    use super::*;
    use crate::hooks::HookEnv;
    use crate::store::Store;

    fn daemon() -> Arc<Daemon> {
        let daemon = Daemon::new(
            Arc::new(Store::open_in_memory().unwrap()),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        );
        daemon
            .store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: ProjectId("p".into()),
                name: "p".into(),
                repo_path: "/berth-test/p".into(),
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        daemon
            .store
            .insert_worktree(&Worktree {
                id: WorktreeId("w".into()),
                project_id: ProjectId("p".into()),
                path: "/berth-test/p".into(),
                branch: "main".into(),
                is_main: true,
                sort_order: 0,
            })
            .unwrap();
        daemon
    }

    fn new_trigger(when: TriggerWhen) -> NewTrigger {
        NewTrigger {
            name: "nightly".into(),
            when,
            target: TriggerTarget::NewAgent {
                worktree: WorktreeId("w".into()),
                kind: AgentKind::Claude,
            },
            prompt: "run the checks".into(),
        }
    }

    #[test]
    fn a_scheduled_prompt_fires_once() {
        let daemon = daemon();
        let at = TriggerWhen::At {
            run_at: 1_000,
            repeat_secs: None,
        };
        daemon.create_trigger(new_trigger(at)).unwrap();
        assert!(daemon.claim_due_triggers(999).is_empty());
        assert_eq!(daemon.claim_due_triggers(1_000).len(), 1);
        assert!(daemon.claim_due_triggers(2_000).is_empty());
        assert!(daemon.store.list_triggers().unwrap().is_empty());
    }

    #[test]
    fn a_repeating_prompt_missed_while_down_runs_once_and_rearms_ahead() {
        let daemon = daemon();
        let at = TriggerWhen::At {
            run_at: 1_000,
            repeat_secs: Some(60),
        };
        daemon.create_trigger(new_trigger(at)).unwrap();
        let now = 1_000 + 10 * 60_000 + 5;
        assert_eq!(daemon.claim_due_triggers(now).len(), 1);
        assert!(daemon.claim_due_triggers(now).is_empty());
        let [kept] = daemon.store.list_triggers().unwrap().try_into().unwrap();
        assert_eq!(
            kept.when,
            TriggerWhen::At {
                run_at: 1_000 + 11 * 60_000,
                repeat_secs: Some(60)
            }
        );
        assert_eq!(kept.last_fired_at, Some(now));
    }

    #[test]
    fn a_schedule_survives_reopening_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("berth.db");
        let id = {
            let store = Store::open(&path).unwrap();
            let daemon = Daemon::new(
                Arc::new(store),
                HookEnv {
                    port: 0,
                    token: String::new(),
                },
            );
            daemon
                .store
                .insert_project(&Project {
                    workspace_id: Default::default(),
                    id: ProjectId("p".into()),
                    name: "p".into(),
                    repo_path: "/berth-test/p".into(),
                    sort_order: 0,
                    pin_order: None,
                })
                .unwrap();
            daemon
                .store
                .insert_worktree(&Worktree {
                    id: WorktreeId("w".into()),
                    project_id: ProjectId("p".into()),
                    path: "/berth-test/p".into(),
                    branch: "main".into(),
                    is_main: true,
                    sort_order: 0,
                })
                .unwrap();
            let at = TriggerWhen::At {
                run_at: 5,
                repeat_secs: None,
            };
            daemon.create_trigger(new_trigger(at)).unwrap().0.id
        };
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.due_triggers(10).unwrap()[0].id, id);
    }

    #[test]
    fn invalid_triggers_are_refused() {
        let daemon = daemon();
        let mut bad = new_trigger(TriggerWhen::Webhook);
        bad.prompt = "  ".into();
        assert!(daemon.create_trigger(bad).is_err());
        let mut bad = new_trigger(TriggerWhen::Webhook);
        bad.prompt = "x\x1b[201~y".into();
        assert!(daemon.create_trigger(bad).is_err());
        let mut bad = new_trigger(TriggerWhen::Webhook);
        bad.target = TriggerTarget::Agent(AgentId("ghost".into()));
        assert!(daemon.create_trigger(bad).is_err());
        let fast = TriggerWhen::At {
            run_at: 1,
            repeat_secs: Some(1),
        };
        assert!(daemon.create_trigger(new_trigger(fast)).is_err());
    }

    #[tokio::test]
    async fn a_webhook_needs_its_own_secret_and_dedupes_deliveries() {
        let daemon = daemon();
        let (hook, secret) = daemon
            .create_trigger(new_trigger(TriggerWhen::Webhook))
            .unwrap();
        let (_, other_secret) = daemon
            .create_trigger(new_trigger(TriggerWhen::Webhook))
            .unwrap();
        let (hook, secret, other_secret) = (hook.id.0, secret.unwrap(), other_secret.unwrap());
        let state = WebhookState {
            daemon: daemon.clone(),
            seen: Mutex::default(),
            last_fired: Mutex::default(),
        };

        assert_eq!(state.receive(&hook, "", None).0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            state.receive(&hook, "wrong", None).0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            state.receive(&hook, &other_secret, None).0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            state.receive("missing", &secret, None).0,
            StatusCode::UNAUTHORIZED
        );
        assert!(daemon.store.list_triggers().unwrap()[0]
            .last_fired_at
            .is_none());

        assert_eq!(
            state.receive(&hook, &secret, Some("d1")).0,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            state.receive(&hook, &secret, Some("d1")),
            (StatusCode::OK, "duplicate")
        );
        assert_eq!(
            state.receive(&hook, &secret, Some("d2")).0,
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn the_secret_is_read_from_either_header() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer abc".parse().unwrap());
        assert_eq!(presented_secret(&headers), "abc");
        headers.insert("x-berth-secret", "def".parse().unwrap());
        assert_eq!(presented_secret(&headers), "def");
        assert_eq!(presented_secret(&HeaderMap::new()), "");
    }
}
