use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use berth_core::daemon_defaults::daemon_defaults;
use berth_core::entities::Project;
use berth_core::{EntityId, ProjectId, Worktree};

use crate::git;
use crate::registry::Daemon;

const FETCH_LIMIT: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq, Eq)]
pub struct ChosenBase {
    pub base: Option<String>,
    pub warning: Option<String>,
}

impl ChosenBase {
    fn quiet(base: Option<String>) -> Self {
        Self {
            base,
            warning: None,
        }
    }
}

pub async fn choose(
    explicit: Option<&str>,
    project: &Project,
    branch: &str,
    configured: &BTreeMap<String, String>,
) -> ChosenBase {
    if let Some(base) = explicit {
        return ChosenBase::quiet(Some(base.to_owned()));
    }
    let repo = &project.repo_path;
    let local = git::list_branches(repo).await.unwrap_or_default();
    if local.iter().any(|name| name == branch) {
        return ChosenBase::quiet(None);
    }
    let Some(base) = base_branch(project, configured, &local) else {
        return ChosenBase::quiet(None);
    };
    if !git::has_remote(repo, "origin").await {
        return ChosenBase::quiet(Some(base));
    }
    let error = match git::fetch_origin_branch(repo, &base, FETCH_LIMIT).await {
        Ok(remote) => return ChosenBase::quiet(Some(format!("{remote}^0"))),
        Err(error) => one_line(&error.to_string()),
    };
    let missing_on_origin = error.contains("couldn't find remote ref");
    if missing_on_origin && local.contains(&base) {
        return ChosenBase::quiet(Some(base));
    }
    match git::local_copy_of_origin_branch(repo, &base).await {
        Some(copy) => ChosenBase {
            base: Some(copy.start),
            warning: Some(format!(
                "created from the local {} because fetching {base} from origin failed: {error}",
                copy.shown_as
            )),
        },
        None => ChosenBase::quiet(Some(base)),
    }
}

fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn base_branch(
    project: &Project,
    configured: &BTreeMap<String, String>,
    local: &[String],
) -> Option<String> {
    let folder = project
        .repo_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let named = configured
        .get(&project.name)
        .or_else(|| configured.get(&folder))
        .map(|name| name.trim())
        .filter(|name| !name.is_empty());
    if let Some(name) = named {
        return Some(name.to_owned());
    }
    daemon_defaults()
        .worktree_default_bases
        .iter()
        .find(|name| local.contains(name))
        .cloned()
}

impl Daemon {
    pub(crate) async fn create_worktree_from_base(
        self: &Arc<Self>,
        project_id: &ProjectId,
        branch: &str,
        base: Option<&str>,
    ) -> Result<(Worktree, Option<String>)> {
        let project = self
            .store
            .get_project(project_id)?
            .context("project not found")?;
        let configured = crate::config::Config::load().worktree_base;
        let chosen = choose(base, &project, branch, &configured).await;
        let created = self
            .create_worktree(project_id, branch, chosen.base.as_deref())
            .await?;
        let EntityId::Worktree(id) = created else {
            bail!("worktree creation returned a non-worktree entity");
        };
        let worktree = self
            .store
            .get_worktree(&id)?
            .context("worktree not found")?;
        Ok((worktree, chosen.warning))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_core::ids::ProjectId;
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

    fn project(repo: &Path) -> Project {
        Project {
            workspace_id: Default::default(),
            id: ProjectId("p".into()),
            name: "web-client".into(),
            repo_path: repo.to_path_buf(),
            sort_order: 0,
            pin_order: None,
        }
    }

    fn repo_on_topic(root: &Path) -> std::path::PathBuf {
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "a"]);
        git_in(&repo, &["branch", "release"]);
        git_in(&repo, &["checkout", "-b", "topic"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "b"]);
        repo
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn clone_of_upstream(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let upstream = root.join("upstream");
        std::fs::create_dir(&upstream).unwrap();
        git_in(&upstream, &["init", "-b", "main"]);
        git_in(&upstream, &["commit", "--allow-empty", "-m", "a"]);
        git_in(root, &["clone", "-q", &upstream.to_string_lossy(), "repo"]);
        (upstream, root.join("repo"))
    }

    fn unreachable_origin(root: &Path, repo: &Path) {
        let gone = root.join("gone-host").to_string_lossy().into_owned();
        git_in(repo, &["remote", "set-url", "origin", &gone]);
    }

    #[tokio::test]
    async fn unset_starts_from_the_first_default_branch_not_head() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = repo_on_topic(tmp.path());
        let chosen = choose(None, &project(&repo), "feat", &BTreeMap::new()).await;
        assert_eq!(chosen, ChosenBase::quiet(Some("main".into())));
    }

    #[tokio::test]
    async fn the_setting_names_the_branch_by_project_then_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = repo_on_topic(tmp.path());
        let p = project(&repo);
        let by_folder = choose(None, &p, "feat", &map(&[("repo", "release")])).await;
        assert_eq!(by_folder.base.as_deref(), Some("release"));
        let both = map(&[("repo", "main"), ("web-client", " release ")]);
        assert_eq!(
            choose(None, &p, "feat", &both).await.base.as_deref(),
            Some("release")
        );
    }

    #[tokio::test]
    async fn an_explicit_base_wins_and_an_existing_branch_needs_none() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = repo_on_topic(tmp.path());
        let p = project(&repo);
        let set = map(&[("web-client", "release")]);
        assert_eq!(
            choose(Some("topic"), &p, "feat", &set)
                .await
                .base
                .as_deref(),
            Some("topic")
        );
        assert_eq!(choose(None, &p, "release", &set).await.base, None);
    }

    #[tokio::test]
    async fn a_base_on_origin_is_fetched_first() {
        let tmp = tempfile::tempdir().unwrap();
        let (upstream, repo) = clone_of_upstream(tmp.path());
        git_in(&upstream, &["commit", "--allow-empty", "-m", "b"]);

        let chosen = choose(None, &project(&repo), "feat", &BTreeMap::new()).await;
        assert_eq!(chosen.warning, None);
        let base = chosen.base.unwrap();
        assert_eq!(base, "refs/remotes/origin/main^0");
        assert_eq!(
            git_in(&repo, &["rev-parse", &base]),
            git_in(&upstream, &["rev-parse", "HEAD"])
        );
    }

    #[tokio::test]
    async fn an_unreachable_origin_branches_from_the_local_copy_with_a_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let (_upstream, repo) = clone_of_upstream(tmp.path());
        unreachable_origin(tmp.path(), &repo);

        let chosen = choose(None, &project(&repo), "feat", &BTreeMap::new()).await;
        assert_eq!(chosen.base.as_deref(), Some("main"));
        let warning = chosen.warning.unwrap();
        assert!(
            warning.starts_with(
                "created from the local main because fetching main from origin failed: "
            ),
            "{warning}"
        );
        assert!(!warning.ends_with("failed: "), "{warning}");
    }

    #[tokio::test]
    async fn an_unreachable_origin_prefers_a_tracking_ref_ahead_of_the_local_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let (upstream, repo) = clone_of_upstream(tmp.path());
        git_in(&upstream, &["commit", "--allow-empty", "-m", "b"]);
        git_in(&repo, &["fetch", "-q", "origin"]);
        unreachable_origin(tmp.path(), &repo);

        let chosen = choose(None, &project(&repo), "feat", &BTreeMap::new()).await;
        let base = chosen.base.unwrap();
        assert_eq!(base, "refs/remotes/origin/main^0");
        assert_eq!(
            git_in(&repo, &["rev-parse", &base]),
            git_in(&upstream, &["rev-parse", "HEAD"])
        );
        assert!(chosen
            .warning
            .unwrap()
            .starts_with("created from the local origin/main because"));
    }
}
