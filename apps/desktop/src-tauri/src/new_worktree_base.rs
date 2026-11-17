use serde::Serialize;

use crate::mr_checkout::{git, mr_fetch_source, FetchProblem, QUICK_TIMEOUT};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WorktreeBase {
    pub base: Option<String>,
    pub local_copy: Option<String>,
    pub detail: Option<String>,
}

async fn ref_sha(repo: &str, name: &str) -> Result<Option<String>, String> {
    let run = git(
        repo,
        &["rev-parse", "--verify", "--quiet", name],
        QUICK_TIMEOUT,
    )
    .await?;
    Ok(run.ok.then_some(run.stdout))
}

async fn local_copy(repo: &str, branch: &str) -> Result<Option<(String, String)>, String> {
    let head = format!("refs/heads/{branch}");
    let tracking = format!("refs/remotes/origin/{branch}");
    let local = ref_sha(repo, &head).await?;
    let remote = ref_sha(repo, &tracking).await?;
    let tracking_is_newer = match (&local, &remote) {
        (Some(a), Some(b)) if a != b => {
            git(
                repo,
                &["merge-base", "--is-ancestor", &head, &tracking],
                QUICK_TIMEOUT,
            )
            .await?
            .ok
        }
        (None, Some(_)) => true,
        _ => false,
    };
    Ok(if tracking_is_newer {
        Some((format!("{tracking}^0"), format!("origin/{branch}")))
    } else {
        local.map(|_| (branch.to_owned(), branch.to_owned()))
    })
}

fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn worktree_base_source(repo: String, branch: String) -> Result<WorktreeBase, String> {
    let fetched = mr_fetch_source(repo.clone(), branch.clone(), String::new(), None).await?;
    if let Some(base) = fetched.base {
        return Ok(WorktreeBase {
            base: Some(format!("{base}^0")),
            local_copy: None,
            detail: None,
        });
    }
    let detail = fetched.detail.as_deref().map(one_line);
    let has_local = ref_sha(&repo, &format!("refs/heads/{branch}"))
        .await?
        .is_some();
    match fetched.problem {
        Some(FetchProblem::NoRemote) => Ok(WorktreeBase {
            base: Some(branch),
            local_copy: None,
            detail: None,
        }),
        Some(FetchProblem::BranchMissing) if has_local => Ok(WorktreeBase {
            base: Some(branch),
            local_copy: None,
            detail: None,
        }),
        Some(FetchProblem::BranchMissing) => Ok(WorktreeBase {
            base: None,
            local_copy: None,
            detail,
        }),
        _ => Ok(match local_copy(&repo, &branch).await? {
            Some((start, shown_as)) => WorktreeBase {
                base: Some(start),
                local_copy: Some(shown_as),
                detail,
            },
            None => WorktreeBase {
                base: None,
                local_copy: None,
                detail,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn git_in(repo: &Path, args: &[&str]) -> String {
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
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn clone_with_unreachable_origin(root: &Path, fetch_ahead: bool) -> (String, String) {
        let upstream = root.join("upstream");
        std::fs::create_dir(&upstream).unwrap();
        git_in(&upstream, &["init", "-b", "main"]);
        git_in(&upstream, &["commit", "--allow-empty", "-m", "a"]);
        git_in(root, &["clone", "-q", &upstream.to_string_lossy(), "repo"]);
        let repo = root.join("repo");
        if fetch_ahead {
            git_in(&upstream, &["commit", "--allow-empty", "-m", "b"]);
            git_in(&repo, &["fetch", "-q", "origin"]);
        }
        let gone = root.join("gone-host").to_string_lossy().into_owned();
        git_in(&repo, &["remote", "set-url", "origin", &gone]);
        (
            repo.to_string_lossy().into_owned(),
            git_in(&upstream, &["rev-parse", "HEAD"]),
        )
    }

    #[tokio::test]
    async fn a_failed_fetch_falls_back_to_the_local_branch_with_gits_message() {
        let tmp = tempfile::tempdir().unwrap();
        let (repo, _) = clone_with_unreachable_origin(tmp.path(), false);
        let chosen = worktree_base_source(repo, "main".to_owned()).await.unwrap();
        assert_eq!(chosen.base.as_deref(), Some("main"));
        assert_eq!(chosen.local_copy.as_deref(), Some("main"));
        assert!(chosen.detail.is_some_and(|detail| !detail.is_empty()));
    }

    #[tokio::test]
    async fn a_failed_fetch_takes_a_tracking_ref_ahead_of_the_local_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let (repo, upstream_head) = clone_with_unreachable_origin(tmp.path(), true);
        let chosen = worktree_base_source(repo.clone(), "main".to_owned())
            .await
            .unwrap();
        let base = chosen.base.unwrap();
        assert_eq!(base, "refs/remotes/origin/main^0");
        assert_eq!(chosen.local_copy.as_deref(), Some("origin/main"));
        assert_eq!(
            git_in(Path::new(&repo), &["rev-parse", &base]),
            upstream_head
        );
    }
}
