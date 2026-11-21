use std::path::PathBuf;
use std::process::Command;
#[cfg(feature = "updater")]
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

use berth_core::platform::NoWindow;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::platform;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
#[cfg_attr(not(feature = "updater"), allow(dead_code))]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Ready {
        version: String,
        notes: Option<String>,
    },
    Installing {
        version: String,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "updater")]
type Package = (tauri_plugin_updater::Update, Arc<Vec<u8>>);

#[derive(Default)]
struct Inner {
    status: Status,
    #[cfg(feature = "updater")]
    ready: Option<Package>,
}

#[derive(Default)]
pub struct ReleaseUpdate {
    inner: Mutex<Inner>,
}

impl ReleaseUpdate {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn status(&self) -> Status {
        self.lock().status.clone()
    }

    #[cfg_attr(not(feature = "updater"), allow(dead_code))]
    fn set(&self, status: Status) -> Status {
        self.lock().status = status.clone();
        status
    }
}

pub fn bundled_cli() -> Option<PathBuf> {
    if !platform::RELEASE_UPDATE {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let cli = exe
        .parent()?
        .join(format!("berth{}", std::env::consts::EXE_SUFFIX));
    cli.is_file().then_some(cli)
}

pub fn adopt_bundled_cli() {
    let Some(dir) = bundled_cli().and_then(|cli| cli.parent().map(PathBuf::from)) else {
        return;
    };
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![dir.clone()];
    entries.extend(std::env::split_paths(&current).filter(|entry| entry != &dir));
    if let Ok(path) = std::env::join_paths(entries) {
        std::env::set_var("PATH", path);
    }
}

pub async fn daemon_update() -> Result<(), String> {
    let cli = bundled_cli().ok_or_else(|| crate::host_text!("bundledCliMissing"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let output = Command::new(&cli)
            .arg("kill")
            .no_window()
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        crate::daemon_start::start()
            .map(drop)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn app_release_status(state: State<'_, ReleaseUpdate>) -> Status {
    state.status()
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
pub async fn app_release_check(
    _app: AppHandle,
    _state: State<'_, ReleaseUpdate>,
) -> Result<Status, String> {
    Err(platform::unsupported())
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
pub async fn app_release_install(
    _app: AppHandle,
    _state: State<'_, ReleaseUpdate>,
) -> Result<(), String> {
    Err(platform::unsupported())
}

#[cfg(feature = "updater")]
#[tauri::command]
pub async fn app_release_check(
    app: AppHandle,
    state: State<'_, ReleaseUpdate>,
) -> Result<Status, String> {
    {
        let mut inner = state.lock();
        if matches!(
            inner.status,
            Status::Checking
                | Status::Downloading { .. }
                | Status::Ready { .. }
                | Status::Installing { .. }
        ) {
            return Ok(inner.status.clone());
        }
        inner.status = Status::Checking;
    }
    match fetch(&app, &state).await {
        Ok(Some((update, bytes))) => {
            let mut inner = state.lock();
            inner.status = ready_status(&update);
            inner.ready = Some((update, bytes));
            Ok(inner.status.clone())
        }
        Ok(None) => Ok(state.set(Status::UpToDate)),
        Err(error) => Ok(state.set(Status::Failed { error })),
    }
}

#[cfg(feature = "updater")]
async fn fetch(app: &AppHandle, state: &ReleaseUpdate) -> Result<Option<Package>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|error| error.to_string())?;
    let Some(update) = updater.check().await.map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    let version = update.version.clone();
    state.set(Status::Downloading {
        version: version.clone(),
        downloaded: 0,
        total: None,
    });
    let mut downloaded = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                state.set(Status::Downloading {
                    version: version.clone(),
                    downloaded,
                    total,
                });
            },
            || {},
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(Some((update, Arc::new(bytes))))
}

#[cfg(feature = "updater")]
#[tauri::command]
pub async fn app_release_install(
    app: AppHandle,
    state: State<'_, ReleaseUpdate>,
) -> Result<(), String> {
    let (update, bytes) = state
        .lock()
        .ready
        .clone()
        .ok_or_else(|| crate::host_text!("noUpdateReady"))?;
    let ready = ready_status(&update);
    state.set(Status::Installing {
        version: update.version.clone(),
    });
    let installed = tauri::async_runtime::spawn_blocking(move || update.install(bytes.as_slice()))
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result.map_err(|error| error.to_string()));
    match installed {
        Ok(()) => app.restart(),
        Err(error) => {
            state.set(ready);
            Err(error)
        }
    }
}

#[cfg(feature = "updater")]
fn ready_status(update: &tauri_plugin_updater::Update) -> Status {
    Status::Ready {
        version: update.version.clone(),
        notes: update.body.clone().filter(|notes| !notes.trim().is_empty()),
    }
}
