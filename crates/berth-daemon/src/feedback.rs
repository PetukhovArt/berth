use crate::registry::Daemon;
use crate::status::HookEvent;
use berth_core::{AgentId, FeedbackKind, FeedbackRequest, ServerEvent};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

const DETAIL_KEYS: [&str; 5] = ["command", "file_path", "path", "url", "pattern"];
const MAX_DETAIL_CHARS: usize = 64 * 1024;
const MAX_PREVIEW_CHARS: usize = 400;
const CUT_NOTE: &str = "\n[cut: the text is too long to show in full]";
pub(crate) const MAX_OPTIONS: usize = 9;
pub(crate) const ALLOW_KEY: &[u8] = b"1";
pub(crate) const ALWAYS_ALLOW_KEY: &[u8] = b"2";
pub(crate) const DENY_KEY: &[u8] = b"\x1b";
const ENTER_KEY: &[u8] = b"\r";

#[derive(Default)]
pub struct PendingFeedback {
    requests: Mutex<HashMap<AgentId, FeedbackRequest>>,
}

impl PendingFeedback {
    fn answer_typed(&self, agent: &AgentId, keys: &[u8]) -> Option<HookEvent> {
        answer_in(self.requests.lock().unwrap().get(agent)?, keys)
    }

    pub fn set(&self, agent: &AgentId, request: FeedbackRequest) {
        self.requests.lock().unwrap().insert(agent.clone(), request);
    }

    pub fn clear(&self, agent: &AgentId) -> bool {
        self.requests.lock().unwrap().remove(agent).is_some()
    }

    pub fn all(&self) -> Vec<(AgentId, FeedbackRequest)> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|(id, request)| (id.clone(), request.clone()))
            .collect()
    }
}

impl Daemon {
    pub fn set_feedback_request(&self, agent: &AgentId, request: FeedbackRequest) {
        self.feedback.set(agent, request.clone());
        self.broadcast(ServerEvent::FeedbackRequestChanged {
            agent: agent.clone(),
            request: Some(request),
        });
    }

    pub fn clear_feedback(&self, agent: &AgentId) {
        if self.feedback.clear(agent) {
            self.broadcast(ServerEvent::FeedbackRequestChanged {
                agent: agent.clone(),
                request: None,
            });
        }
    }

    pub fn note_session_killed(&self, agent: &AgentId) {
        self.clear_feedback(agent);
        self.apply_hook_event(agent, HookEvent::Killed, None);
    }

    pub fn note_typed_into_agent(&self, agent: &AgentId, keys: &[u8]) {
        if let Some(answer) = self.feedback.answer_typed(agent, keys) {
            self.apply_hook_event(agent, answer, None);
        }
    }
}

fn answer_in(request: &FeedbackRequest, keys: &[u8]) -> Option<HookEvent> {
    let permission = request.kind == FeedbackKind::Permission;
    let declined = match keys {
        DENY_KEY => true,
        ALLOW_KEY | ENTER_KEY if permission => false,
        ALWAYS_ALLOW_KEY if permission && request.always_allow => false,
        _ => return None,
    };
    Some(HookEvent::FeedbackAnswered { declined })
}

pub fn from_hook(
    hook_event: &str,
    tool_name: Option<&str>,
    tool_input: Option<&Value>,
) -> Option<FeedbackRequest> {
    match (hook_event, tool_name) {
        ("PermissionRequest", Some("AskUserQuestion"))
        | ("PreToolUse", Some("AskUserQuestion")) => question(tool_input?),
        ("PermissionRequest", Some(tool)) => Some(FeedbackRequest {
            kind: FeedbackKind::Permission,
            title: tool.to_string(),
            detail: tool_input.and_then(detail),
            options: Vec::new(),
            always_allow: false,
            form: None,
        }),
        _ => None,
    }
}

pub fn with_permission_suggestions(
    mut request: FeedbackRequest,
    suggestions: Option<&Value>,
) -> FeedbackRequest {
    request.always_allow = request.kind == FeedbackKind::Permission
        && suggestions
            .and_then(Value::as_array)
            .is_some_and(|list| !list.is_empty());
    request
}

fn question(input: &Value) -> Option<FeedbackRequest> {
    let first = input.get("questions")?.as_array()?.first()?;
    let title = first.get("question")?.as_str()?.to_string();
    let options = first
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(|o| o.get("label").and_then(Value::as_str))
                .take(MAX_OPTIONS)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(FeedbackRequest {
        kind: FeedbackKind::Question,
        title,
        detail: None,
        options,
        always_allow: false,
        form: None,
    })
}

pub(crate) fn detail(input: &Value) -> Option<String> {
    let text = detail_text(input)?;
    match text.char_indices().nth(MAX_DETAIL_CHARS) {
        None => Some(text.to_string()),
        Some((end, _)) => Some(format!("{}{CUT_NOTE}", &text[..end])),
    }
}

pub(crate) fn preview(input: &Value) -> Option<String> {
    Some(
        detail_text(input)?
            .chars()
            .take(MAX_PREVIEW_CHARS)
            .collect(),
    )
}

fn detail_text(input: &Value) -> Option<&str> {
    DETAIL_KEYS
        .iter()
        .find_map(|key| input.get(key).and_then(Value::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn permission_names_the_tool_and_its_command() {
        let input = json!({"command": "pnpm test coupons", "description": "run"});
        let request = from_hook("PermissionRequest", Some("Bash"), Some(&input)).unwrap();
        assert_eq!(request.kind, FeedbackKind::Permission);
        assert_eq!(request.title, "Bash");
        assert_eq!(request.detail.as_deref(), Some("pnpm test coupons"));
    }

    #[test]
    fn a_long_command_reaches_the_approval_whole() {
        let command = format!("echo {}", "x".repeat(995));
        let input = json!({ "command": command });
        let request = from_hook("PermissionRequest", Some("Bash"), Some(&input)).unwrap();
        assert_eq!(request.detail.as_deref(), Some(command.as_str()));
    }

    #[test]
    fn an_absurd_command_is_cut_and_says_so() {
        let input = json!({ "command": "y".repeat(MAX_DETAIL_CHARS + 5) });
        let request = from_hook("PermissionRequest", Some("Bash"), Some(&input)).unwrap();
        let detail = request.detail.unwrap();
        assert!(detail.starts_with(&"y".repeat(MAX_DETAIL_CHARS)));
        assert!(detail.ends_with(CUT_NOTE));
    }

    #[test]
    fn question_lists_option_labels() {
        let input = json!({"questions": [{"question": "Which rounding?", "options": [
            {"label": "Half up", "description": ""}, {"label": "Half even", "description": ""}]}]});
        let request = from_hook("PreToolUse", Some("AskUserQuestion"), Some(&input)).unwrap();
        assert_eq!(request.kind, FeedbackKind::Question);
        assert_eq!(request.title, "Which rounding?");
        assert_eq!(request.options, vec!["Half up", "Half even"]);
    }

    #[test]
    fn other_hooks_carry_no_request() {
        assert!(from_hook("PreToolUse", Some("Bash"), None).is_none());
        assert!(from_hook("Stop", None, None).is_none());
    }

    #[test]
    fn pending_is_cleared_once() {
        let pending = PendingFeedback::default();
        let agent = AgentId("a".into());
        pending.set(
            &agent,
            from_hook("PermissionRequest", Some("Bash"), None).unwrap(),
        );
        assert_eq!(pending.all().len(), 1);
        assert!(pending.clear(&agent));
        assert!(!pending.clear(&agent));
    }

    #[test]
    fn a_killed_session_takes_its_request_card_away() {
        let daemon = crate::registry::tests::test_daemon();
        let agent = AgentId("a".into());
        let request = from_hook("PermissionRequest", Some("Bash"), None).unwrap();
        daemon.set_feedback_request(&agent, request);
        let mut rx = daemon.events.subscribe();

        daemon.kill_session(&berth_core::SessionRef::Agent(agent.clone()));

        assert!(daemon.feedback.all().is_empty(), "nobody is left to answer");
        assert!(matches!(
            rx.try_recv().unwrap(),
            ServerEvent::FeedbackRequestChanged { request: None, .. }
        ));
    }

    fn answered(declined: bool) -> Option<HookEvent> {
        Some(HookEvent::FeedbackAnswered { declined })
    }

    #[test]
    fn the_card_keys_answer_a_permission() {
        let mut request = from_hook("PermissionRequest", Some("Bash"), None).unwrap();
        assert_eq!(answer_in(&request, ALLOW_KEY), answered(false));
        assert_eq!(answer_in(&request, DENY_KEY), answered(true));
        assert_eq!(answer_in(&request, ALWAYS_ALLOW_KEY), None);
        request.always_allow = true;
        assert_eq!(answer_in(&request, ALWAYS_ALLOW_KEY), answered(false));
        for typed in [&b"3"[..], b"\x1b[A", b"\x1b[B", b"1\r", b"yes"] {
            assert_eq!(answer_in(&request, typed), None, "{typed:?}");
        }
    }

    #[test]
    fn enter_on_the_highlighted_choice_answers_a_permission_but_not_a_question() {
        let request = from_hook("PermissionRequest", Some("Edit"), None).unwrap();
        assert_eq!(answer_in(&request, ENTER_KEY), answered(false));
        let input = json!({"questions": [{"question": "Which?", "options": [
            {"label": "A"}, {"label": "B"}]}]});
        let question = from_hook("PreToolUse", Some("AskUserQuestion"), Some(&input)).unwrap();
        assert_eq!(answer_in(&question, ENTER_KEY), None);
    }

    #[test]
    fn only_a_rejection_answers_a_question() {
        let input = json!({"questions": [{"question": "Which?", "options": [
            {"label": "A"}, {"label": "B"}]}]});
        let request = from_hook("PreToolUse", Some("AskUserQuestion"), Some(&input)).unwrap();
        assert_eq!(answer_in(&request, DENY_KEY), answered(true));
        assert_eq!(answer_in(&request, b"1"), None);
        assert_eq!(answer_in(&request, b"2"), None);
    }

    #[test]
    fn an_allow_typed_into_a_waiting_agent_ends_the_wait_and_takes_the_card() {
        use crate::registry::tests::{seed_agent, seed_projects, seed_worktree, test_daemon};
        use berth_core::AgentStatus;
        let daemon = test_daemon();
        seed_projects(&daemon, &["p"]);
        seed_worktree(&daemon, "p", "w", "/tmp/p", true);
        seed_agent(&daemon, "a", "w", Some("s1"));
        let agent = AgentId("a".into());
        let input = json!({"command": "cargo test"});
        let asked = HookEvent::PermissionRequest {
            tool_name: Some("Bash".into()),
            subagent_id: None,
            detail: detail(&input),
        };
        daemon.apply_hook_event(&agent, asked, Some("s1".into()));
        let request = from_hook("PermissionRequest", Some("Bash"), Some(&input)).unwrap();
        daemon.set_feedback_request(&agent, request);
        let status = || daemon.store.get_agent(&agent).unwrap().unwrap().status;
        assert_eq!(status(), AgentStatus::NeedsFeedback);

        daemon.note_typed_into_agent(&agent, b"\x1b[B");
        assert_eq!(
            status(),
            AgentStatus::NeedsFeedback,
            "an arrow key is no answer"
        );

        daemon.note_typed_into_agent(&agent, ALLOW_KEY);
        assert_eq!(status(), AgentStatus::Running);
        assert!(daemon.feedback.all().is_empty(), "the card is answered");
    }
}
