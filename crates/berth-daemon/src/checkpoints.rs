use anyhow::{bail, Context, Result};
use berth_core::platform::NoWindow;
use berth_core::{AgentId, AgentKind, Checkpoint};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::registry::Daemon;

const REF_ROOT: &str = "refs/berth/checkpoints";
const KEEP_PER_AGENT: usize = 50;
const SUBJECT_PROMPT: &str = "berth checkpoint";
const SUBJECT_BEFORE_RESTORE: &str = "berth checkpoint before restore";

static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn git(repo: &Path, index: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .args(["-c", "user.name=berth", "-c", "user.email=berth@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let output = cmd.no_window().output().await.context("run git")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

struct TempIndex(PathBuf);

impl TempIndex {
    async fn seeded_from_worktree(repo: &Path) -> Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "berth-checkpoint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        let temp = TempIndex(path);
        let real = git(repo, None, &["rev-parse", "--git-path", "index"]).await?;
        let real = repo.join(real.trim());
        if real.is_file() {
            std::fs::copy(&real, &temp.0).context("copy the index")?;
        } else if head(repo).await?.is_some() {
            git(repo, Some(&temp.0), &["read-tree", "HEAD"]).await?;
        }
        Ok(temp)
    }

    fn empty() -> Self {
        TempIndex(std::env::temp_dir().join(format!(
            "berth-checkpoint-restore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        )))
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let mut lock = self.0.clone().into_os_string();
        lock.push(".lock");
        let _ = std::fs::remove_file(lock);
    }
}

async fn head(repo: &Path) -> Result<Option<String>> {
    Ok(git(repo, None, &["rev-parse", "-q", "--verify", "HEAD"])
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

fn ref_name(agent: &AgentId, number: u32) -> String {
    format!("{REF_ROOT}/{}/{number}", agent.as_str())
}

async fn listed(repo: &Path, agent: &AgentId) -> Result<Vec<Checkpoint>> {
    let prefix = format!("{REF_ROOT}/{}/", agent.as_str());
    let out = git(
        repo,
        None,
        &[
            "for-each-ref",
            "--format=%(refname)%09%(committerdate:unix)%09%(contents:subject)",
            &prefix,
        ],
    )
    .await?;
    let mut all: Vec<Checkpoint> = out
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let number = parts.next()?.strip_prefix(&prefix)?.parse().ok()?;
            let created_at = parts.next()?.parse().ok()?;
            let before_restore = parts.next()? == SUBJECT_BEFORE_RESTORE;
            Some(Checkpoint {
                number,
                created_at,
                before_restore,
            })
        })
        .collect();
    all.sort_by_key(|c| std::cmp::Reverse(c.number));
    Ok(all)
}

async fn take(repo: &Path, agent: &AgentId, before_restore: bool) -> Result<u32> {
    let temp = TempIndex::seeded_from_worktree(repo).await?;
    git(repo, Some(&temp.0), &["add", "-A"]).await?;
    let tree = git(repo, Some(&temp.0), &["write-tree"]).await?;
    let tree = tree.trim();
    let subject = if before_restore {
        SUBJECT_BEFORE_RESTORE
    } else {
        SUBJECT_PROMPT
    };
    let mut args = vec!["commit-tree", tree, "-m", subject];
    let parent = head(repo).await?;
    if let Some(parent) = &parent {
        args.extend(["-p", parent]);
    }
    let commit = git(repo, None, &args).await?;
    let existing = listed(repo, agent).await?;
    let number = existing.first().map_or(1, |c| c.number + 1);
    git(
        repo,
        None,
        &["update-ref", &ref_name(agent, number), commit.trim()],
    )
    .await?;
    for stale in existing.iter().skip(KEEP_PER_AGENT - 1) {
        git(
            repo,
            None,
            &["update-ref", "-d", &ref_name(agent, stale.number)],
        )
        .await?;
    }
    Ok(number)
}

async fn tracked_names(repo: &Path, rev: &str) -> Result<HashSet<String>> {
    let out = git(repo, None, &["ls-tree", "-r", "-z", "--name-only", rev]).await?;
    Ok(out
        .split('\0')
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect())
}

async fn restore(repo: &Path, agent: &AgentId, number: u32) -> Result<()> {
    let target = ref_name(agent, number);
    git(repo, None, &["rev-parse", "-q", "--verify", &target])
        .await
        .map_err(|_| anyhow::anyhow!("Checkpoint {number} no longer exists"))?;
    let safety = take(repo, agent, true).await?;
    let safety_ref = ref_name(agent, safety);
    let top = PathBuf::from(
        git(repo, None, &["rev-parse", "--show-toplevel"])
            .await?
            .trim(),
    );
    let temp = TempIndex::empty();
    git(&top, Some(&temp.0), &["read-tree", &target]).await?;
    git(&top, Some(&temp.0), &["checkout-index", "-a", "-f"]).await?;
    let keep = tracked_names(&top, &target).await?;
    for name in tracked_names(&top, &safety_ref).await? {
        if !keep.contains(&name) {
            match std::fs::remove_file(top.join(&name)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!("remove {name}")),
            }
        }
    }
    Ok(())
}

fn takes_checkpoints(kind: AgentKind, config: &crate::config::Config) -> bool {
    kind != AgentKind::Claude || config.checkpoints_claude
}

impl Daemon {
    fn checkpoint_dir(&self, agent: &AgentId) -> Result<PathBuf> {
        let agent = self.store.get_agent(agent)?.context("agent not found")?;
        let worktree = self
            .store
            .get_worktree(&agent.worktree_id)?
            .context("worktree not found")?;
        Ok(worktree.path)
    }

    pub async fn checkpoint_for_prompt(&self, agent: &AgentId) {
        let Ok(Some(row)) = self.store.get_agent(agent) else {
            return;
        };
        if !takes_checkpoints(row.kind, &crate::config::Config::load()) {
            return;
        }
        let Ok(dir) = self.checkpoint_dir(agent) else {
            return;
        };
        let _guard = SERIAL.lock().await;
        if let Err(e) = take(&dir, agent, false).await {
            tracing::debug!(agent = %agent, error = %e, "checkpoint skipped");
        }
    }

    pub async fn list_checkpoints(&self, agent: &AgentId) -> Result<Vec<Checkpoint>> {
        let dir = self.checkpoint_dir(agent)?;
        listed(&dir, agent).await
    }

    pub async fn restore_checkpoint(&self, agent: &AgentId, number: u32) -> Result<()> {
        let dir = self.checkpoint_dir(agent)?;
        let _guard = SERIAL.lock().await;
        restore(&dir, agent, number).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(repo: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn text(dir: &Path) -> String {
        let all = std::fs::read_to_string(dir.join("a.txt")).unwrap();
        all.lines().collect::<Vec<_>>().join("\n")
    }

    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), &["init", "-b", "main"]);
        std::fs::write(tmp.path().join(".gitignore"), "ignored.log\n").unwrap();
        std::fs::write(tmp.path().join("a.txt"), "one\n").unwrap();
        run(tmp.path(), &["add", "."]);
        run(tmp.path(), &["commit", "-m", "base"]);
        tmp
    }

    #[test]
    fn claude_takes_no_checkpoint_by_default_and_codex_does() {
        let config = crate::config::Config::default();
        assert!(!takes_checkpoints(AgentKind::Claude, &config));
        assert!(takes_checkpoints(AgentKind::Codex, &config));
        let on = crate::config::Config {
            checkpoints_claude: true,
            ..config
        };
        assert!(takes_checkpoints(AgentKind::Claude, &on));
    }

    #[tokio::test]
    async fn a_checkpoint_leaves_head_branch_index_and_stash_alone() {
        let tmp = repo();
        let dir = tmp.path();
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        std::fs::write(dir.join("new.txt"), "fresh\n").unwrap();
        run(dir, &["add", "new.txt"]);
        std::fs::write(dir.join("ignored.log"), "noise\n").unwrap();
        let head = run(dir, &["rev-parse", "HEAD"]);
        let status = run(dir, &["status", "--porcelain=v1"]);
        let agent = AgentId("ag1".into());

        assert_eq!(take(dir, &agent, false).await.unwrap(), 1);
        assert_eq!(take(dir, &agent, false).await.unwrap(), 2);

        assert_eq!(run(dir, &["rev-parse", "HEAD"]), head);
        assert_eq!(run(dir, &["status", "--porcelain=v1"]), status);
        assert_eq!(run(dir, &["stash", "list"]), "");
        assert_eq!(run(dir, &["branch", "--list"]).trim(), "* main");
        let files = run(dir, &["ls-tree", "-r", "--name-only", &ref_name(&agent, 1)]);
        assert!(
            files.contains("a.txt") && files.contains("new.txt"),
            "{files}"
        );
        assert!(!files.contains("ignored.log"), "{files}");
        let shown = run(dir, &["show", &format!("{}:a.txt", ref_name(&agent, 1))]);
        assert_eq!(shown, "two\n");

        let list = listed(dir, &agent).await.unwrap();
        assert_eq!(
            list.iter().map(|c| c.number).collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert!(list.iter().all(|c| !c.before_restore));
    }

    #[tokio::test]
    async fn restore_brings_files_back_and_keeps_what_it_replaced() {
        let tmp = repo();
        let dir = tmp.path();
        let agent = AgentId("ag1".into());
        std::fs::write(dir.join("a.txt"), "v1\n").unwrap();
        take(dir, &agent, false).await.unwrap();

        std::fs::write(dir.join("a.txt"), "v2 uncommitted\n").unwrap();
        std::fs::write(dir.join("later.txt"), "created after\n").unwrap();
        let head = run(dir, &["rev-parse", "HEAD"]);

        restore(dir, &agent, 1).await.unwrap();

        assert_eq!(text(dir), "v1");
        assert!(!dir.join("later.txt").exists());
        assert_eq!(run(dir, &["rev-parse", "HEAD"]), head);
        assert_eq!(run(dir, &["branch", "--list"]).trim(), "* main");

        let list = listed(dir, &agent).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].before_restore && list[0].number == 2);
        let saved = run(dir, &["show", &format!("{}:a.txt", ref_name(&agent, 2))]);
        assert_eq!(saved, "v2 uncommitted\n");
        let saved = run(
            dir,
            &["show", &format!("{}:later.txt", ref_name(&agent, 2))],
        );
        assert_eq!(saved, "created after\n");

        restore(dir, &agent, 2).await.unwrap();
        assert_eq!(text(dir), "v2 uncommitted");
        assert!(dir.join("later.txt").exists());
    }

    #[tokio::test]
    async fn restoring_a_missing_checkpoint_changes_nothing() {
        let tmp = repo();
        let dir = tmp.path();
        let agent = AgentId("ag1".into());
        std::fs::write(dir.join("a.txt"), "keep\n").unwrap();
        assert!(restore(dir, &agent, 9).await.is_err());
        assert_eq!(text(dir), "keep");
        assert!(listed(dir, &agent).await.unwrap().is_empty());
    }
}
