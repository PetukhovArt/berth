use std::ffi::OsString;
use std::path::PathBuf;

use tokio::process::Child;

pub const POWERSHELL: &str = "pwsh";

pub fn tie_to_group(command: &mut tokio::process::Command) {
    command.process_group(0);
}

pub fn bind_to_parent(_child: &Child) {}

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
