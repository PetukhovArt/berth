mod acp;
mod apply;
mod hooks;
mod mcp;
mod plan;
mod settings;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use berth_core::platform::NoWindow;
use berth_core::{Project, Worktree};
use serde_json::Value;

use crate::store::Store;

pub use apply::Report;
pub use plan::{SetupFile, SETUP_FILE};

#[derive(Debug, Default)]
pub struct SharedSetup {
    root: PathBuf,
    file: SetupFile,
}

enum Source {
    LocalDir(PathBuf),
    Repository { url: String, clone: PathBuf },
}

impl Source {
    fn from_config() -> Option<Self> {
        let value = crate::config::Config::load().shared_setup?;
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        if value.contains("://") || value.starts_with("git@") {
            return Some(Source::Repository {
                url: value.to_string(),
                clone: berth_core::paths::data_dir()
                    .join("shared-setup")
                    .join(apply::fingerprint(value.as_bytes())),
            });
        }
        let dir = match (value.strip_prefix("~/"), berth_core::env::home_dir()) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => PathBuf::from(value),
        };
        Some(Source::LocalDir(dir))
    }
}

impl SharedSetup {
    pub fn load() -> Result<Option<Self>> {
        match Source::from_config() {
            None => Ok(None),
            Some(Source::LocalDir(dir)) => Self::read(dir).map(Some),
            Some(Source::Repository { clone, .. }) if !clone.exists() => Ok(None),
            Some(Source::Repository { clone, .. }) => Self::read(clone).map(Some),
        }
    }

    pub async fn fetch() -> Result<Option<Self>> {
        match Source::from_config() {
            None => Ok(None),
            Some(Source::LocalDir(dir)) => Self::read(dir).map(Some),
            Some(Source::Repository { url, clone }) => {
                if clone.join(".git").exists() {
                    run_git(&[
                        "-C".as_ref(),
                        clone.as_os_str(),
                        "pull".as_ref(),
                        "--ff-only".as_ref(),
                        "--quiet".as_ref(),
                    ])
                    .await?;
                } else {
                    run_git(&[
                        "clone".as_ref(),
                        "--quiet".as_ref(),
                        url.as_ref(),
                        clone.as_os_str(),
                    ])
                    .await?;
                }
                Self::read(clone).map(Some)
            }
        }
    }

    fn read(root: PathBuf) -> Result<Self> {
        let path = root.join(SETUP_FILE);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read the shared setup file {}", path.display()))?;
        let file = SetupFile::parse(&text).with_context(|| format!("{}", path.display()))?;
        Ok(Self { root, file })
    }

    pub fn sync(&self, project: &Project, checkout: &Path) -> Result<Report> {
        self.sync_with_codex_home(project, checkout, &crate::hooks::installer::codex_home())
    }

    fn sync_with_codex_home(
        &self,
        project: &Project,
        checkout: &Path,
        codex_home: &Path,
    ) -> Result<Report> {
        let plan = self.file.plan(&self.root, project)?;
        let mut report = apply::apply(checkout, &plan)?;
        let codex_groups = self.file.codex_hook_groups_of_every_project(&self.root)?;
        if apply::merge_codex_home(codex_home, &codex_groups)? {
            report
                .written
                .push(codex_home.join("hooks.json").display().to_string());
        }
        Ok(report)
    }
}

fn acp_mcp_servers(project: &Project, http: bool) -> Vec<Value> {
    match SharedSetup::load() {
        Ok(Some(setup)) => setup.file.acp_servers(project, http),
        Ok(None) => Vec::new(),
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "loading the shared setup failed");
            Vec::new()
        }
    }
}

pub fn insert_adopted_worktree(
    store: &Store,
    project: &Project,
    worktree: &Worktree,
) -> Result<()> {
    store.insert_worktree(worktree)?;
    sync_checkout(project, &worktree.path);
    Ok(())
}

pub fn sync_checkout(project: &Project, checkout: &Path) {
    let setup = match SharedSetup::load() {
        Ok(Some(setup)) => setup,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "loading the shared setup failed");
            return;
        }
    };
    match setup.sync(project, checkout) {
        Ok(report) => {
            if !report.kept_as_found.is_empty() {
                tracing::info!(
                    checkout = %checkout.display(),
                    kept = ?report.kept_as_found,
                    "shared setup kept the checkout's own files"
                );
            }
            if !report.skipped.is_empty() {
                tracing::debug!(
                    checkout = %checkout.display(),
                    skipped = ?report.skipped,
                    "shared setup items an agent kind cannot take"
                );
            }
        }
        Err(e) => tracing::warn!(
            checkout = %checkout.display(),
            error = %format!("{e:#}"),
            "syncing the shared setup failed"
        ),
    }
}

async fn run_git(args: &[&std::ffi::OsStr]) -> Result<()> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .no_window()
        .output()
        .await
        .context("run git")?;
    if !output.status.success() {
        bail!("git: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
