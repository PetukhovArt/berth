use std::collections::BTreeMap;
use std::time::Duration;

use berth_core::daemon_defaults::daemon_defaults;
use berth_core::entities::Project;

use crate::git;

const FETCH_LIMIT: Duration = Duration::from_secs(30);

pub async fn choose(
    explicit: Option<&str>,
    project: &Project,
    branch: &str,
    configured: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(base) = explicit {
        return Some(base.to_owned());
    }
    let local = git::list_branches(&project.repo_path)
        .await
        .unwrap_or_default();
    if local.iter().any(|name| name == branch) {
        return None;
    }
    let base = base_branch(project, configured, &local)?;
    match git::fetch_origin_branch(&project.repo_path, &base, FETCH_LIMIT).await {
        Ok(remote) => Some(format!("{remote}^0")),
        Err(error) => {
            tracing::info!(%base, %error, "worktree base not fetched, branching from the local one");
            Some(base)
        }
    }
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

    #[tokio::test]
    async fn unset_starts_from_the_first_default_branch_not_head() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = repo_on_topic(tmp.path());
        let base = choose(None, &project(&repo), "feat", &BTreeMap::new()).await;
        assert_eq!(base.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn the_setting_names_the_branch_by_project_then_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = repo_on_topic(tmp.path());
        let p = project(&repo);
        let by_folder = choose(None, &p, "feat", &map(&[("repo", "release")])).await;
        assert_eq!(by_folder.as_deref(), Some("release"));
        let both = map(&[("repo", "main"), ("web-client", " release ")]);
        assert_eq!(
            choose(None, &p, "feat", &both).await.as_deref(),
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
            choose(Some("topic"), &p, "feat", &set).await.as_deref(),
            Some("topic")
        );
        assert_eq!(choose(None, &p, "release", &set).await, None);
    }

    #[tokio::test]
    async fn a_base_on_origin_is_fetched_first() {
        let tmp = tempfile::tempdir().unwrap();
        let upstream = tmp.path().join("upstream");
        std::fs::create_dir(&upstream).unwrap();
        git_in(&upstream, &["init", "-b", "main"]);
        git_in(&upstream, &["commit", "--allow-empty", "-m", "a"]);
        git_in(
            tmp.path(),
            &["clone", "-q", &upstream.to_string_lossy(), "repo"],
        );
        git_in(&upstream, &["commit", "--allow-empty", "-m", "b"]);
        let repo = tmp.path().join("repo");

        let base = choose(None, &project(&repo), "feat", &BTreeMap::new())
            .await
            .unwrap();
        assert_eq!(base, "refs/remotes/origin/main^0");
        assert_eq!(
            git_in(&repo, &["rev-parse", &base]),
            git_in(&upstream, &["rev-parse", "HEAD"])
        );
    }
}
