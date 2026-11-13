use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

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
            format!(
                "git {} не ответил за {} с",
                args.join(" "),
                STEP_TIMEOUT.as_secs()
            )
        })?
        .map_err(|error| format!("не удалось запустить git: {error}"))?;
    let bytes = if output.status.success() {
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

async fn default_branch(path: &str) -> Option<String> {
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
    let branch = line(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await;
    let base = default_branch(&path).await;
    let dirty = line(&path, &["status", "--porcelain"]).await.is_some();
    let upstream = line(&path, &UPSTREAM).await.is_some();
    let (mut behind, mut ahead) = (0, 0);
    if upstream {
        if let Some(pair) = line(
            &path,
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
                &path,
                &["rev-list", "--count", &format!("origin/{base}..HEAD")],
            )
            .await
        }
        None => 0,
    };
    Ok(GitState {
        branch,
        base,
        dirty,
        upstream,
        ahead,
        behind,
        ahead_of_base,
    })
}

async fn run(path: &str, args: &[&str]) -> Result<(), String> {
    match git(path, args).await? {
        (true, _) => Ok(()),
        (false, error) => Err(format!("git {}: {error}", args.first().unwrap_or(&""))),
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
                return Err("пустое сообщение коммита".to_owned());
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
        other => Err(format!("неизвестный шаг git: {other}")),
    }
}
