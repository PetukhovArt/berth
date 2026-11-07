//! Windows. No `ps`, no signals, no AF_UNIX, a console per process: most of
//! what unix gets from a shell tool is a Win32 call or the `sysinfo` crate
//! here.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{Capabilities, NoWindow as _, Os};

pub use resolve::{resolve_editor_program, resolve_program};

pub const OS: Os = Os::Windows;

pub const CAPABILITIES: Capabilities = Capabilities {
    pty_answers_terminal_queries: true,
    console_drops_kitty_input: true,
    paste_arrives_as_keys: true,
    win32_console_keys: true,
    paths_case_insensitive: true,
    system_sounds: false,
};

pub const PROFILE_HOME_VAR: &str = "USERPROFILE";
pub const EXECUTABLE_SUFFIXES: &[&str] = &[".exe", ".cmd", ".bat"];

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn no_window_std(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW)
}

pub fn no_window_tokio(command: &mut tokio::process::Command) -> &mut tokio::process::Command {
    command.creation_flags(CREATE_NO_WINDOW)
}

pub fn open_url(url: &str) -> bool {
    use std::os::windows::process::CommandExt;
    // `start` is a cmd builtin (ShellExecute under the hood), so the URL
    // crosses cmd's own parser. std quotes an argv item only when it has
    // whitespace, and an unquoted `&` in a query string would split the
    // command — so the URL is quoted by hand and passed raw. Embedded
    // `"` are stripped, not escaped: cmd has no escape `start` survives,
    // and no http(s) URL needs one. The empty quoted arg is `start`'s
    // window-title slot.
    let quoted = format!("\"{}\"", url.replace('"', ""));
    Command::new("cmd.exe")
        .args(["/c", "start", ""])
        .raw_arg(quoted)
        .no_window()
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn open_path(path: &Path) -> io::Result<()> {
    Command::new("cmd")
        .args(["/c", "start", ""])
        .arg(path)
        .no_window()
        .spawn()
        .map(drop)
}

/// clip.exe assumes the console codepage unless the input carries a
/// UTF-16LE BOM, so non-ASCII text must go over as UTF-16.
pub fn copy_to_clipboard(text: &str) -> bool {
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    super::pipe_into("clip", &[], &utf16)
}

pub fn system_sound(_name: &str) -> Option<PathBuf> {
    None
}

pub fn play_sound_file(_path: &Path) -> bool {
    false
}

/// The working set is Windows' name for the resident set: the process's
/// pages currently in physical memory, which is the number the metrics modal
/// means by RSS everywhere else.
pub fn process_rss_bytes(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let ok = GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0;
        CloseHandle(handle);
        ok.then_some(counters.WorkingSetSize as u64)
    }
}

pub fn system_total_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        // The struct carries its own size so the OS knows which version it
        // was handed; zeroing it leaves that field wrong.
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        (GlobalMemoryStatusEx(&mut status) != 0).then_some(status.ullTotalPhys)
    }
}

/// Windows has no `ps`, and the two candidates that ship with it are both
/// worse than a crate here: `wmic` is deprecated and absent on newer builds,
/// and a PowerShell `Get-CimInstance` costs a second of shell start per poll.
pub fn process_table() -> String {
    use std::fmt::Write;
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

    let what = ProcessRefreshKind::nothing().with_memory().with_cpu();
    let mut system = System::new_with_specifics(RefreshKind::nothing().with_processes(what));
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, what);
    let mut table = String::new();
    for (pid, process) in system.processes() {
        let ppid = process.parent().map(|p| p.as_u32()).unwrap_or(0);
        // `ps` reports KB and the parser multiplies back up, so match it
        // rather than teach the parser a second unit.
        let kb = process.memory() / 1024;
        let cpu_ms = process.accumulated_cpu_time();
        let _ = writeln!(table, "{} {ppid} {kb} {cpu_ms}", pid.as_u32());
    }
    table
}

/// No `pgrep`; sweep the process table for a child instead (`wmic` is gone
/// on 24H2, and PowerShell costs a shell start per poll). A stale table
/// entry can only answer "busy", which is the safe direction.
pub fn has_child_processes(pid: u32) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    );
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .values()
        .any(|p| p.parent().map(|pp| pp.as_u32()) == Some(pid))
}

pub fn kill_process_tree(pid: u32) -> Result<(), String> {
    let out = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .no_window()
        .output()
        .map_err(|error| error.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Err(if stderr.is_empty() { stdout } else { stderr })
}

/// A refusal here is almost always the platform's: creating a symlink needs
/// Developer Mode or elevation.
pub fn symlink(target: &Path, link: &Path, kind_of: &Path) -> io::Result<()> {
    if kind_of.metadata()?.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

pub fn is_link(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const REPARSE_POINT: u32 = 0x400;
    meta.file_type().is_symlink() || meta.file_attributes() & REPARSE_POINT != 0
}

/// `\\?\C:\x` → `C:\x`, `\\?\UNC\srv\share\x` → `\\srv\share\x`. Anything
/// else (a device path, a raw volume GUID) is left exactly as it came: those
/// have no ordinary spelling, and inventing one would be worse than the
/// verbatim form.
pub fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    use std::path::{Component, Prefix};
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return path;
    };
    let text = path.to_string_lossy();
    let stripped = match prefix.kind() {
        Prefix::VerbatimDisk(_) => text.strip_prefix(r"\\?\").map(str::to_string),
        Prefix::VerbatimUNC(..) => text
            .strip_prefix(r"\\?\UNC\")
            .map(|rest| format!(r"\\{rest}")),
        _ => None,
    };
    stripped.map(PathBuf::from).unwrap_or(path)
}

/// Windows has no SIGTERM. The console control events are the equivalent,
/// and the DETACHED_PROCESS launch (see `berth-tui`'s `spawn_daemon`) gives
/// the daemon its own console, so neither reaches it from the TUI's terminal
/// — they arrive only from a deliberate `taskkill` or a `berth kill`
/// fallback. A handler that fails to install leaves Ctrl+C alone standing.
pub async fn termination_signal() {
    use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_shutdown};
    let (Ok(mut brk), Ok(mut close), Ok(mut shutdown)) =
        (ctrl_break(), ctrl_close(), ctrl_shutdown())
    else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = brk.recv() => {}
        _ = close.recv() => {}
        _ = shutdown.recv() => {}
    }
}

pub fn owner_only(_options: &mut std::fs::OpenOptions) {}

pub fn restrict_to_owner(_file: &std::fs::File) -> io::Result<()> {
    Ok(())
}

pub fn login_shell_path() -> Option<OsString> {
    None
}

pub fn login_shell() -> Option<String> {
    None
}

/// Resolve bare program names the way a shell would, because
/// `CreateProcess` will not.
mod resolve {
    use std::path::{Path, PathBuf};

    /// Resolve a program name the way a Windows shell does: an explicit path
    /// is taken as written, a bare name is searched down `PATH`, and either
    /// may be missing the extension, which `PATHEXT` supplies.
    pub fn resolve_program(program: &str) -> Option<PathBuf> {
        let extensions = pathext();
        let path_var = std::env::var_os("PATH").unwrap_or_default();

        let named = Path::new(program);
        if named.components().count() > 1 || named.is_absolute() {
            return first_existing(named, &extensions);
        }
        std::env::split_paths(&path_var)
            .filter(|dir| !dir.as_os_str().is_empty())
            .find_map(|dir| first_existing(&dir.join(program), &extensions))
    }

    /// Resolve the configured editor — `resolve_program`, then Git for
    /// Windows' `usr\bin` as a fallback for a bare name that PATH doesn't
    /// carry. berth's audience always has Git for Windows, and it ships
    /// `vim.exe` (and friends) in an `usr\bin` that PowerShell PATHs never
    /// expose — only `Git\cmd` is. Editor-only on purpose: agent CLIs must
    /// fail honestly rather than silently resolve into MSYS binaries.
    pub fn resolve_editor_program(name: &str) -> Option<PathBuf> {
        if let Some(found) = resolve_program(name) {
            return Some(found);
        }
        if Path::new(name).components().count() > 1 {
            return None; // An explicit path means that path, not a stand-in.
        }
        git_usr_bin(&resolve_program("git.exe")?, name, &pathext())
    }

    /// `<git install root>\usr\bin\<name>`, derived from where `git.exe`
    /// resolved: `<root>\cmd`, `<root>\bin`, or `<root>\mingw64\bin` — the
    /// three places a Git for Windows PATH entry can point.
    fn git_usr_bin(git: &Path, name: &str, extensions: &str) -> Option<PathBuf> {
        let dir = git.parent()?;
        let root = match dir.file_name()?.to_str()?.to_ascii_lowercase().as_str() {
            "cmd" => dir.parent()?,
            "bin" => {
                let up = dir.parent()?;
                match up.file_name().and_then(|n| n.to_str()) {
                    Some(n)
                        if n.eq_ignore_ascii_case("mingw64")
                            || n.eq_ignore_ascii_case("mingw32") =>
                    {
                        up.parent()?
                    }
                    _ => up,
                }
            }
            _ => return None,
        };
        first_existing(&root.join("usr").join("bin").join(name), extensions)
    }

    fn pathext() -> String {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
    }

    /// `base` itself if it is a file, else `base` + each `PATHEXT` entry.
    fn first_existing(base: &Path, extensions: &str) -> Option<PathBuf> {
        if base.is_file() {
            return Some(base.to_path_buf());
        }
        extensions
            .split(';')
            .map(str::trim)
            .filter(|ext| !ext.is_empty())
            // PATHEXT entries carry the leading dot; appending rather than
            // using `set_extension` keeps `cursor-agent` from becoming
            // `cursor.exe` on a name that already contains a dot.
            .map(|ext| PathBuf::from(format!("{}{ext}", base.display())))
            .find(|candidate| candidate.is_file())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The resolved path is compared case-insensitively: PATHEXT is
        /// conventionally upper-case while the file on disk is not, and NTFS
        /// treats the two as one name. What matters is *which file*, not how
        /// it is spelled.
        fn assert_resolves(base: &Path, extensions: &str, expected: Option<&Path>) {
            let got = first_existing(base, extensions);
            let normalize = |p: &Path| p.to_string_lossy().to_lowercase();
            assert_eq!(
                got.as_deref().map(normalize),
                expected.map(normalize),
                "resolving {}",
                base.display()
            );
        }

        /// The two things a Windows shell does that `CreateProcess` will not:
        /// supply the extension, and search `PATH` for a bare name.
        #[test]
        fn a_bare_name_resolves_through_path_and_pathext() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("stub-tool.cmd"), b"@echo off\n").unwrap();
            std::fs::write(dir.join("plain-tool"), b"").unwrap();

            let extensions = ".COM;.EXE;.BAT;.CMD";
            assert_resolves(
                &dir.join("stub-tool"),
                extensions,
                Some(&dir.join("stub-tool.cmd")),
            );
            assert_resolves(
                &dir.join("plain-tool"),
                extensions,
                Some(&dir.join("plain-tool")),
            );
            assert_resolves(&dir.join("absent"), extensions, None);
        }

        /// A name that already contains a dot must gain the extension, not
        /// have its own replaced — `cursor-agent` is fine, but a future
        /// `foo.js` shim would break under `set_extension`.
        #[test]
        fn pathext_appends_and_never_replaces_an_existing_dot() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("foo.js.cmd"), b"").unwrap();
            assert_resolves(
                &dir.join("foo.js"),
                ".EXE;.CMD",
                Some(&dir.join("foo.js.cmd")),
            );
        }

        /// The whole point of the lookup: a bare name, found by walking PATH.
        #[test]
        fn path_is_walked_in_order_for_a_bare_name() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("berth-probe-stub.exe"), b"").unwrap();
            let previous = std::env::var_os("PATH");
            std::env::set_var("PATH", dir);
            let found = resolve_program("berth-probe-stub");
            match previous {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
            assert!(found.is_some(), "a bare name must be found down PATH");
        }

        /// The Git-for-Windows fallback must find `usr\bin\vim.exe` from any
        /// of the three dirs a PATH entry resolves `git.exe` in, and stay
        /// silent when the editor isn't shipped there.
        #[test]
        fn git_usr_bin_is_derived_from_each_git_location() {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            for dir in ["cmd", "bin", r"mingw64\bin", r"usr\bin"] {
                std::fs::create_dir_all(root.join(dir)).unwrap();
            }
            let vim = root.join(r"usr\bin\vim.exe");
            std::fs::write(&vim, b"").unwrap();

            let extensions = ".COM;.EXE";
            // Case-normalized like `assert_resolves`: PATHEXT supplies `.EXE`.
            let normalize = |p: &Path| p.to_string_lossy().to_lowercase();
            for git in [r"cmd\git.exe", r"bin\git.exe", r"mingw64\bin\git.exe"] {
                let got = git_usr_bin(&root.join(git), "vim", extensions);
                assert_eq!(
                    got.as_deref().map(normalize),
                    Some(normalize(&vim)),
                    "from {git}"
                );
            }
            assert_eq!(
                git_usr_bin(&root.join(r"cmd\git.exe"), "hx", extensions),
                None,
                "an editor Git doesn't ship must stay unresolved"
            );
            assert_eq!(
                git_usr_bin(Path::new(r"C:\somewhere\else\git.exe"), "vim", extensions),
                None,
                "an unrecognised git layout must not invent a root"
            );
        }
    }
}
