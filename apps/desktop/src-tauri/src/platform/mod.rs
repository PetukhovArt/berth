use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use berth_core::platform::{Capabilities, Os};
use serde::Serialize;
use tokio::process::Child;

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

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowChrome {
    pub native_decorations: bool,
    pub native_menu: bool,
}

pub const WINDOW_CHROME: WindowChrome = imp::WINDOW_CHROME;

pub const SELF_UPDATE: bool = imp::SELF_UPDATE;

pub const UNSUPPORTED: &str = "not supported on this platform yet";

pub const SECRETS_DEFAULT_TO_FILE: bool = imp::SECRETS_DEFAULT_TO_FILE;

pub const POWERSHELL: &str = imp::POWERSHELL;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    os: Os,
    capabilities: Capabilities,
    self_update: bool,
    window_chrome: WindowChrome,
}

pub async fn platform_info() -> Result<PlatformInfo, String> {
    Ok(PlatformInfo {
        os: berth_core::platform::OS,
        capabilities: berth_core::platform::CAPABILITIES,
        self_update: SELF_UPDATE,
        window_chrome: WINDOW_CHROME,
    })
}

pub fn spawn_tied(command: &mut tokio::process::Command) -> io::Result<Child> {
    imp::tie_to_parent(command);
    let child = command.spawn()?;
    imp::bind_to_parent(&child);
    Ok(child)
}

pub fn adopt_login_shell_path() {
    if let Some(path) = berth_core::platform::login_shell_path() {
        std::env::set_var("PATH", path);
    }
}

pub fn default_install_root() -> Option<PathBuf> {
    imp::default_install_root()
}

pub const INSTALL_ROOT_MISSING: &str = imp::INSTALL_ROOT_MISSING;

pub const INSTALLER_SCRIPT: &str = imp::INSTALLER_SCRIPT;

pub const APP_EXECUTABLE: &str = imp::APP_EXECUTABLE;

pub fn installer_command(script: &Path) -> Result<Command, String> {
    imp::installer_command(script)
}

pub fn relaunch_command(exe: &Path, after_pid: u32) -> Result<Command, String> {
    imp::relaunch_command(exe, after_pid)
}

pub fn detach(command: &mut Command) -> io::Result<u32> {
    imp::detach(command)
}

pub fn cli_search_dirs() -> Vec<PathBuf> {
    imp::cli_search_dirs()
}

pub async fn open_login_console(command: &[&str]) -> Result<(), String> {
    imp::open_login_console(command).await
}

#[cfg(feature = "desktop")]
pub fn webview_profile_bases() -> Vec<PathBuf> {
    imp::webview_profile_bases()
}

pub fn bash_program() -> Result<PathBuf, String> {
    imp::bash_program()
}

pub fn script_tools_path() -> Option<std::ffi::OsString> {
    imp::script_tools_path()
}
