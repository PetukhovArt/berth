use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Wry};

use crate::tray;

const ACTION_PREFIX: &str = "action:";
const ACTION_EVENT: &str = "menu://action";
const PRODUCT: &str = "Berth";

fn action(
    app: &AppHandle,
    id: &str,
    text: &str,
    accelerator: Option<&str>,
) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, format!("{ACTION_PREFIX}{id}"), text, true, accelerator)
}

fn app_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let about = AboutMetadata {
        name: Some(PRODUCT.to_owned()),
        ..AboutMetadata::default()
    };
    let menu = Submenu::with_items(
        app,
        PRODUCT,
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Berth"), Some(about))?,
            &PredefinedMenuItem::separator(app)?,
            &action(app, "nav.settings", "Settings…", Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    if crate::platform::SELF_UPDATE {
        menu.insert(&action(app, "app.update", "Update Berth…", None)?, 2)?;
    }
    Ok(menu)
}

fn edit_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )
}

fn window_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::fullscreen(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    if !crate::platform::WINDOW_CHROME.native_menu {
        return Ok(());
    }
    let menu = Menu::with_items(
        app,
        &[
            &app_submenu(app)?,
            &edit_submenu(app)?,
            &window_submenu(app)?,
        ],
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if let Some(id) = event.id().as_ref().strip_prefix(ACTION_PREFIX) {
            tray::show_main(app);
            let _ = app.emit(ACTION_EVENT, id);
        }
    });
    Ok(())
}
