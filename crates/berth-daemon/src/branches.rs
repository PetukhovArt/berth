//! `ListBranches`: a project's local branches for a client's branch picker,
//! each tied to the worktree row that has it checked out.

use std::sync::Arc;

use anyhow::{Context, Result};
use berth_core::{Branch, ProjectId, Worktree};

use crate::git;
use crate::registry::Daemon;

/// Pair each branch with the row holding it. Rows come from the store after
/// a reconcile, so their `branch` is git's own name for the checkout.
pub(crate) fn attach_worktrees(names: Vec<String>, rows: &[Worktree]) -> Vec<Branch> {
    names
        .into_iter()
        .map(|name| {
            let worktree = rows.iter().find(|w| w.branch == name).map(|w| w.id.clone());
            Branch { name, worktree }
        })
        .collect()
}

impl Daemon {
    pub async fn list_branches(self: &Arc<Self>, project_id: &ProjectId) -> Result<Vec<Branch>> {
        let project = self
            .store
            .get_project(project_id)?
            .context("project not found")?;
        // Reconcile first: a checkout made outside berth gets its row now,
        // so every worktree named below is one the client can target.
        self.sync_project_worktrees(&project).await?;
        let names = git::list_branches(&project.repo_path).await?;
        let (_, worktrees, _, _) = self.store.load_tree()?;
        let rows: Vec<Worktree> = worktrees
            .into_iter()
            .filter(|w| w.project_id == project.id)
            .collect();
        Ok(attach_worktrees(names, &rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_core::WorktreeId;
    use std::path::PathBuf;

    fn row(branch: &str, is_main: bool) -> Worktree {
        Worktree {
            id: WorktreeId::generate(),
            project_id: ProjectId::generate(),
            path: PathBuf::from(format!("/repo-{branch}")),
            branch: branch.into(),
            is_main,
            sort_order: 0,
        }
    }

    #[test]
    fn branches_name_the_row_that_checks_them_out() {
        let main = row("main", true);
        let topic = row("feat/x", false);
        let rows = vec![main.clone(), topic.clone()];
        let got = attach_worktrees(vec!["feat/x".into(), "free".into(), "main".into()], &rows);
        assert_eq!(
            got,
            vec![
                Branch {
                    name: "feat/x".into(),
                    worktree: Some(topic.id)
                },
                Branch {
                    name: "free".into(),
                    worktree: None
                },
                Branch {
                    name: "main".into(),
                    worktree: Some(main.id)
                },
            ]
        );
    }
}
