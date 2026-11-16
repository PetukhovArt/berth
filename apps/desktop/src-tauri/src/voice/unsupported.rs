use std::path::PathBuf;

use tauri::ipc::Channel;
use tauri::AppHandle;

use super::{RunEvent, VoiceModels};

#[derive(Clone)]
pub struct Engine;

impl Engine {
    pub fn new(_app: AppHandle) -> Self {
        Self
    }

    pub fn models(&self) -> VoiceModels {
        VoiceModels {
            supported: false,
            dir: None,
            models: Vec::new(),
        }
    }

    pub fn listen(
        &self,
        _wake_phrase: Option<String>,
        _hands_free: bool,
        _sink: Channel<RunEvent>,
    ) -> Result<u32, String> {
        Err(crate::platform::UNSUPPORTED.to_owned())
    }

    pub fn stop(&self, _run: u32) {}

    pub fn cancel(&self, _run: u32) {}

    pub async fn transcribe_file(&self, _pcm: PathBuf) -> Result<String, String> {
        Err(crate::platform::UNSUPPORTED.to_owned())
    }

    pub fn remove_models(&self) -> Result<VoiceModels, String> {
        Ok(self.models())
    }
}
