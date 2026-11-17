//! macOS. Shares the unix plumbing; differs in the openers
//! and memory probes (there is no `/proc`).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::{Capabilities, Os};

pub use super::unix::{
    has_child_processes, is_link, kill_process_tree, lead_process_tree, no_window_std,
    no_window_tokio, owner_only, resolve_program, restrict_to_owner, strip_verbatim_prefix,
    symlink, termination_signal, EXECUTABLE_SUFFIXES, PROFILE_HOME_VAR,
};

pub const OS: Os = Os::Macos;

pub const CAPABILITIES: Capabilities = Capabilities {
    pty_answers_terminal_queries: false,
    pty_gives_vs16_its_own_cell: false,
    paths_case_insensitive: false,
};

pub fn open_path(path: &Path) -> io::Result<()> {
    super::unix::open_path_with("open", path)
}

pub fn speech_to_file(
    text_file: &Path,
    _language: &str,
) -> Option<(std::process::Command, PathBuf)> {
    let out = text_file.with_extension("m4a");
    let mut command = std::process::Command::new("say");
    command.arg("-f").arg(text_file).arg("-o").arg(&out);
    Some((command, out))
}

pub fn system_total_bytes() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

pub fn process_table() -> String {
    use std::fmt::Write;

    // macOS `time=` is `[dd-]hh:mm:ss.ff` — hundredths, so 10ms.
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss=,time="])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let mut table = String::new();
    for line in out.lines() {
        let mut cols = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(kb), Some(time)) =
            (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            continue;
        };
        let _ = writeln!(table, "{pid} {ppid} {kb} {}", parse_ps_time_ms(time));
    }
    table
}

/// `[dd-]hh:mm:ss[.ff]` to milliseconds. An unparseable field reads 0,
/// which the caller turns into "not measured" rather than "idle".
fn parse_ps_time_ms(time: &str) -> u64 {
    let (days, rest) = match time.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().unwrap_or(0), rest),
        None => (0, time),
    };
    let (rest, hundredths) = match rest.split_once('.') {
        Some((r, f)) => (r, f.parse::<u64>().unwrap_or(0)),
        None => (rest, 0),
    };
    // Fields run least-significant last and the hours field is optional.
    let secs = rest
        .rsplit(':')
        .enumerate()
        .map(|(i, part)| part.parse::<u64>().unwrap_or(0) * 60u64.pow(i as u32))
        .sum::<u64>();
    (days * 86_400 + secs) * 1000 + hundredths * 10
}

const DEFAULT_SHELL: &str = "/bin/zsh";

pub fn login_shell() -> Option<String> {
    Some(super::unix::login_shell(DEFAULT_SHELL))
}

pub fn login_shell_path() -> Option<OsString> {
    super::unix::login_shell_path(&super::unix::login_shell(DEFAULT_SHELL))
}
