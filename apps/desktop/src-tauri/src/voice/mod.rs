#![cfg_attr(
    not(all(feature = "voice", any(windows, target_os = "macos"))),
    allow(dead_code)
)]

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

#[cfg(all(feature = "voice", any(windows, target_os = "macos")))]
#[path = "native/mod.rs"]
mod engine;

#[cfg(not(all(feature = "voice", any(windows, target_os = "macos"))))]
#[path = "unsupported.rs"]
mod engine;

pub use engine::Engine as Voice;

pub const MODELS_EVENT: &str = "voice-models";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelId {
    Dictation,
    Wake,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelState {
    pub id: ModelId,
    pub ready: bool,
    pub bytes: u64,
    pub received: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModels {
    pub supported: bool,
    pub dir: Option<String>,
    pub models: Vec<ModelState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Issue {
    Denied,
    Offline,
    Loading,
    Phrase,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RunEvent {
    Preparing,
    Listening,
    Woke,
    Speech,
    Partial { text: String },
    Finished { text: String },
    Failed { issue: Issue, message: String },
}

#[tauri::command]
pub fn voice_models(voice: State<'_, Voice>) -> VoiceModels {
    voice.models()
}

#[tauri::command]
pub fn voice_listen(
    voice: State<'_, Voice>,
    wake_phrase: Option<String>,
    hands_free: bool,
    on_event: Channel<RunEvent>,
) -> Result<u32, String> {
    voice.listen(wake_phrase, hands_free, on_event)
}

#[tauri::command]
pub fn voice_stop(voice: State<'_, Voice>, run: u32) {
    voice.stop(run);
}

#[tauri::command]
pub fn voice_cancel(voice: State<'_, Voice>, run: u32) {
    voice.cancel(run);
}

#[tauri::command]
pub fn voice_remove_models(voice: State<'_, Voice>) -> Result<VoiceModels, String> {
    voice.remove_models()
}
