use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use tokio::process::Child;

pub const SELF_UPDATE: bool = false;
pub const POWERSHELL: &str = "pwsh";
pub const INSTALL_ROOT_MISSING: &str = "self-update is not supported on this platform yet";
pub const INSTALLER_SCRIPT: &str = "install-app.sh";
pub const APP_EXECUTABLE: &str = "berth-desktop";

pub fn tie_to_group(command: &mut tokio::process::Command) {
    command.process_group(0);
}

pub fn bind_to_parent(_child: &Child) {}

pub fn default_install_root() -> Option<PathBuf> {
    None
}

pub fn installer_command(_script: &Path) -> Result<Command, String> {
    Err(INSTALL_ROOT_MISSING.into())
}

pub fn relaunch_command(_exe: &Path, _after_pid: u32) -> Result<Command, String> {
    Err(INSTALL_ROOT_MISSING.into())
}

pub fn detach(_command: &mut Command) -> io::Result<u32> {
    Err(io::Error::other(INSTALL_ROOT_MISSING))
}

pub fn home_cli_dirs() -> Vec<PathBuf> {
    let Some(home) = berth_core::platform::profile_home() else {
        return Vec::new();
    };
    [".local/bin", ".bun/bin", ".npm-global/bin", ".volta/bin"]
        .into_iter()
        .map(|relative| home.join(relative))
        .collect()
}

pub fn bash_program() -> Result<PathBuf, String> {
    Ok(PathBuf::from("bash"))
}

pub fn script_tools_path() -> Option<OsString> {
    None
}
