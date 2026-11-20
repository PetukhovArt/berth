use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use berth_core::daemon_defaults::daemon_defaults;
use serde::Serialize;
use tokio::process::Command;

const STEP_TIMEOUT: Duration = Duration::from_secs(120);
const UPSTREAM: [&str; 4] = ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"];

#[derive(Debug, Serialize)]
pub struct GitState {
    branch: Option<String>,
    base: Option<String>,
    dirty: bool,
    upstream: bool,
    remote: bool,
    ahead: u32,
    behind: u32,
    ahead_of_base: u32,
}

async fn git(path: &str, args: &[&str]) -> Result<(bool, String), String> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(Path::new(path))
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let run = async {
        crate::platform::spawn_tied(&mut command)?
            .wait_with_output()
            .await
    };
    let output = tokio::time::timeout(STEP_TIMEOUT, run)
        .await
        .map_err(|_| {
            crate::host_text!(
                "gitTimeout",
                command = args.join(" "),
                seconds = STEP_TIMEOUT.as_secs()
            )
        })?
        .map_err(|error| crate::host_text!("launchFailed", program = "git", error = error))?;
    let bytes = if output.status.success() || output.stderr.trim_ascii().is_empty() {
        output.stdout
    } else {
        output.stderr
    };
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&bytes).trim().to_owned(),
    ))
}

async fn line(path: &str, args: &[&str]) -> Option<String> {
    match git(path, args).await {
        Ok((true, text)) if !text.is_empty() => Some(text),
        _ => None,
    }
}

async fn count(path: &str, args: &[&str]) -> u32 {
    line(path, args)
        .await
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}

struct Base {
    name: String,
    rev: String,
}

async fn base_branch(path: &str, configured: &BTreeMap<String, String>) -> Option<Base> {
    if let Some(name) = origin_default_branch(path).await {
        let rev = format!("origin/{name}");
        return Some(Base { name, rev });
    }
    let name = local_default_branch(path, configured).await?;
    let rev = format!("refs/heads/{name}");
    Some(Base { name, rev })
}

async fn has_local_branch(path: &str, name: &str) -> bool {
    let spec = format!("refs/heads/{name}");
    line(path, &["rev-parse", "--verify", "--quiet", &spec])
        .await
        .is_some()
}

async fn repo_folder(path: &str) -> Option<String> {
    let common = line(
        path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await?;
    let common = Path::new(&common);
    let repo = if common.file_name().is_some_and(|name| name == ".git") {
        common.parent()?
    } else {
        common
    };
    Some(repo.file_name()?.to_string_lossy().into_owned())
}

async fn local_default_branch(path: &str, configured: &BTreeMap<String, String>) -> Option<String> {
    let named = match repo_folder(path).await {
        Some(folder) => configured
            .get(&folder)
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty()),
        None => None,
    };
    for candidate in named
        .into_iter()
        .chain(daemon_defaults().worktree_default_bases.iter().cloned())
    {
        if has_local_branch(path, &candidate).await {
            return Some(candidate);
        }
    }
    None
}

async fn origin_default_branch(path: &str) -> Option<String> {
    if let Some(remote) = line(
        path,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .await
    {
        return remote.strip_prefix("origin/").map(str::to_owned);
    }
    for candidate in ["main", "master"] {
        let spec = format!("refs/remotes/origin/{candidate}");
        if line(path, &["rev-parse", "--verify", "--quiet", &spec])
            .await
            .is_some()
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

pub async fn worktree_git_state(path: String) -> Result<GitState, String> {
    let configured = crate::settings::berth_worktree_base_read()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| daemon_defaults().worktree_base.clone());
    git_state(&path, &configured).await
}

async fn git_state(path: &str, configured: &BTreeMap<String, String>) -> Result<GitState, String> {
    let branch = line(path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await;
    let base = base_branch(path, configured).await;
    let dirty = line(path, &["status", "--porcelain"]).await.is_some();
    let upstream = line(path, &UPSTREAM).await.is_some();
    let remote = upstream || line(path, &["remote", "get-url", "origin"]).await.is_some();
    let (mut behind, mut ahead) = (0, 0);
    if upstream {
        if let Some(pair) = line(
            path,
            &["rev-list", "--left-right", "--count", "@{u}...HEAD"],
        )
        .await
        {
            let mut parts = pair.split_whitespace().map(|n| n.parse().unwrap_or(0));
            behind = parts.next().unwrap_or(0);
            ahead = parts.next().unwrap_or(0);
        }
    }
    let ahead_of_base = match &base {
        Some(base) => {
            count(
                path,
                &["rev-list", "--count", &format!("{}..HEAD", base.rev)],
            )
            .await
        }
        None => 0,
    };
    Ok(GitState {
        branch,
        base: base.map(|base| base.name),
        dirty,
        upstream,
        remote,
        ahead,
        behind,
        ahead_of_base,
    })
}

async fn run(path: &str, args: &[&str]) -> Result<(), String> {
    match git(path, args).await? {
        (true, _) => Ok(()),
        (false, error) => Err(format!(
            "git {}: {}",
            args.first().unwrap_or(&""),
            failure_cause(&error)
        )),
    }
}

fn failure_cause(output: &str) -> String {
    let causes: Vec<&str> = output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("fatal:")
                .or_else(|| line.strip_prefix("error:"))
                .map(str::trim)
        })
        .filter(|cause| !cause.is_empty())
        .collect();
    if causes.is_empty() {
        output.to_owned()
    } else {
        causes.join("\n")
    }
}

pub async fn worktree_git_step(
    path: String,
    step: String,
    message: Option<String>,
) -> Result<(), String> {
    match step.as_str() {
        "commit" => {
            let message = message.unwrap_or_default();
            if message.trim().is_empty() {
                return Err(crate::host_text!("emptyCommitMessage"));
            }
            run(&path, &["add", "-A"]).await?;
            run(&path, &["commit", "-m", message.trim()]).await
        }
        "push" => {
            if line(&path, &UPSTREAM).await.is_some() {
                run(&path, &["push"]).await
            } else {
                run(&path, &["push", "-u", "origin", "HEAD"]).await
            }
        }
        "pull" => run(&path, &["pull", "--ff-only"]).await,
        other => Err(crate::host_text!("unknownGitStep", step = other)),
    }
}
