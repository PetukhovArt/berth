use std::path::PathBuf;

use tokio::process::Command;

use super::WindowChrome;

pub use super::unix::{
    bash_program, bind_to_parent, default_install_root, detach, installer_command,
    relaunch_command, script_tools_path, APP_EXECUTABLE, INSTALLER_SCRIPT, INSTALL_ROOT_MISSING,
    POWERSHELL, SELF_UPDATE,
};

pub const WINDOW_CHROME: WindowChrome = WindowChrome {
    native_decorations: false,
    native_menu: false,
};

pub const SECRETS_DEFAULT_TO_FILE: bool = false;

pub fn tie_to_parent(command: &mut Command) {
    super::unix::tie_to_group(command);
}

pub fn cli_search_dirs() -> Vec<PathBuf> {
    let mut dirs = super::unix::home_cli_dirs();
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs
}

pub async fn open_login_console(command: &[&str]) -> Result<(), String> {
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
        command.join(" ")
    );
    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .await
        .map_err(|error| format!("не удалось открыть Terminal: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("не удалось открыть Terminal".into())
    }
}

#[cfg(feature = "desktop")]
pub fn webview_profile_bases() -> Vec<PathBuf> {
    berth_core::platform::profile_home()
        .map(|home| home.join("Library/WebKit"))
        .into_iter()
        .collect()
}
