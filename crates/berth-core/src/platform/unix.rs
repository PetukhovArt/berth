//! What macOS and Linux share. `macos.rs` and `linux.rs` re-export from here
//! and add their own differences.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const PROFILE_HOME_VAR: &str = "HOME";
pub const EXECUTABLE_SUFFIXES: &[&str] = &[""];

pub fn no_window_std(command: &mut Command) -> &mut Command {
    command
}

pub fn no_window_tokio(command: &mut tokio::process::Command) -> &mut tokio::process::Command {
    command
}

pub fn open_url_with(opener: &str, url: &str) -> bool {
    Command::new(opener)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn open_path_with(opener: &str, path: &Path) -> io::Result<()> {
    Command::new(opener).arg(path).spawn().map(drop)
}

/// Unix spawns go through the user's login shell (see `berth-daemon`'s
/// `launch`), so this only has to say whether the name is there at all.
pub fn resolve_program(program: &str) -> Option<PathBuf> {
    let named = Path::new(program);
    if named.components().count() > 1 {
        return named.is_file().then(|| named.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

pub fn resolve_editor_program(name: &str) -> Option<PathBuf> {
    resolve_program(name)
}

pub fn process_rss_bytes(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    text.trim().parse::<u64>().ok().map(|kb| kb * 1024)
}

pub fn has_child_processes(pid: u32) -> bool {
    !matches!(
        Command::new("pgrep")
            .arg("-P")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
        Ok(status) if !status.success()
    )
}

pub fn kill_process_tree(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("kill -KILL -{pid} failed"))
    }
}

pub fn symlink(target: &Path, link: &Path, _kind_of: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

pub fn is_link(meta: &std::fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

pub fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    path
}

pub async fn termination_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let (Ok(mut term), Ok(mut int)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

pub fn owner_only(options: &mut std::fs::OpenOptions) {
    std::os::unix::fs::OpenOptionsExt::mode(options, 0o600);
}

pub fn restrict_to_owner(file: &std::fs::File) -> io::Result<()> {
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))
}
