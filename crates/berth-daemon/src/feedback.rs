use crate::registry::Daemon;
use berth_core::{AgentId, FeedbackKind, FeedbackRequest, ServerEvent};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

const DETAIL_KEYS: [&str; 5] = ["command", "file_path", "path", "url", "pattern"];
const MAX_DETAIL_CHARS: usize = 400;
pub(crate) const MAX_OPTIONS: usize = 9;

#[derive(Default)]
pub struct PendingFeedback {
    requests: Mutex<HashMap<AgentId, FeedbackRequest>>,
}

impl PendingFeedback {
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
        }),
        _ => None,
    }
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
    })
}

pub(crate) fn detail(input: &Value) -> Option<String> {
    let text = DETAIL_KEYS
        .iter()
        .find_map(|key| input.get(key).and_then(Value::as_str))?;
    Some(text.chars().take(MAX_DETAIL_CHARS).collect())
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
}
