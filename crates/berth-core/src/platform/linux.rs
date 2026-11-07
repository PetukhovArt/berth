//! Linux, and any unix that is not macOS. Shares the unix plumbing; reads
//! `/proc` where macOS shells out.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::{Capabilities, Os};

pub use super::unix::{
    has_child_processes, is_link, kill_process_tree, no_window_std, no_window_tokio, owner_only,
    process_rss_bytes, resolve_editor_program, resolve_program, restrict_to_owner,
    strip_verbatim_prefix, symlink, termination_signal, EXECUTABLE_SUFFIXES, PROFILE_HOME_VAR,
};

pub const OS: Os = Os::Linux;

pub const CAPABILITIES: Capabilities = Capabilities {
    pty_answers_terminal_queries: false,
    console_drops_kitty_input: false,
    paste_arrives_as_keys: false,
    win32_console_keys: false,
    paths_case_insensitive: false,
    system_sounds: false,
};

pub fn open_url(url: &str) -> bool {
    super::unix::open_url_with("xdg-open", url)
}

pub fn open_path(path: &Path) -> io::Result<()> {
    super::unix::open_path_with("xdg-open", path)
}

pub fn copy_to_clipboard(text: &str) -> bool {
    let bytes = text.as_bytes();
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return super::pipe_into("wl-copy", &[], bytes);
    }
    // X11: prefer xclip, fall back to xsel.
    super::pipe_into("xclip", &["-selection", "clipboard"], bytes)
        || super::pipe_into("xsel", &["--clipboard", "--input"], bytes)
}

pub fn system_sound(_name: &str) -> Option<PathBuf> {
    None
}

pub fn play_sound_file(_path: &Path) -> bool {
    false
}

pub fn system_total_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

pub fn process_table() -> String {
    use std::fmt::Write;

    // `ps -o time=` quantizes to whole seconds, a 20% error at a 5s tick.
    // /proc carries the real thing in clock ticks, costs no fork, and has
    // ppid and rss in the same read.
    let sysconf = |name| {
        let value = unsafe { libc::sysconf(name) };
        if value > 0 {
            value as u64
        } else {
            1
        }
    };
    let ticks_per_sec = sysconf(libc::_SC_CLK_TCK);
    let page_kb = (sysconf(libc::_SC_PAGESIZE) / 1024).max(1);
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return String::new();
    };
    let mut table = String::new();
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        // A process can exit between the readdir and the read; skip it.
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        // Field 2 is the comm, parenthesised and free to contain both
        // spaces and parens, so cut after its last ')' rather than
        // tokenizing the whole line.
        let Some((_, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        // `rest` starts at field 3 (state), so ppid is [1], utime [11],
        // stime [12] and rss-in-pages [21].
        let f: Vec<&str> = rest.split_whitespace().collect();
        let field = |i: usize| f.get(i).and_then(|s| s.parse::<u64>().ok());
        let (Some(ppid), Some(utime), Some(stime), Some(rss_pages)) =
            (field(1), field(11), field(12), field(21))
        else {
            continue;
        };
        let cpu_ms = (utime + stime) * 1000 / ticks_per_sec;
        let _ = writeln!(table, "{pid} {ppid} {} {cpu_ms}", rss_pages * page_kb);
    }
    table
}

pub fn login_shell_path() -> Option<OsString> {
    None
}
