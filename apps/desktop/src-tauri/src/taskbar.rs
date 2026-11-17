use tauri::{AppHandle, Manager, UserAttentionType};

const MAIN_WINDOW: &str = "main";

pub fn sync(app: &AppHandle, previous: u32, waiting: u32) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    if waiting > previous && !window.is_focused().unwrap_or(false) {
        let _ = window.request_user_attention(Some(UserAttentionType::Informational));
    } else if waiting == 0 {
        let _ = window.request_user_attention(None);
    }
}
