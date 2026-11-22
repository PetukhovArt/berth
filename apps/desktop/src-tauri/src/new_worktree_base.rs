use std::sync::LazyLock;
use std::time::Duration;

use berth_core::fetch_gate::FetchGate;
use serde::Serialize;

use crate::mr_checkout::{git, mr_fetch_source, FetchProblem, Fetched, QUICK_TIMEOUT};

const BASE_FRESH_FOR: Duration = Duration::from_secs(120);

static BASE_FETCHES: LazyLock<FetchGate> = LazyLock::new(|| FetchGate::new(BASE_FRESH_FOR));

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

async fn fetch_base(repo: &str, branch: &str) -> Result<Fetched, String> {
    let ran = BASE_FETCHES
        .fetch_unless_fresh(repo, branch, || async {
            let result =
                mr_fetch_source(repo.to_owned(), branch.to_owned(), String::new(), None).await;
            let fetched = matches!(&result, Ok(fetched) if fetched.problem.is_none());
            (result, fetched)
        })
        .await;
    ran.unwrap_or_else(|| {
        Ok(Fetched {
            remote: "origin".to_owned(),
            base: Some(format!("refs/remotes/origin/{branch}")),
            problem: None,
            detail: None,
            diverged: false,
        })
    })
}

pub async fn worktree_base_prefetch(repo: String, branch: String) -> Result<(), String> {
    tokio::spawn(async move {
        let _ = fetch_base(&repo, &branch).await;
    });
    Ok(())
}

pub async fn worktree_base_source(repo: String, branch: String) -> Result<WorktreeBase, String> {
    let fetched = fetch_base(&repo, &branch).await?;
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
