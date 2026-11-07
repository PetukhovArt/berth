use std::ffi::OsString;
use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use berth_core::platform::NoWindow;
use tokio::process::Child;

use super::WindowChrome;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

pub const WINDOW_CHROME: WindowChrome = WindowChrome {
    native_decorations: false,
    native_menu: false,
};

pub const SELF_UPDATE: bool = true;
pub const SECRETS_DEFAULT_TO_FILE: bool = false;
pub const POWERSHELL: &str = "powershell";
pub const INSTALL_ROOT_MISSING: &str = "LOCALAPPDATA is not set";
pub const INSTALLER_SCRIPT: &str = "install-app.ps1";
pub const APP_EXECUTABLE: &str = "berth-desktop.exe";

pub fn tie_to_parent(command: &mut tokio::process::Command) {
    command.no_window();
}

pub fn bind_to_parent(child: &Child) {
    let (Some(job), Some(process)) = (job::handle(), child.raw_handle()) else {
        return;
    };
    unsafe {
        windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(job, process);
    }
}

pub fn default_install_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Berth"))
}

fn hidden_powershell() -> Command {
    let mut command = Command::new("powershell.exe");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_window();
    command
}

pub fn installer_command(script: &Path) -> Result<Command, String> {
    let mut command = hidden_powershell();
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script);
    Ok(command)
}

pub fn relaunch_command(exe: &Path, after_pid: u32) -> Result<Command, String> {
    let quoted = exe.display().to_string().replace('\'', "''");
    let line = format!(
        "Wait-Process -Id {after_pid} -ErrorAction SilentlyContinue; Start-Process -FilePath '{quoted}'"
    );
    let mut command = hidden_powershell();
    command.args(["-NoProfile", "-NonInteractive", "-Command", &line]);
    Ok(command)
}

pub fn detach(command: &mut Command) -> io::Result<u32> {
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
    match command.spawn() {
        Ok(child) => Ok(child.id()),
        Err(_) => {
            command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
            command.spawn().map(|child| child.id())
        }
    }
}

pub fn cli_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = berth_core::platform::profile_home() {
        for relative in [".local/bin", ".bun/bin", ".npm-global/bin", ".volta/bin"] {
            dirs.push(home.join(relative));
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs
}

pub async fn open_login_console(command: &[&str]) -> Result<(), String> {
    let status = tokio::process::Command::new("cmd")
        .args(["/c", "start", "", "cmd", "/k"])
        .args(command)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .await
        .map_err(|error| format!("не удалось открыть консоль: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("не удалось открыть консоль".into())
    }
}

#[cfg(feature = "desktop")]
pub fn webview_profile_bases() -> Vec<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .into_iter()
        .collect()
}

pub fn bash_program() -> Result<PathBuf, String> {
    git_bash().ok_or_else(|| "Git Bash не найден: укажите JIG_BASH".to_owned())
}

pub fn script_tools_path() -> Option<OsString> {
    let dir = git_bash()?.parent()?.to_path_buf();
    let mut entries = vec![dir];
    if let Some(existing) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(entries).ok()
}

fn git_bash() -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("JIG_BASH") {
        return Some(PathBuf::from(custom));
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if !dir.join("git.exe").is_file() {
            continue;
        }
        let mut level = Some(dir.as_path());
        for _ in 0..3 {
            let Some(current) = level else { break };
            for candidate in [
                current.join("bin").join("bash.exe"),
                current.join("usr").join("bin").join("bash.exe"),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            level = current.parent();
        }
    }
    None
}

mod job {
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    static JOB: OnceLock<usize> = OnceLock::new();

    pub fn handle() -> Option<HANDLE> {
        let raw = *JOB.get_or_init(create);
        (raw != 0).then_some(raw as HANDLE)
    }

    fn create() -> usize {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if set == 0 {
                return 0;
            }
            job as usize
        }
    }
}
