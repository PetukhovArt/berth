//! `berth enter <dir>` from inside an agent session: the session moves into
//! another project's checkout the way `berth worktree` moves it into a
//! sibling worktree — the row at once, the live CLI at its turn end. A repo
//! the agent's workspace does not hold yet is added to it first.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use berth_core::{paths, Agent, AgentId, AgentKind, EnterOutcome, WorkspaceId, Worktree};

use crate::git;
use crate::registry::Daemon;

/// What berth appends to Claude's system prompt so "go to that project" is
/// one `berth enter` call. Claude only, like the worktree guidance.
pub const CLAUDE_ENTER_GUIDANCE: &str = "[berth] When the user asks you to move this session to \
another project or repository (\"go there\", \"switch to that project\", \"continue in <dir>\"), \
run this shell command exactly once:\n\n  berth enter <dir>\n\nwhere <dir> is that repository's \
directory. berth adds the project when it is not in berth yet, associates this session with it, \
and relocates the session there once your current turn ends, with the conversation resumed. So \
when the command succeeds, end your turn at once: tell the user in one line that the session is \
moving, and make no further tool calls or edits. If the command fails, report the error and stay \
where you are.";

impl Daemon {
    pub async fn enter_project(
        self: &Arc<Self>,
        id: &AgentId,
        path: &Path,
    ) -> Result<(Worktree, EnterOutcome)> {
        let agent = self.store.get_agent(id)?.context("agent not found")?;
        if agent.archived {
            bail!("agent is archived");
        }
        let current = self
            .store
            .get_worktree(&agent.worktree_id)?
            .context("worktree not found")?;
        let workspace = self
            .store
            .get_project(&current.project_id)?
            .context("project not found")?
            .workspace_id;
        let checkout = git::repo_toplevel(path)
            .await
            .with_context(|| format!("{} is not a git repository", path.display()))?;
        let checkout = paths::canonical_or_raw(&checkout);
        let target = match self.checkout_row(&checkout, &workspace)? {
            Some(row) => row,
            None => {
                self.add_project(path, None, false, Some(workspace.clone()))
                    .await?;
                self.checkout_row(&checkout, &workspace)?
                    .with_context(|| format!("berth has no row for {}", checkout.display()))?
            }
        };
        if target.id == current.id {
            return Ok((target, EnterOutcome::AlreadyThere));
        }
        let outcome = self.relocate_agent(id, &target)?;
        if outcome == EnterOutcome::NextLaunch {
            self.carry_claude_transcript(&agent, &target);
        }
        Ok((target, outcome))
    }

    /// The worktree row at `checkout` under a project of `workspace`.
    fn checkout_row(&self, checkout: &Path, workspace: &WorkspaceId) -> Result<Option<Worktree>> {
        let (projects, worktrees, _, _) = self.store.load_tree()?;
        Ok(worktrees.into_iter().find(|w| {
            paths::canonical_or_raw(&w.path) == checkout
                && projects
                    .iter()
                    .any(|p| p.id == w.project_id && &p.workspace_id == workspace)
        }))
    }

    /// Bring a stopped Claude session's transcript to where `claude
    /// --resume` in `target` looks for it. A move between worktrees of one
    /// project finds it already in reach and leaves it alone.
    pub(crate) fn carry_claude_transcript(&self, agent: &Agent, target: &Worktree) {
        if agent.kind != AgentKind::Claude {
            return;
        }
        let Some(session_id) = agent.session_id.as_deref() else {
            return;
        };
        let checkouts: Vec<PathBuf> = match self.store.load_tree() {
            Ok((_, worktrees, _, _)) => worktrees
                .into_iter()
                .filter(|w| w.project_id == target.project_id)
                .map(|w| w.path)
                .collect(),
            Err(e) => {
                tracing::warn!(agent = %agent.id, error = %e, "transcript carry skipped");
                return;
            }
        };
        if let Err(e) = crate::orphans::carry_transcript(session_id, &checkouts, &target.path) {
            tracing::warn!(agent = %agent.id, error = %e, "transcript carry failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::HookEnv;
    use crate::store::Store;
    use berth_core::{AgentStatus, EntityId};

    fn git_init(repo: &Path) {
        std::fs::create_dir_all(repo).unwrap();
        for args in [
            &["init", "-b", "main"][..],
            &["commit", "--allow-empty", "-m", "init"][..],
        ] {
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
            assert!(out.status.success(), "git {args:?} failed");
        }
    }

    #[tokio::test]
    async fn enter_adds_the_repo_to_the_agents_workspace_and_moves_the_row() {
        let tmp = tempfile::tempdir().unwrap();
        let root = paths::canonical_or_raw(tmp.path());
        let (here, there) = (root.join("here"), root.join("there"));
        git_init(&here);
        git_init(&there);
        let daemon = Daemon::new(
            Arc::new(Store::open_in_memory().unwrap()),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        );
        let EntityId::Project(home) = daemon.add_project(&here, None, false, None).await.unwrap()
        else {
            panic!("expected a project id");
        };
        let (_, worktrees, _, _) = daemon.store.load_tree().unwrap();
        let start = worktrees.iter().find(|w| w.project_id == home).unwrap();
        let id = AgentId("a1".into());
        daemon
            .store
            .insert_agent(&Agent {
                id: id.clone(),
                worktree_id: start.id.clone(),
                name: "a1".into(),
                status: AgentStatus::Running,
                archived: false,
                archived_at: 0,
                unseen: false,
                kind: AgentKind::Claude,
                model: None,
                effort: None,
                session_id: None,
                cloud_session_id: None,
                sort_order: 0,
                status_changed_at: 0,
                alive: false,
                cloud_mirroring: false,
                suggested_prompt: None,
            })
            .unwrap();

        let (target, outcome) = daemon.enter_project(&id, &there).await.unwrap();
        assert_eq!(outcome, EnterOutcome::NextLaunch);
        assert_eq!(paths::canonical_or_raw(&target.path), there);
        let project = daemon
            .store
            .get_project(&target.project_id)
            .unwrap()
            .unwrap();
        assert_ne!(project.id, home, "the other repo became its own project");
        let agent = daemon.store.get_agent(&id).unwrap().unwrap();
        assert_eq!(agent.worktree_id, target.id);

        let (again, outcome) = daemon.enter_project(&id, &there).await.unwrap();
        assert_eq!(again.id, target.id, "no second project for the same repo");
        assert_eq!(outcome, EnterOutcome::AlreadyThere);
    }
}
