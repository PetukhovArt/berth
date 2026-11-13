//! PLATFORM ADAPTER: every OS service the crates (and the desktop) need,
//! behind one API.
//!
//! This module is where Windows, macOS and Linux differ, so the rest of the
//! tree does not have to. One implementation module per OS is picked at
//! compile time below — `windows.rs`, `macos.rs`, `linux.rs` (which also
//! stands in for any other unix), with `unix.rs` holding what macOS and Linux
//! share. Callers write `platform::copy_to_clipboard(..)`, never
//! `cfg(target_os = ..)`; a caller that has to behave differently branches
//! on a [`Capabilities`] field, which names *why* rather than *where*.
//!
//! Adding a concern: a facade function (or a `Capabilities` field) here,
//! then the same function in every implementation module. A target that
//! lacks one fails to compile on that target only — which is why CI builds
//! all three. Where an OS has no implementation yet, the function answers
//! "unsupported" (`None`, `false`, an `Err`) rather than panicking, so the
//! caller can hide the action.
//!
//! Adding a platform: one more implementation module and one more `cfg`
//! line below.
//!
//! Not everything per-OS lives here. Concerns big enough to own a module
//! already hide their OS split behind one API of their own, in the same
//! shape: `transport` (the DAEMON SOCKET), `paths` (the RUNTIME DIR, whose
//! names are load-bearing), and in `berth-daemon` the PTY layer, `launch`
//! and `lifecycle`.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(windows)]
#[path = "windows.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod imp;

#[cfg(all(unix, not(target_os = "macos")))]
#[path = "linux.rs"]
mod imp;

#[cfg(unix)]
mod unix;

#[cfg(any(unix, test))]
mod login_path;

/// The OS this binary was built for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Macos,
    Linux,
}

pub const OS: Os = imp::OS;

/// What differs about this OS, named by consequence. Callers branch on
/// these, so a behaviour can be tested (and later switched) without an OS
/// in the condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// The pseudo console answers a child's terminal queries (DSR, DA)
    /// itself, so a client's own answer arrives as a duplicate — ConPTY.
    pub pty_answers_terminal_queries: bool,
    /// The console drops CSI-u (kitty) key encodings on input, so keys must
    /// stay on legacy bytes whatever the child pushed — the Windows 10
    /// inbox ConPTY.
    pub console_drops_kitty_input: bool,
    /// A paste reaches the TUI as a burst of keystrokes rather than as a
    /// bracketed paste — the Windows console.
    pub paste_arrives_as_keys: bool,
    /// Keys come through the Win32 console API, which delivers chords a
    /// unix terminal emulator would swallow, and swallows others.
    pub win32_console_keys: bool,
    /// One directory may be spelled in any case (`D:\Repo` is `d:\repo`).
    pub paths_case_insensitive: bool,
    /// Named system sounds exist ([`system_sound`] can answer).
    pub system_sounds: bool,
}

pub const CAPABILITIES: Capabilities = imp::CAPABILITIES;

/// The OS's own home directory — `USERPROFILE` on Windows, `HOME` on unix.
/// Not [`crate::env::home_dir`], which honours a shell's `HOME` on Windows
/// too: this one is for data dirs that must not move with the shell.
pub fn profile_home() -> Option<PathBuf> {
    std::env::var_os(imp::PROFILE_HOME_VAR).map(PathBuf::from)
}

/// The variable [`profile_home`] reads, for error messages.
pub const PROFILE_HOME_VAR: &str = imp::PROFILE_HOME_VAR;

/// `cmd.no_window()` — chainable, platform-free at the call site.
///
/// On Windows a console child spawned from a process without a console (the
/// DETACHED_PROCESS daemon, or the TUI once its console is in raw mode's
/// alternate screen) allocates a brand-new visible console — every git poll
/// and `gh` call flashed a window while the user browsed worktrees.
/// `CREATE_NO_WINDOW` runs the child with a console but no window, which is
/// exactly what a captured-output helper wants. Elsewhere this is a no-op.
///
/// Not for every spawn: `creation_flags` *replaces* the flag word, so a site
/// that needs other creation flags (the daemon auto-spawn's
/// `DETACHED_PROCESS`, see `detach` in `crates/berth/src/client.rs`) must
/// keep setting its own.
pub trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for std::process::Command {
    fn no_window(&mut self) -> &mut Self {
        imp::no_window_std(self)
    }
}

impl NoWindow for tokio::process::Command {
    fn no_window(&mut self) -> &mut Self {
        imp::no_window_tokio(self)
    }
}

/// Hand `url` to the desktop's default browser: `open` on macOS, `xdg-open`
/// on Linux, `cmd /c start` on Windows. Returns whether the opener reported
/// success (it hands off and exits; nothing waits on the browser).
///
/// Callers own their scheme allowlists and test shortcuts — a `cfg!(test)`
/// here would be false in every crate that depends on this one.
pub fn open_url(url: &str) -> bool {
    imp::open_url(url)
}

/// Open a file with whatever the OS associates with it. Spawns and returns;
/// nothing waits on the app.
pub fn open_path(path: &Path) -> io::Result<()> {
    imp::open_path(path)
}

/// Resolve a program name the way a shell would: an explicit path is taken
/// as written when it is a file, a bare name is searched down `PATH`, and
/// on Windows either may be missing the extension, which `PATHEXT` supplies
/// (`CreateProcess` does none of this).
pub fn resolve_program(program: &str) -> Option<PathBuf> {
    imp::resolve_program(&expand_home(program, std::env::home_dir().as_deref()))
}

fn expand_home(program: &str, home: Option<&Path>) -> String {
    let rest = program
        .strip_prefix("~/")
        .or_else(|| program.strip_prefix("~\\"));
    match (rest, home) {
        (Some(rest), Some(home)) => home.join(rest).display().to_string(),
        _ => program.to_string(),
    }
}

/// [`resolve_program`] for the configured editor, plus whatever fallback
/// the OS ships editors in (Git for Windows' `usr\bin`).
pub fn resolve_editor_program(name: &str) -> Option<PathBuf> {
    imp::resolve_editor_program(name)
}

/// Suffixes an executable file may carry, most likely first: `[""]` on
/// unix, `.exe`/`.cmd`/`.bat` on Windows.
pub const EXECUTABLE_SUFFIXES: &[&str] = imp::EXECUTABLE_SUFFIXES;

/// Copy to *this machine's* system clipboard. macOS: pbcopy. Windows:
/// clip.exe. Linux: wl-copy on Wayland, xclip (or xsel) on X11.
pub fn copy_to_clipboard(text: &str) -> bool {
    imp::copy_to_clipboard(text)
}

/// The file behind a named system sound (`Glass`), when this OS has named
/// sounds and that one exists. `name` must already be a bare stem.
pub fn system_sound(name: &str) -> Option<PathBuf> {
    imp::system_sound(name)
}

/// Play a sound file detached; `false` when nothing could start.
pub fn play_sound_file(path: &Path) -> bool {
    imp::play_sound_file(path)
}

/// Resident set size of one process, bytes.
pub fn process_rss_bytes(pid: u32) -> Option<u64> {
    imp::process_rss_bytes(pid)
}

/// Physical memory installed on this machine, bytes.
pub fn system_total_bytes() -> Option<u64> {
    imp::system_total_bytes()
}

/// Every process on the machine as `pid ppid rss-in-KB cpu-ms`, one per
/// line, where `cpu-ms` is that process's cumulative CPU time since it
/// started. A platform that cannot answer `cpu-ms` may leave the column off.
///
/// Text on purpose: the parser (the daemon's `metrics`) is the tested part
/// and takes nothing else, so a platform only says how it enumerates.
pub fn process_table() -> String {
    imp::process_table()
}

/// Does `pid` have any child process? A failed probe answers `true` —
/// callers reap on `false`, and must never reap what they cannot inspect.
pub fn has_child_processes(pid: u32) -> bool {
    imp::has_child_processes(pid)
}

/// Kill `pid` and everything under it. On unix that is the process group
/// `pid` leads, so the child must have been spawned as a group leader.
pub fn kill_process_tree(pid: u32) -> Result<(), String> {
    imp::kill_process_tree(pid)
}

pub fn lead_process_tree(command: &mut tokio::process::Command) -> &mut tokio::process::Command {
    imp::lead_process_tree(command)
}

/// Create `link` pointing at `target`. Windows picks a file or a directory
/// symlink at creation time, settled by what `kind_of` is (followed).
pub fn symlink(target: &Path, link: &Path, kind_of: &Path) -> io::Result<()> {
    imp::symlink(target, link, kind_of)
}

/// Is this entry a link — a symlink, or on Windows any reparse point (a
/// junction included)?
pub fn is_link(meta: &std::fs::Metadata) -> bool {
    imp::is_link(meta)
}

/// `\\?\C:\x` → `C:\x`: the verbatim form only the Win32 file APIs read.
/// Identity elsewhere.
pub fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    imp::strip_verbatim_prefix(path)
}

/// Resolve when the OS asks this process to stop: SIGTERM or SIGINT on
/// unix; Ctrl+C, Ctrl+Break, console close or shutdown on Windows.
pub async fn termination_signal() {
    imp::termination_signal().await
}

/// Create files through `options` readable by this user only (mode 0600).
/// Windows relies on the profile's inherited ACL instead.
pub fn owner_only(options: &mut std::fs::OpenOptions) {
    imp::owner_only(options)
}

/// Narrow an open file to this user (mode 0600), whatever it was created
/// with. A no-op on Windows.
pub fn restrict_to_owner(file: &std::fs::File) -> io::Result<()> {
    imp::restrict_to_owner(file)
}

/// The `PATH` a login shell would give a program, for a process that was
/// not started from one (an app launched from the Dock or Finder inherits
/// `launchd`'s minimal `PATH`, one started from a `.desktop` file the
/// session's). The login shell's directories come first, then the
/// inherited ones it lacks. `None` means keep the inherited one: the shell
/// did not answer within a few seconds, and always on Windows, where
/// `PATH` lives in the registry and every process reads the same.
///
/// Costs one interactive login shell start, so call it once, before the
/// process has threads that read the environment.
pub fn login_shell_path() -> Option<OsString> {
    imp::login_shell_path()
}

/// The user's login shell: `$SHELL`, else the account's shell, else the
/// OS default (`/bin/zsh` on macOS, `/bin/bash` on Linux). `None` on
/// Windows, where nothing is launched through one.
pub fn login_shell() -> Option<String> {
    imp::login_shell()
}

/// `-l` login, `-i` interactive, `-c` take the command line: between them
/// a command sees the profile and rc files a terminal tab would.
pub const LOGIN_SHELL_ARGS: [&str; 3] = ["-l", "-i", "-c"];

/// Run `program args…` with `data` on its stdin; whether it took it and
/// exited cleanly. The clipboard helpers are all this shape.
fn pipe_into(program: &str, args: &[&str], data: &[u8]) -> bool {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .no_window()
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(data).is_ok());
    wrote && child.wait().is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both probes answer for *this* process and *this* machine, so a plain
    /// sanity check is available on every platform: a running process has a
    /// non-zero resident set, and the machine has some RAM installed.
    #[test]
    fn this_process_and_this_machine_report_plausible_memory() {
        let rss = process_rss_bytes(std::process::id());
        assert!(
            rss.is_some_and(|b| b > 0),
            "a running process has a resident set: {rss:?}"
        );
        let total = system_total_bytes();
        assert!(
            total.is_some_and(|b| b > 64 * 1024 * 1024),
            "the machine has RAM: {total:?}"
        );
    }

    #[test]
    fn a_leading_tilde_is_the_home_dir() {
        let home = Path::new("/home/me");
        assert_eq!(
            expand_home("~/.grok/bin/agent.exe", Some(home)),
            home.join(".grok/bin/agent.exe").display().to_string()
        );
        assert_eq!(
            expand_home("~\\bin\\a.exe", Some(home)),
            home.join("bin\\a.exe").display().to_string()
        );
        assert_eq!(expand_home("~user/x", Some(home)), "~user/x");
        assert_eq!(expand_home("agent", Some(home)), "agent");
        assert_eq!(expand_home("~/x", None), "~/x");
    }

    /// The process table names this process with its parent, in the text
    /// shape the daemon's parser reads.
    #[test]
    fn the_process_table_lists_this_process() {
        let me = std::process::id().to_string();
        let table = process_table();
        assert!(
            table.lines().any(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                cols.len() >= 3 && cols[0] == me
            }),
            "pid {me} missing from the table"
        );
    }
}
