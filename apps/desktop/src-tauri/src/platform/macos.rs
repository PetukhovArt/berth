use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};

use tokio::process::Command;

use super::WindowChrome;

pub use super::unix::{bash_program, bind_to_parent, script_tools_path, POWERSHELL};

pub const WINDOW_CHROME: WindowChrome = WindowChrome {
    native_decorations: true,
    native_menu: true,
};

pub const SECRETS_DEFAULT_TO_FILE: bool = false;
pub const SELF_UPDATE: bool = true;
pub const INSTALL_ROOT_MISSING: &str = "HOME is not set";
pub const INSTALLER_SCRIPT: &str = "install-app.sh";
pub const APP_EXECUTABLE: &str = "Berth.app/Contents/MacOS/berth-desktop";

pub fn tie_to_parent(command: &mut Command) {
    super::unix::tie_to_group(command);
}

pub fn default_install_root() -> Option<PathBuf> {
    berth_core::platform::profile_home().map(|home| home.join("Library/Application Support/Berth"))
}

pub fn installer_command(script: &Path) -> Result<StdCommand, String> {
    let mut command = StdCommand::new("/bin/bash");
    command
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

pub fn relaunch_command(exe: &Path, after_pid: u32) -> Result<StdCommand, String> {
    let bundle = exe
        .ancestors()
        .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
        .ok_or_else(|| format!("{} is not inside an .app bundle", exe.display()))?;
    let mut command = StdCommand::new("/bin/sh");
    command
        .arg("-c")
        .arg("while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec open -n \"$2\"")
        .arg("sh")
        .arg(after_pid.to_string())
        .arg(bundle);
    Ok(command)
}

pub fn detach(command: &mut StdCommand) -> io::Result<u32> {
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
    let bundle = exe_dir.parent().and_then(Path::parent);
    match bundle {
        Some(app)
            if exe_dir.ends_with("Contents/MacOS")
                && app.extension().is_some_and(|ext| ext == "app") =>
        {
            app.parent().unwrap_or(app).to_path_buf()
        }
        _ => exe_dir.to_path_buf(),
    }
}

#[cfg(feature = "desktop")]
pub fn tray_template_icon() -> Option<tauri::image::Image<'static>> {
    Some(tauri::include_image!("icons/tray-template.png"))
}

#[cfg(feature = "desktop")]
pub fn reopens_main_window(event: &tauri::RunEvent) -> bool {
    matches!(event, tauri::RunEvent::Reopen { .. })
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
