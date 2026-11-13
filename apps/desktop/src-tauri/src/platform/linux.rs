use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use super::WindowChrome;

pub use super::unix::{bash_program, bind_to_parent, script_tools_path, POWERSHELL};

pub const WINDOW_CHROME: WindowChrome = WindowChrome {
    native_decorations: false,
    native_menu: false,
};

pub const SECRETS_DEFAULT_TO_FILE: bool = true;
pub const SELF_UPDATE: bool = false;
pub const INSTALL_ROOT_MISSING: &str = "self-update is not supported on this platform yet";
pub const INSTALLER_SCRIPT: &str = "install-app.sh";
pub const APP_EXECUTABLE: &str = "berth-desktop";

pub fn default_install_root() -> Option<PathBuf> {
    None
}

pub fn installer_command(_script: &Path) -> Result<std::process::Command, String> {
    Err(INSTALL_ROOT_MISSING.into())
}

pub fn relaunch_command(_exe: &Path, _after_pid: u32) -> Result<std::process::Command, String> {
    Err(INSTALL_ROOT_MISSING.into())
}

pub fn detach(command: &mut std::process::Command) -> io::Result<u32> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = command.spawn()?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

pub fn install_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.to_path_buf()
}

#[cfg(feature = "desktop")]
pub fn tray_template_icon() -> Option<tauri::image::Image<'static>> {
    None
}

#[cfg(feature = "desktop")]
pub fn reopens_main_window(_event: &tauri::RunEvent) -> bool {
    false
}

pub fn tie_to_parent(command: &mut Command) {
    super::unix::tie_to_group(command);
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                libc::_exit(1);
            }
            Ok(())
        });
    }
}

pub fn cli_search_dirs() -> Vec<PathBuf> {
    let mut dirs = super::unix::home_cli_dirs();
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

pub async fn open_login_console(command: &[&str]) -> Result<(), String> {
    let script = format!(
        "{}; echo; printf 'Нажмите Enter, чтобы закрыть: '; read -r _",
        command.join(" ")
    );
    for terminal in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        let mut launcher = Command::new(terminal);
        launcher.arg(if terminal == "gnome-terminal" {
            "--"
        } else {
            "-e"
        });
        launcher.args(["sh", "-c", &script]);
        if let Ok(mut child) = launcher.spawn() {
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            return Ok(());
        }
    }
    Err("не найден терминал для запуска входа".into())
}

#[cfg(feature = "desktop")]
pub fn webview_profile_bases() -> Vec<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| berth_core::platform::profile_home().map(|home| home.join(".local/share")))
        .into_iter()
        .collect()
}
