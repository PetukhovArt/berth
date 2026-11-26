use super::AcpLive;
use crate::registry::Daemon;
use anyhow::Result;
use berth_core::{AgentId, ServerEvent};
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn command_name(text: &str) -> Option<&str> {
    let word = text.trim().strip_prefix('/')?.split_whitespace().next()?;
    let valid = word.starts_with(|c: char| c.is_ascii_alphabetic())
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'));
    valid.then_some(word)
}

impl Daemon {
    pub(super) async fn acp_slash(
        self: &Arc<Self>,
        agent_id: &AgentId,
        live: &AcpLive,
        text: &str,
    ) -> Result<bool> {
        let Some(name) = command_name(text) else {
            return Ok(false);
        };
        let (listed, unlisted, can_rewind) = {
            let state = live.state.lock().unwrap();
            let listed = state.commands.iter().any(|command| {
                command
                    .name
                    .trim_start_matches('/')
                    .eq_ignore_ascii_case(name)
            });
            (listed, state.commands.is_empty(), state.caps.rewind)
        };
        if name.eq_ignore_ascii_case("clear") && can_rewind {
            if live.turn_open.load(Ordering::SeqCst) {
                self.acp_notice(agent_id, "Wait for the turn to end before /clear.".into());
            } else {
                self.acp_clear(agent_id, live).await?;
            }
            return Ok(true);
        }
        if !listed && !unlisted {
            self.acp_notice(
                agent_id,
                format!("/{name} is not a command this agent offers in the chat. It was not sent."),
            );
            return Ok(true);
        }
        Ok(false)
    }

    async fn acp_clear(&self, agent_id: &AgentId, live: &AcpLive) -> Result<()> {
        self.acp_rewind_session(agent_id, live, None).await?;
        self.acp.persist.flush(&self.store, agent_id);
        self.store.delete_acp_items_from(agent_id, 0)?;
        self.broadcast(ServerEvent::AcpTruncated {
            agent: agent_id.clone(),
            from_seq: 0,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::command_name;

    #[test]
    fn a_command_is_a_leading_slash_word_and_a_path_is_not() {
        assert_eq!(command_name("/clear"), Some("clear"));
        assert_eq!(
            command_name("  /jig:jig-review pr 4"),
            Some("jig:jig-review")
        );
        assert_eq!(command_name("/etc/hosts what is it"), None);
        assert_eq!(command_name("/ 5 / 2"), None);
        assert_eq!(command_name("clear"), None);
    }
}
