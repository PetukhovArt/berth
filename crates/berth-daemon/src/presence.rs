use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use berth_core::SessionRef;

use crate::channels::hub;
use crate::registry::Daemon;

const REPORT_EVERY: Duration = Duration::from_secs(15);

pub fn on_tasks(running: &[(String, String)], keys: &[String]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for key in keys {
        let needle = key.to_lowercase();
        for (agent, branch) in running {
            if branch.to_lowercase().contains(&needle) {
                out.entry(key.clone()).or_default().push(agent.clone());
            }
        }
    }
    out
}

impl Daemon {
    fn running_on_branches(&self) -> Vec<(String, String)> {
        self.session_pids()
            .into_iter()
            .filter_map(|(sref, _, _)| match sref {
                SessionRef::Agent(id) => {
                    let agent = self.store.get_agent(&id).ok().flatten()?;
                    let worktree = self.store.get_worktree(&agent.worktree_id).ok().flatten()?;
                    Some((agent.name, worktree.branch))
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
            let _ = hub
                .report_presence(&on_tasks(&self.running_on_branches(), &keys))
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
        let found = on_tasks(&running, &keys);
        assert_eq!(found.get("proj-12"), Some(&vec!["agent-1".to_string()]));
        assert!(!found.contains_key("PROJ-7"));
    }
}
