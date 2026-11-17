use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use berth_core::SessionRef;

use crate::channels::hub;
use crate::registry::Daemon;

const REPORT_EVERY: Duration = Duration::from_secs(15);

pub fn on_tasks(
    running: &[(String, String)],
    keys: &[String],
    app_open: bool,
) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for key in keys {
        let needle = key.to_lowercase();
        let agents: Vec<String> = running
            .iter()
            .filter(|(_, branch_and_note)| branch_and_note.to_lowercase().contains(&needle))
            .map(|(agent, _)| agent.clone())
            .collect();
        if app_open || !agents.is_empty() {
            out.insert(key.clone(), agents);
        }
    }
    out
}

impl Daemon {
    fn running_on_tasks(&self) -> Vec<(String, String)> {
        self.session_pids()
            .into_iter()
            .filter_map(|(sref, _, _)| match sref {
                SessionRef::Agent(id) => {
                    let agent = self.store.get_agent(&id).ok().flatten()?;
                    let worktree = self.store.get_worktree(&agent.worktree_id).ok().flatten()?;
                    let note = self.store.task_note(&worktree.id).unwrap_or_default();
                    Some((agent.name, format!("{}\n{note}", worktree.branch)))
                }
                _ => None,
            })
            .collect()
    }

    pub async fn run_presence_report(self: Arc<Self>) {
        let mut interval = tokio::time::interval(REPORT_EVERY);
        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = interval.tick() => {}
            }
            let Ok(hub) = hub() else { continue };
            let Ok(channels) = hub.channels().await else {
                continue;
            };
            let keys: Vec<String> = channels.into_iter().map(|c| c.key).collect();
            let app_open = self.events.receiver_count() > 0;
            let _ = hub
                .report_presence(&on_tasks(&self.running_on_tasks(), &keys, app_open))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_agent_is_present_on_the_channel_its_branch_names() {
        let running = vec![
            ("agent-1".to_string(), "feature/PROJ-12-orders".to_string()),
            ("agent-2".to_string(), "main".to_string()),
        ];
        let keys = vec!["proj-12".to_string(), "PROJ-7".to_string()];
        let found = on_tasks(&running, &keys, false);
        assert_eq!(found.get("proj-12"), Some(&vec!["agent-1".to_string()]));
        assert!(!found.contains_key("PROJ-7"));
    }

    #[test]
    fn a_task_note_key_puts_the_agent_on_the_channel() {
        let running = vec![(
            "agent-1".to_string(),
            "main\nfinish PROJ-7 first".to_string(),
        )];
        let keys = vec!["PROJ-7".to_string()];
        let found = on_tasks(&running, &keys, false);
        assert_eq!(found.get("PROJ-7"), Some(&vec!["agent-1".to_string()]));
    }

    #[test]
    fn an_open_app_is_present_on_every_channel_even_without_agents() {
        let keys = vec!["PROJ-7".to_string()];
        let found = on_tasks(&[], &keys, true);
        assert_eq!(found.get("PROJ-7"), Some(&Vec::new()));
        assert!(on_tasks(&[], &keys, false).is_empty());
    }
}
