use std::sync::Arc;

use anyhow::{Context, Result};
use berth_core::{EntityId, ProjectId, SessionRef, Worktree};

use crate::registry::Daemon;

pub const SETUP_NAME: &str = "setup";
pub const SETUP_MAX_BYTES: usize = 4 * 1024;

impl Daemon {
    pub fn set_project_setup(&self, project: &ProjectId, command: &str) -> Result<()> {
        let command = command.trim();
        anyhow::ensure!(
            command.len() <= SETUP_MAX_BYTES,
            "Setup command is longer than {SETUP_MAX_BYTES} bytes"
        );
        anyhow::ensure!(
            !command.contains(['\r', '\n']),
            "Setup command must be one line"
        );
        self.store
            .get_project(project)?
            .context("project not found")?;
        self.store.set_project_setup(project, command)
    }

    pub(crate) fn run_project_setup(self: &Arc<Self>, worktree: &Worktree) {
        let command = match self.store.project_setup(&worktree.project_id) {
            Ok(command) if !command.is_empty() => command,
            Ok(_) => return,
            Err(e) => {
                tracing::warn!(error = %e, "reading the setup command failed");
                return;
            }
        };
        let started = self
            .create_terminal(&worktree.id, Some(SETUP_NAME.into()))
            .and_then(|id| {
                let EntityId::Terminal(id) = id else {
                    anyhow::bail!("not a terminal");
                };
                let session = self
                    .session(&SessionRef::Terminal(id))
                    .context("setup terminal did not start")?;
                session.write_input(format!("{command}\r").as_bytes())
            });
        if let Err(e) = started {
            tracing::warn!(worktree = %worktree.id, error = %e, "starting the setup command failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use berth_core::{Project, ProjectId};

    use crate::store::Store;

    #[test]
    fn setup_round_trips_and_empty_clears() {
        let store = Store::open_in_memory().unwrap();
        let project = ProjectId("p1".into());
        store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: project.clone(),
                name: "p".into(),
                repo_path: "/tmp/p".into(),
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        assert_eq!(store.project_setup(&project).unwrap(), "");
        store.set_project_setup(&project, "npm ci").unwrap();
        assert_eq!(store.project_setup(&project).unwrap(), "npm ci");
        store.set_project_setup(&project, "").unwrap();
        assert_eq!(store.project_setup(&project).unwrap(), "");
    }
}

#[cfg(test)]
mod create_tests {
    use std::sync::Arc;

    use berth_core::{EntityId, Project, ProjectId};

    use crate::hooks::HookEnv;
    use crate::registry::Daemon;
    use crate::store::Store;

    fn git_in(repo: &std::path::Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }

    #[tokio::test]
    async fn blank_branch_is_generated_and_setup_opens_a_terminal() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = berth_core::paths::canonical_or_raw(tmp.path()).join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);
        let daemon: Arc<Daemon> = Daemon::new(
            Arc::new(Store::open_in_memory().unwrap()),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        );
        let project = ProjectId("p".into());
        daemon
            .store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: project.clone(),
                name: "p".into(),
                repo_path: repo,
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        daemon.set_project_setup(&project, "echo ready").unwrap();

        let Ok(EntityId::Worktree(id)) = daemon.create_worktree(&project, "  ", None).await
        else {
            panic!("worktree not created");
        };

        let worktree = daemon.store.get_worktree(&id).unwrap().unwrap();
        assert_eq!(worktree.branch.split('-').count(), 3);
        let (_, _, _, terminals) = daemon.store.load_tree().unwrap();
        assert!(terminals
            .iter()
            .any(|t| t.worktree_id == id && t.name == super::SETUP_NAME));
    }
}
