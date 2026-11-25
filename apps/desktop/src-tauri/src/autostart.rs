use std::path::PathBuf;

use auto_launch::{AutoLaunch, AutoLaunchBuilder, WindowsEnableMode};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Runtime};

use crate::working_copy;

const ARG: &str = "--autostart";
const HIDDEN_FLAG: &str = "window.__BERTH_AT_LOGIN__ = true;";

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let mut builder = Builder::new("berth-autostart");
    if std::env::args().any(|arg| arg == ARG) {
        builder = builder.js_init_script(HIDDEN_FLAG.to_owned());
    }
    builder.build()
}

fn executable() -> Result<PathBuf, String> {
    if let Ok(launcher) = working_copy::launcher() {
        return Ok(launcher);
    }
    if let Some(image) = std::env::var_os("APPIMAGE") {
        return Ok(image.into());
    }
    std::env::current_exe().map_err(|error| error.to_string())
}

fn login_item() -> Result<AutoLaunch, String> {
    let path = executable()?.display().to_string();
    let (name, path) = if cfg!(target_os = "macos") {
        (crate::IDENTIFIER, path)
    } else {
        ("Berth", format!("\"{path}\""))
    };
    AutoLaunchBuilder::new()
        .set_app_name(name)
        .set_app_path(&path)
        .set_args(&[ARG])
        .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
        .build()
        .map_err(|error| error.to_string())
}

fn is_release<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config().identifier == crate::IDENTIFIER
}

pub fn enabled<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    if !is_release(app) {
        return Ok(false);
    }
    login_item()?
        .is_enabled()
        .map_err(|error| error.to_string())
}

pub fn set<R: Runtime>(app: &AppHandle<R>, value: bool) -> Result<(), String> {
    if !is_release(app) {
        return Ok(());
    }
    let item = login_item()?;
    if value { item.enable() } else { item.disable() }.map_err(|error| error.to_string())
}
