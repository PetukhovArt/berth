use std::sync::Arc;
use std::time::Duration;

use berth_core::{AgentId, AgentKind, SessionRef};

use crate::registry::Daemon;
use crate::status::HookEvent;

const GAP_BEFORE_SUBMIT: Duration = Duration::from_millis(30);

pub(crate) fn pasteable_into_warm_cli(kind: AgentKind, prompt: &str) -> bool {
    kind == AgentKind::Claude && !prompt.contains('\x1b')
}

pub(crate) fn warm_cli_booted(hooks: &[(HookEvent, Option<String>)]) -> bool {
    hooks
        .iter()
        .any(|(event, _)| matches!(event, HookEvent::SessionStart { .. }))
}

impl Daemon {
    pub(crate) fn type_starting_prompt(self: &Arc<Self>, agent: &AgentId, prompt: String) {
        let Some(session) = self.session(&SessionRef::Agent(agent.clone())) else {
            return;
        };
        let agent = agent.clone();
        tokio::spawn(async move {
            let pasted = session.write_input(&crate::triggers::paste(&prompt));
            tokio::time::sleep(GAP_BEFORE_SUBMIT).await;
            if let Err(error) = pasted.and_then(|()| session.write_input(b"\r")) {
                tracing::warn!(agent = %agent, error = %error, "starting prompt not typed into the warm CLI");
            }
        });
    }
}
