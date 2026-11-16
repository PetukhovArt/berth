use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use super::super::{Issue, ModelId, ModelState};

pub type Failure = (Issue, String);

struct Spec {
    id: ModelId,
    url: &'static str,
    sha256: &'static str,
    bytes: u64,
}

const DICTATION: Spec = Spec {
    id: ModelId::Dictation,
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
    sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    bytes: 147_951_465,
};

const WAKE: Spec = Spec {
    id: ModelId::Wake,
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2",
    sha256: "f170013b4716e41b62b9bfd809687c207cef798ef9bc6534d524e17af9b6561a",
    bytes: 17_626_723,
};

const DICTATION_FILE: &str = "ggml-base.bin";
const WAKE_DIR: &str = "kws";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub struct WakeFiles {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
}

impl WakeFiles {
    fn in_dir(dir: &Path) -> Self {
        Self {
            encoder: dir.join("encoder.onnx"),
            decoder: dir.join("decoder.onnx"),
            joiner: dir.join("joiner.onnx"),
            tokens: dir.join("tokens.txt"),
        }
    }

    fn all(&self) -> [&PathBuf; 4] {
        [&self.encoder, &self.decoder, &self.joiner, &self.tokens]
    }
}

#[derive(Clone, Debug)]
enum Progress {
    Running(u64),
    Done,
    Failed(Issue, String),
}

pub struct Models {
    dir: Option<PathBuf>,
    downloads: Mutex<HashMap<ModelId, watch::Receiver<Progress>>>,
    errors: Mutex<HashMap<ModelId, String>>,
}

impl Models {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            downloads: Mutex::new(HashMap::new()),
            errors: Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    fn root(&self) -> Result<&Path, Failure> {
        self.dir.as_deref().ok_or_else(|| {
            (
                Issue::Failed,
                "no app data directory for speech models".to_owned(),
            )
        })
    }

    pub fn dictation_path(&self) -> Result<PathBuf, Failure> {
        Ok(self.root()?.join(DICTATION_FILE))
    }

    pub fn wake_files(&self) -> Result<WakeFiles, Failure> {
        Ok(WakeFiles::in_dir(&self.root()?.join(WAKE_DIR)))
    }

    pub fn ready(&self, id: ModelId) -> bool {
        match id {
            ModelId::Dictation => self.dictation_path().is_ok_and(|path| path.is_file()),
            ModelId::Wake => self
                .wake_files()
                .is_ok_and(|files| files.all().iter().all(|path| path.is_file())),
        }
    }

    fn on_disk(&self, id: ModelId) -> u64 {
        let paths: Vec<PathBuf> = match id {
            ModelId::Dictation => self.dictation_path().into_iter().collect(),
            ModelId::Wake => self
                .wake_files()
                .map(|files| files.all().into_iter().cloned().collect())
                .unwrap_or_default(),
        };
        paths
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|meta| meta.len())
            .sum()
    }

    pub fn states(&self) -> Vec<ModelState> {
        let downloads = self.downloads.lock().unwrap_or_else(|e| e.into_inner());
        let errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        [DICTATION, WAKE]
            .iter()
            .map(|spec| {
                let ready = self.ready(spec.id);
                let received = downloads.get(&spec.id).and_then(|rx| match *rx.borrow() {
                    Progress::Running(received) => Some(received),
                    _ => None,
                });
                ModelState {
                    id: spec.id,
                    ready,
                    bytes: if ready {
                        self.on_disk(spec.id)
                    } else {
                        spec.bytes
                    },
                    received,
                    error: errors.get(&spec.id).cloned(),
                }
            })
            .collect()
    }

    pub fn downloading(&self) -> bool {
        let downloads = self.downloads.lock().unwrap_or_else(|e| e.into_inner());
        downloads
            .values()
            .any(|rx| matches!(*rx.borrow(), Progress::Running(_)))
    }

    pub async fn ensure(
        &self,
        id: ModelId,
        on_progress: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), Failure> {
        if self.ready(id) {
            return Ok(());
        }
        let mut rx = self.download(id, on_progress)?;
        loop {
            match rx.borrow_and_update().clone() {
                Progress::Done => return Ok(()),
                Progress::Failed(issue, message) => return Err((issue, message)),
                Progress::Running(_) => {}
            }
            if rx.changed().await.is_err() {
                return Err((Issue::Failed, "the download stopped".to_owned()));
            }
        }
    }

    fn download(
        &self,
        id: ModelId,
        on_progress: impl Fn() + Send + Sync + 'static,
    ) -> Result<watch::Receiver<Progress>, Failure> {
        let mut downloads = self.downloads.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rx) = downloads.get(&id) {
            if matches!(*rx.borrow(), Progress::Running(_)) {
                return Ok(rx.clone());
            }
        }
        let root = self.root()?.to_path_buf();
        let spec = match id {
            ModelId::Dictation => DICTATION,
            ModelId::Wake => WAKE,
        };
        let (tx, rx) = watch::channel(Progress::Running(0));
        downloads.insert(id, rx.clone());
        self.errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
        tauri::async_runtime::spawn(async move {
            on_progress();
            let outcome = fetch(&spec, &root, &tx, &on_progress).await;
            tx.send_replace(match outcome {
                Ok(()) => Progress::Done,
                Err((issue, message)) => Progress::Failed(issue, message),
            });
            on_progress();
        });
        Ok(rx)
    }

    pub fn settle(&self) {
        let mut downloads = self.downloads.lock().unwrap_or_else(|e| e.into_inner());
        let mut errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        downloads.retain(|id, rx| match &*rx.borrow() {
            Progress::Running(_) => true,
            Progress::Done => false,
            Progress::Failed(_, message) => {
                errors.insert(*id, message.clone());
                false
            }
        });
    }

    pub fn remove(&self) -> Result<(), String> {
        if self.downloading() {
            return Err("a speech model is still downloading".to_owned());
        }
        self.errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let Some(dir) = self.dir.as_deref() else {
            return Ok(());
        };
        match fs::remove_dir_all(dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("{}: {error}", dir.display())),
        }
    }
}

fn transport(error: reqwest::Error) -> Failure {
    (Issue::Offline, error.to_string())
}

fn failed(error: impl std::fmt::Display) -> Failure {
    (Issue::Failed, error.to_string())
}

async fn fetch(
    spec: &Spec,
    root: &Path,
    progress: &watch::Sender<Progress>,
    on_progress: &(impl Fn() + Send + Sync),
) -> Result<(), Failure> {
    tokio::fs::create_dir_all(root).await.map_err(failed)?;
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(failed)?;
    let mut response = client.get(spec.url).send().await.map_err(transport)?;
    if !response.status().is_success() {
        return Err(failed(format!(
            "{} answered {}",
            spec.url,
            response.status()
        )));
    }
    let part = root.join(format!("{}.part", file_stem(spec.id)));
    let mut file = tokio::fs::File::create(&part).await.map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut shown = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(failed)?;
        received += chunk.len() as u64;
        progress.send_replace(Progress::Running(received));
        if received * 100 / spec.bytes.max(1) > shown {
            shown = received * 100 / spec.bytes.max(1);
            on_progress();
        }
    }
    file.flush().await.map_err(failed)?;
    drop(file);
    let digest = format!("{:x}", hasher.finalize());
    if digest != spec.sha256 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(failed(format!("{} does not match its checksum", spec.url)));
    }
    let root = root.to_path_buf();
    let id = spec.id;
    tokio::task::spawn_blocking(move || install(id, &part, &root))
        .await
        .map_err(failed)?
}

fn file_stem(id: ModelId) -> &'static str {
    match id {
        ModelId::Dictation => DICTATION_FILE,
        ModelId::Wake => WAKE_DIR,
    }
}

fn install(id: ModelId, part: &Path, root: &Path) -> Result<(), Failure> {
    match id {
        ModelId::Dictation => fs::rename(part, root.join(DICTATION_FILE)).map_err(failed),
        ModelId::Wake => {
            let staging = root.join(format!("{WAKE_DIR}.new"));
            let _ = fs::remove_dir_all(&staging);
            fs::create_dir_all(&staging).map_err(failed)?;
            let unpacked = unpack_wake(part, &WakeFiles::in_dir(&staging));
            let _ = fs::remove_file(part);
            unpacked?;
            let target = root.join(WAKE_DIR);
            let _ = fs::remove_dir_all(&target);
            fs::rename(&staging, &target).map_err(failed)
        }
    }
}

fn wake_target<'a>(name: &str, files: &'a WakeFiles) -> Option<&'a PathBuf> {
    let int8 = name.ends_with(".int8.onnx");
    if name == "tokens.txt" {
        Some(&files.tokens)
    } else if name.starts_with("encoder-") && int8 {
        Some(&files.encoder)
    } else if name.starts_with("decoder-") && name.ends_with(".onnx") && !int8 {
        Some(&files.decoder)
    } else if name.starts_with("joiner-") && int8 {
        Some(&files.joiner)
    } else {
        None
    }
}

fn unpack_wake(archive: &Path, files: &WakeFiles) -> Result<(), Failure> {
    let reader = bzip2::read::BzDecoder::new(fs::File::open(archive).map_err(failed)?);
    let mut tar = tar::Archive::new(reader);
    for entry in tar.entries().map_err(failed)? {
        let mut entry = entry.map_err(failed)?;
        let path = entry.path().map_err(failed)?.into_owned();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(target) = wake_target(name, files) else {
            continue;
        };
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(failed)?;
        fs::write(target, bytes).map_err(failed)?;
    }
    match files.all().iter().find(|path| !path.is_file()) {
        Some(missing) => Err(failed(format!(
            "the wake word model has no {}",
            missing.display()
        ))),
        None => Ok(()),
    }
}
