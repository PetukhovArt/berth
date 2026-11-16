mod capture;
mod models;
mod run;
mod transcribe;
mod wake;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use whisper_rs::WhisperContext;

use super::{Issue, ModelId, RunEvent, VoiceModels, MODELS_EVENT};
use models::{Failure, Models};
use run::Signal;
use wake::Spotter;

const MODEL_DIR: &str = "voice";
const UNLOAD_AFTER: Duration = Duration::from_secs(10 * 60);

pub struct Engine {
    inner: Arc<Inner>,
}

struct Inner {
    app: AppHandle,
    models: Models,
    active: Mutex<HashMap<u32, watch::Sender<Signal>>>,
    next_run: AtomicU32,
    idle: AtomicU64,
    whisper: Mutex<Option<Arc<WhisperContext>>>,
    spotter: Mutex<Option<Arc<Spotter>>>,
}

impl Engine {
    pub fn new(app: AppHandle) -> Self {
        let dir = app
            .path()
            .app_local_data_dir()
            .ok()
            .map(|dir| dir.join(MODEL_DIR));
        Self {
            inner: Arc::new(Inner {
                app,
                models: Models::new(dir),
                active: Mutex::new(HashMap::new()),
                next_run: AtomicU32::new(1),
                idle: AtomicU64::new(0),
                whisper: Mutex::new(None),
                spotter: Mutex::new(None),
            }),
        }
    }

    pub fn models(&self) -> VoiceModels {
        self.inner.snapshot()
    }

    pub fn listen(
        &self,
        wake_phrase: Option<String>,
        hands_free: bool,
        sink: Channel<RunEvent>,
    ) -> Result<u32, String> {
        let id = self.inner.next_run.fetch_add(1, Ordering::Relaxed);
        let (signal, rx) = watch::channel(Signal::Go);
        self.inner
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, signal);
        self.inner.idle.fetch_add(1, Ordering::Relaxed);
        let request = run::Request {
            wake_phrase: wake_phrase
                .map(|phrase| phrase.trim().to_owned())
                .filter(|phrase| !phrase.is_empty()),
            hands_free,
        };
        let inner = self.inner.clone();
        tauri::async_runtime::spawn(async move {
            run::run(&inner, request, &sink, rx).await;
            inner.finish(id);
        });
        Ok(id)
    }

    pub fn stop(&self, run: u32) {
        self.inner.signal(run, Signal::Stop);
    }

    pub fn cancel(&self, run: u32) {
        self.inner.signal(run, Signal::Cancel);
    }

    pub fn remove_models(&self) -> Result<VoiceModels, String> {
        let active = self.inner.active.lock().unwrap_or_else(|e| e.into_inner());
        if !active.is_empty() {
            return Err("voice input is listening: turn it off first".to_owned());
        }
        self.inner.unload();
        self.inner.models.remove()?;
        self.inner.publish();
        Ok(self.inner.snapshot())
    }
}

impl Inner {
    fn snapshot(&self) -> VoiceModels {
        self.models.settle();
        VoiceModels {
            supported: true,
            dir: self
                .models
                .dir()
                .map(|dir| dir.to_string_lossy().into_owned()),
            models: self.models.states(),
        }
    }

    fn publish(&self) {
        let _ = self.app.emit(MODELS_EVENT, self.snapshot());
    }

    fn signal(&self, run: u32, signal: Signal) {
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = active.get(&run) {
            sender.send_replace(signal);
        }
    }

    fn finish(self: &Arc<Self>, run: u32) {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&run);
        let ticket = self.idle.fetch_add(1, Ordering::Relaxed) + 1;
        let inner = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(UNLOAD_AFTER).await;
            let idle = inner.idle.load(Ordering::Relaxed) == ticket;
            let quiet = inner
                .active
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty();
            if idle && quiet {
                inner.unload();
            }
        });
    }

    fn unload(&self) {
        self.whisper
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        self.spotter
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }

    async fn ensure(self: &Arc<Self>, id: ModelId) -> Result<(), Failure> {
        let inner = self.clone();
        let outcome = self.models.ensure(id, move || inner.publish()).await;
        self.publish();
        outcome
    }

    async fn whisper(&self) -> Result<Arc<WhisperContext>, Failure> {
        if let Some(context) = self
            .whisper
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Ok(context);
        }
        let path = self.models.dictation_path()?;
        let context = tokio::task::spawn_blocking(move || transcribe::load(&path))
            .await
            .map_err(|error| (Issue::Failed, error.to_string()))?
            .map_err(|message| (Issue::Failed, message))?;
        let context = Arc::new(context);
        *self.whisper.lock().unwrap_or_else(|e| e.into_inner()) = Some(context.clone());
        Ok(context)
    }

    async fn spotter(&self, phrase: &str) -> Result<Arc<Spotter>, Failure> {
        if let Some(spotter) = self
            .spotter
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .filter(|spotter| spotter.phrase() == phrase)
        {
            return Ok(spotter);
        }
        let files = self.models.wake_files()?;
        let phrase = phrase.to_owned();
        let spotter = tokio::task::spawn_blocking(move || Spotter::load(&files, &phrase))
            .await
            .map_err(|error| (Issue::Failed, error.to_string()))??;
        let spotter = Arc::new(spotter);
        *self.spotter.lock().unwrap_or_else(|e| e.into_inner()) = Some(spotter.clone());
        Ok(spotter)
    }
}
