//! `PATH` for PTY children with the daemon's own directory first.
//!
//! A session runs a bare `berth` (`berth rename`, `berth worktree`,
//! `berth spawn`), and that must be the build this daemon speaks to. The
//! desktop app starts the daemon from the `berth` beside it, which is newer
//! than the one on the user's `PATH` after every app update: VERSION SKEW.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The inherited `PATH` with the daemon's directory in front, or `None`
/// when the daemon cannot name its own executable.
pub(super) fn path() -> Option<OsString> {
    let exe = std::env::current_exe().ok()?;
    // The app runs the daemon through its `current` link, which the next
    // update repoints at a newer build: name the build itself.
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let dir = plain(exe.parent()?);
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(std::iter::once(dir).chain(std::env::split_paths(&inherited))).ok()
}

/// `canonicalize` answers `\\?\C:\…` on Windows, which a `PATH` search
/// in `cmd` or Git Bash does not follow.
fn plain(dir: &Path) -> PathBuf {
    match dir.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => dir.to_path_buf(),
    }
}
