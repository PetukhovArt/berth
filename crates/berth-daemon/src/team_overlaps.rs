use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use berth_core::SessionRef;

use crate::channels::hub;
use crate::git::git;
use crate::hooks::installer::WRITTEN_IN_CHECKOUT;
use crate::presence::on_tasks;
use crate::registry::Daemon;

const REPORT_EVERY: Duration = Duration::from_secs(15);
const MODULE_DEPTH: usize = 2;
const DEFAULT_BRANCHES: [&str; 5] = [
    "origin/HEAD",
    "main",
    "master",
    "origin/main",
    "origin/master",
];

pub fn module_of(file: &str) -> String {
    let parts: Vec<&str> = file.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 1 {
        return parts.join("/");
    }
    parts[..MODULE_DEPTH.min(parts.len() - 1)].join("/")
}

pub fn modules_of<'a>(files: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    files
        .into_iter()
        .filter(|f| !f.is_empty())
        .map(module_of)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn written_by_berth(file: &str) -> bool {
    WRITTEN_IN_CHECKOUT.contains(&file)
}

fn names(out: &str) -> impl Iterator<Item = &str> {
    out.split('\0')
        .filter(|f| !f.is_empty() && !written_by_berth(f))
}

async fn merge_base(worktree: &Path) -> String {
    for default in DEFAULT_BRANCHES {
        if let Ok(out) = git(worktree, &["merge-base", "HEAD", default]).await {
            return out.trim().to_string();
        }
    }
    "HEAD".to_string()
}

async fn touched_modules(worktree: &Path) -> Vec<String> {
    let base = merge_base(worktree).await;
    let diff = ["--no-optional-locks", "diff", "--name-only", "-z", &base];
    let others = [
        "--no-optional-locks",
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
    ];
    let (tracked, untracked) = tokio::join!(git(worktree, &diff), git(worktree, &others));
    let (tracked, untracked) = (tracked.unwrap_or_default(), untracked.unwrap_or_default());
    modules_of(names(&tracked).chain(names(&untracked)))
}

impl Daemon {
    fn running_in_worktrees(&self) -> Vec<(String, String, PathBuf)> {
        self.session_pids()
            .into_iter()
            .filter_map(|(sref, _, _)| match sref {
                SessionRef::Agent(id) => {
                    let agent = self.store.get_agent(&id).ok().flatten()?;
                    let worktree = self.store.get_worktree(&agent.worktree_id).ok().flatten()?;
                    Some((agent.name, worktree.branch, worktree.path))
                }
                _ => None,
            })
            .collect()
    }

    pub async fn run_touched_report(self: Arc<Self>) {
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
            let running = self.running_in_worktrees();
            let branches: Vec<(String, String)> = running
                .iter()
                .map(|(agent, branch, _)| (agent.clone(), branch.clone()))
                .collect();
            let mut report: HashMap<String, BTreeMap<String, Vec<String>>> = HashMap::new();
            let mut modules: HashMap<String, Vec<String>> = HashMap::new();
            for (key, agents) in on_tasks(&branches, &keys, false) {
                for agent in agents {
                    if !modules.contains_key(&agent) {
                        let Some((_, _, path)) = running.iter().find(|(a, _, _)| *a == agent)
                        else {
                            continue;
                        };
                        modules.insert(agent.clone(), touched_modules(path).await);
                    }
                    report
                        .entry(key.clone())
                        .or_default()
                        .insert(agent.clone(), modules[&agent].clone());
                }
            }
            let _ = hub.report_touched(&report).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_is_the_first_two_directories_and_a_root_file_is_its_own() {
        assert_eq!(
            module_of("crates/berth-hub/src/store.rs"),
            "crates/berth-hub"
        );
        assert_eq!(module_of("apps/web/package.json"), "apps/web");
        assert_eq!(module_of("docs/cli.md"), "docs");
        assert_eq!(module_of("Cargo.toml"), "Cargo.toml");
        assert_eq!(
            modules_of(["docs/a.md", "docs/b.md", "src/x/y.rs", ""]),
            ["docs", "src/x"]
        );
    }

    #[test]
    fn hook_files_berth_writes_are_no_module() {
        let out =
            ".claude/settings.local.json\0.cursor/hooks.json\0.claude/skills/x.md\0src/a/b.rs\0";
        assert_eq!(modules_of(names(out)), [".claude", "src/a"]);
    }
}
