use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use berth_core::platform::{self, NoWindow};
use tokio::process::Command;

use super::telegram::Upload;

const TOOL_TIMEOUT: Duration = Duration::from_secs(90);
const ERROR_CHARS: usize = 300;

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("berth-chat-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        Ok(Self(dir))
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn ffmpeg() -> Option<PathBuf> {
    platform::resolve_program("ffmpeg")
}

async fn run(command: &mut Command, what: &str) -> Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_window();
    let output = tokio::time::timeout(TOOL_TIMEOUT, command.output())
        .await
        .with_context(|| format!("{what} took longer than {} s", TOOL_TIMEOUT.as_secs()))?
        .with_context(|| format!("run {what}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr
            .trim()
            .chars()
            .rev()
            .take(ERROR_CHARS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        bail!("{what} failed: {tail}");
    }
    Ok(())
}

pub(crate) async fn decode(ffmpeg: &Path, input: &Path, pcm: &Path) -> Result<()> {
    run(
        Command::new(ffmpeg)
            .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(input)
            .args(["-ac", "1", "-ar", "16000", "-f", "f32le"])
            .arg(pcm),
        "ffmpeg",
    )
    .await
}

fn language(text: &str) -> &'static str {
    if text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
        "ru"
    } else {
        "en"
    }
}

pub(crate) async fn speak(text: &str, scratch: &Scratch) -> Option<Upload> {
    let text_file = scratch.path("reply.txt");
    std::fs::write(&text_file, text).ok()?;
    let (command, spoken) = platform::speech_to_file(&text_file, language(text))?;
    if let Err(e) = run(&mut Command::from(command), "text to speech").await {
        tracing::warn!(error = %e, "chat bridge could not speak the answer");
        return None;
    }
    if let Some(ffmpeg) = ffmpeg() {
        let ogg = scratch.path("reply.ogg");
        let encoded = run(
            Command::new(ffmpeg)
                .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&spoken)
                .args(["-ac", "1", "-c:a", "libopus", "-b:a", "32k"])
                .arg(&ogg),
            "ffmpeg",
        )
        .await;
        match encoded {
            Ok(()) => return Some(Upload::Voice(ogg)),
            Err(e) => tracing::warn!(error = %e, "chat bridge could not encode the voice answer"),
        }
    }
    match spoken.extension().and_then(|ext| ext.to_str()) {
        Some("m4a" | "mp3") => Some(Upload::Audio(spoken)),
        _ => None,
    }
}
