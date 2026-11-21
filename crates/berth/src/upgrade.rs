//! `berth upgrade`: re-run the published install script.
//!
//! install.sh already knows how to pick the right prebuilt binary, fall back
//! to cargo, and report where it landed — this subcommand just saves the user
//! from remembering the curl one-liner.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "PetukhovArt/berth";

#[cfg(unix)]
const INSTALL_URL: &str = "https://raw.githubusercontent.com/PetukhovArt/berth/main/install.sh";

/// The published install script, with `BERTH_INSTALL_URL` as the override
/// hook (tests point it at a file:// URL).
#[cfg(unix)]
fn install_url() -> String {
    berth_core::env::non_empty(berth_core::env::INSTALL_URL)
        .unwrap_or_else(|| INSTALL_URL.to_string())
}

/// Printed whenever a daemon from an older binary is left running: the only
/// way onto the new code is a restart, and a restart takes the sessions.
pub(crate) const KILL_HINT: &str =
    "      run 'berth kill' to restart onto the new binary (stops all sessions).";

fn manifest_version() -> Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest/download/latest.json");
    let output = Command::new(system_tool("curl"))
        .args(["-fsSL", &url])
        .output()
        .context("run curl — is it installed?")?;
    if !output.status.success() {
        bail!(
            "downloading {url} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    parse_manifest_version(&output.stdout).with_context(|| format!("{url} has no version"))
}

fn parse_manifest_version(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let version = value.get("version")?.as_str()?.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    (!version.is_empty()).then(|| version.to_owned())
}

#[cfg(windows)]
fn system_tool(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let path = PathBuf::from(root)
        .join("System32")
        .join(format!("{name}.exe"));
    if path.is_file() {
        path
    } else {
        PathBuf::from(name)
    }
}

#[cfg(unix)]
fn system_tool(name: &str) -> PathBuf {
    PathBuf::from(name)
}

pub fn run_upgrade(force: bool) -> Result<()> {
    // The runtime dir is already the 0700 auth boundary; staging the script
    // there keeps it out of a world-writable /tmp before we execute it.
    berth_daemon::lifecycle::ensure_runtime_dir()?;
    install_release(&berth_core::paths::runtime_dir(), force)?;
    finish_daemon_handoff();
    Ok(())
}

#[cfg(unix)]
fn install_release(staging_dir: &Path, force: bool) -> Result<()> {
    let version = match manifest_version() {
        Ok(version) => Some(version),
        Err(error) => {
            println!("note: {error:#}; installing the latest release");
            None
        }
    };
    install_with(&install_url(), staging_dir, force, version.as_deref())
}

#[cfg(windows)]
fn install_release(staging_dir: &Path, force: bool) -> Result<()> {
    refuse_dev_build(force)?;
    let version = manifest_version()?;
    if version == env!("CARGO_PKG_VERSION") && !force {
        println!("berth {version} is already installed");
        return Ok(());
    }
    let exe = std::env::current_exe().context("locate the running berth.exe")?;
    let exe = berth_core::paths::canonical_or_raw(&exe);
    let dir = staging_dir.join(format!("upgrade-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let swapped = download_and_swap(&version, &dir, &exe);
    let _ = std::fs::remove_dir_all(&dir);
    swapped?;
    println!("installed berth {version} to {}", exe.display());
    Ok(())
}

#[cfg(windows)]
fn download_and_swap(version: &str, dir: &Path, exe: &Path) -> Result<()> {
    let url = format!(
        "https://github.com/{REPO}/releases/download/v{version}/berth-x86_64-pc-windows-msvc.zip"
    );
    let zip = dir.join("berth.zip");
    println!("downloading {url}");
    let status = Command::new(system_tool("curl"))
        .args(["-fsSL", &url, "-o"])
        .arg(&zip)
        .status()
        .context("run curl")?;
    if !status.success() {
        bail!("downloading {url} failed ({status})");
    }
    let status = Command::new(system_tool("tar"))
        .arg("-xf")
        .arg(&zip)
        .arg("-C")
        .arg(dir)
        .status()
        .context("run tar")?;
    let fresh = dir.join("berth.exe");
    if !status.success() || !fresh.is_file() {
        bail!("{url} did not unpack to berth.exe");
    }
    let old = exe.with_file_name("berth.old.exe");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).with_context(|| format!("move {} aside", exe.display()))?;
    if let Err(error) = std::fs::copy(&fresh, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(error).with_context(|| format!("write {}", exe.display()));
    }
    let _ = std::fs::remove_file(&old);
    Ok(())
}

fn refuse_dev_build(force: bool) -> Result<()> {
    if force {
        return Ok(());
    }
    if let Some(exe) = dev_build() {
        bail!(
            "{} is a local cargo build.\n\n\
             Upgrading installs the published binary and would overwrite a \
             ~/.cargo/bin symlink pointing at this build. Rebuild instead:\n\
             \x20   cargo build --release\n\n\
             Re-run with --force to install the published binary anyway.",
            exe.display()
        );
    }
    Ok(())
}

/// Swapping the binary on disk doesn't touch the running daemon — it keeps
/// executing the old code. An idle daemon (no live PTYs) is shut down here so
/// the next launch spawns the new binary; live sessions would die with the
/// daemon, so that restart stays the user's call. Never fails the upgrade:
/// the install already succeeded.
fn finish_daemon_handoff() {
    use crate::client::IdleShutdown;
    match crate::client::run_shutdown_if_idle() {
        Ok(IdleShutdown::NoDaemon) => {}
        Ok(IdleShutdown::ShutDown) => {
            println!(
                "old daemon had no live sessions — shut it down; \
                 the new binary starts on the next launch"
            );
        }
        Ok(IdleShutdown::SessionsLive { count }) => {
            let plural = if count == 1 { "" } else { "s" };
            println!("note: the old daemon is still running with {count} live session{plural}.");
            println!("{KILL_HINT}");
        }
        Ok(IdleShutdown::Skewed) | Err(_) => {
            println!("note: a daemon from a previous version may still be running.");
            println!("{KILL_HINT}");
        }
    }
}

#[cfg(all(test, unix))]
fn upgrade_with(url: &str, staging_dir: &Path, force: bool) -> Result<()> {
    install_with(url, staging_dir, force, None)
}

#[cfg(unix)]
fn install_with(url: &str, staging_dir: &Path, force: bool, version: Option<&str>) -> Result<()> {
    refuse_dev_build(force)?;

    let script = stage_script(url, staging_dir)?;
    // Inherited stdio: the script's own progress lines are the UI here.
    // BERTH_UPGRADE_HANDOFF tells install.sh to skip its "daemon still
    // running" note — finish_daemon_handoff owns that messaging here.
    let mut command = Command::new("sh");
    command.arg(&script).env("BERTH_UPGRADE_HANDOFF", "1");
    if let Some(version) = version {
        command.env("BERTH_VERSION", version);
    }
    let result = command
        .status()
        .with_context(|| format!("run {}", script.display()));
    let _ = std::fs::remove_file(&script);

    let status = result?;
    if !status.success() {
        bail!("install script failed ({status})");
    }
    Ok(())
}

/// Download the installer to a file before running it. `curl … | sh` executes
/// whatever arrived when a connection drops mid-transfer; a staged file either
/// passes the shebang check below or never runs at all.
#[cfg(unix)]
fn stage_script(url: &str, dir: &Path) -> Result<PathBuf> {
    let path = dir.join(format!("install-{}.sh", std::process::id()));
    let output = Command::new("curl")
        .args(["-fsSL", url, "-o"])
        .arg(&path)
        .output()
        .context("run curl — is it installed?")?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        bail!(
            "downloading {url} failed{}{}",
            if detail.is_empty() { "" } else { ": " },
            detail
        );
    }
    // A 200 that isn't the installer (captive portal, error page) would
    // otherwise get executed as a shell script.
    if !std::fs::read_to_string(&path)
        .unwrap_or_default()
        .starts_with("#!")
    {
        let _ = std::fs::remove_file(&path);
        bail!("{url} did not return a shell script");
    }
    Ok(path)
}

/// The running binary when it lives in a cargo build dir (`target/release`,
/// `target/debug`, or the `deps/` dir test binaries run from). Symlinks
/// resolve first: the usual dev setup is a `~/.cargo/bin/berth` symlink
/// pointing into `target/release`, and it's that symlink an upgrade replaces.
fn dev_build() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let dir = exe.parent()?;
    let dir = if dir.file_name()? == "deps" {
        dir.parent()?
    } else {
        dir
    };
    let is_target_dir = matches!(dir.file_name()?.to_str()?, "debug" | "release");
    is_target_dir.then_some(exe)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// curl speaks file://, so these exercise the real download → stage →
    /// execute path without a network.
    fn url_of(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    fn write_script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("source.sh");
        std::fs::write(&path, body).unwrap();
        path
    }

    fn staged_files(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("install-"))
            .collect()
    }

    #[test]
    fn runs_the_downloaded_script_then_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("ran");
        let script = write_script(
            tmp.path(),
            &format!("#!/bin/sh\necho upgrading\n: > '{}'\n", marker.display()),
        );

        upgrade_with(&url_of(&script), tmp.path(), true).unwrap();

        assert!(marker.exists(), "the installer actually ran");
        assert!(
            staged_files(tmp.path()).is_empty(),
            "staged copy is removed"
        );
    }

    #[test]
    fn a_failing_installer_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_script(tmp.path(), "#!/bin/sh\nexit 3\n");

        let err = upgrade_with(&url_of(&script), tmp.path(), true).unwrap_err();
        assert!(err.to_string().contains("install script failed"), "{err}");
        assert!(
            staged_files(tmp.path()).is_empty(),
            "staged copy is removed on failure"
        );
    }

    #[test]
    fn a_missing_url_never_reaches_sh() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nope.sh");

        let err = upgrade_with(&url_of(&missing), tmp.path(), true).unwrap_err();
        assert!(err.to_string().contains("downloading"), "{err}");
        assert!(staged_files(tmp.path()).is_empty());
    }

    #[test]
    fn a_non_script_payload_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        // What a captive portal or a 200-with-error-page looks like.
        let page = write_script(tmp.path(), "<html>sign in to continue</html>");

        let err = upgrade_with(&url_of(&page), tmp.path(), true).unwrap_err();
        assert!(
            err.to_string().contains("did not return a shell script"),
            "{err}"
        );
        assert!(staged_files(tmp.path()).is_empty());
    }

    #[test]
    fn dev_builds_are_refused_without_force() {
        // Cargo runs this binary from <target>/debug/deps, so the guard sees
        // exactly what it would see for a symlinked target/release build.
        assert!(
            dev_build().is_some(),
            "test binary should look like a cargo build"
        );

        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("ran");
        let script = write_script(
            tmp.path(),
            &format!("#!/bin/sh\n: > '{}'\n", marker.display()),
        );

        let err = upgrade_with(&url_of(&script), tmp.path(), false).unwrap_err();
        assert!(err.to_string().contains("local cargo build"), "{err}");
        assert!(
            !marker.exists(),
            "refusing must happen before the installer runs"
        );
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::parse_manifest_version;

    #[test]
    fn reads_the_version_without_a_leading_v() {
        let tagged = br#"{"version":"v0.31.0","platforms":{}}"#;
        assert_eq!(parse_manifest_version(tagged).as_deref(), Some("0.31.0"));
        let plain = br#"{"version":"0.31.0"}"#;
        assert_eq!(parse_manifest_version(plain).as_deref(), Some("0.31.0"));
    }

    #[test]
    fn a_page_that_is_not_the_manifest_has_no_version() {
        assert_eq!(parse_manifest_version(b"<html>sign in</html>"), None);
        assert_eq!(parse_manifest_version(br#"{"version":""}"#), None);
    }
}
