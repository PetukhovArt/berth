use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::ipc::Channel;
use tokio::sync::{mpsc, watch};
use whisper_rs::WhisperContext;

use super::super::{Issue, ModelId, RunEvent};
use super::capture::{self, Captured, SAMPLE_RATE};
use super::models::Failure;
use super::{transcribe, Inner};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Go,
    Stop,
    Cancel,
}

pub struct Request {
    pub wake_phrase: Option<String>,
    pub hands_free: bool,
}

const FRAME: usize = SAMPLE_RATE as usize * 30 / 1000;
const PAUSE: Duration = Duration::from_millis(1600);
const NO_SPEECH: Duration = Duration::from_secs(10);
const HANDS_FREE_LIMIT: Duration = Duration::from_secs(60);
const HOLD_LIMIT: Duration = Duration::from_secs(120);
const PRE_ROLL: Duration = Duration::from_millis(500);
const PARTIAL_EVERY: Duration = Duration::from_millis(1500);
const PARTIAL_LIMIT: Duration = Duration::from_secs(20);
const MIN_LEVEL: f32 = 0.006;
const LEVEL_OVER_NOISE: f32 = 3.0;
const NOISE_WINDOW: usize = 100;

fn samples(duration: Duration) -> usize {
    (duration.as_millis() as usize) * SAMPLE_RATE as usize / 1000
}

enum Outcome {
    Heard(String),
    Cancelled,
}

pub async fn run(
    inner: &Arc<Inner>,
    request: Request,
    sink: &Channel<RunEvent>,
    mut signal: watch::Receiver<Signal>,
) {
    let event = match drive(inner, &request, sink, &mut signal).await {
        Ok(Outcome::Heard(text)) => RunEvent::Finished { text },
        Ok(Outcome::Cancelled) => return,
        Err(_) if *signal.borrow() == Signal::Cancel => return,
        Err((issue, message)) => RunEvent::Failed { issue, message },
    };
    let _ = sink.send(event);
}

async fn interrupted(signal: &mut watch::Receiver<Signal>) -> Signal {
    loop {
        let now = *signal.borrow_and_update();
        if now != Signal::Go {
            return now;
        }
        if signal.changed().await.is_err() {
            return Signal::Cancel;
        }
    }
}

async fn drive(
    inner: &Arc<Inner>,
    request: &Request,
    sink: &Channel<RunEvent>,
    signal: &mut watch::Receiver<Signal>,
) -> Result<Outcome, Failure> {
    let mut needed = vec![ModelId::Dictation];
    if request.wake_phrase.is_some() {
        needed.push(ModelId::Wake);
    }
    if needed.iter().any(|id| !inner.models.ready(*id)) {
        let _ = sink.send(RunEvent::Preparing);
    }
    for id in needed {
        tokio::select! {
            ready = inner.ensure(id) => ready?,
            now = interrupted(signal) => return halted(now),
        }
    }
    let whisper = tokio::select! {
        loaded = inner.whisper() => loaded?,
        now = interrupted(signal) => return halted(now),
    };
    let spotter = match request.wake_phrase.as_deref() {
        Some(phrase) => Some(tokio::select! {
            loaded = inner.spotter(phrase) => loaded?,
            now = interrupted(signal) => return halted(now),
        }),
        None => None,
    };
    let (capture, mut audio) = tokio::task::spawn_blocking(capture::open)
        .await
        .map_err(|error| (Issue::Failed, error.to_string()))??;
    let _ = sink.send(RunEvent::Listening);
    if let Some(spotter) = spotter.as_deref() {
        let listener = spotter.listener();
        loop {
            tokio::select! {
                chunk = audio.recv() => match chunk {
                    Some(Captured::Samples(samples)) => {
                        if listener.heard(&samples) {
                            break;
                        }
                    }
                    Some(Captured::Lost(message)) => return Err((Issue::Failed, message)),
                    None => return Err((Issue::Failed, "the microphone stopped".to_owned())),
                },
                now = interrupted(signal) => return match now {
                    Signal::Stop => Ok(Outcome::Heard(String::new())),
                    _ => Ok(Outcome::Cancelled),
                },
            }
        }
        let _ = sink.send(RunEvent::Woke);
    }
    let hands_free = request.hands_free || spotter.is_some();
    let mut recorder = Recorder::new(hands_free);
    let (partial_tx, mut partial_rx) = mpsc::unbounded_channel::<String>();
    let mut partial: Option<Arc<AtomicBool>> = None;
    let mut ticker = tokio::time::interval(PARTIAL_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let outcome = loop {
        tokio::select! {
            chunk = audio.recv() => match chunk {
                Some(Captured::Samples(samples)) => {
                    if recorder.push(&samples) {
                        let _ = sink.send(RunEvent::Speech);
                    }
                    if recorder.done() {
                        break None;
                    }
                }
                Some(Captured::Lost(message)) => break Some(Err((Issue::Failed, message))),
                None => break Some(Err((Issue::Failed, "the microphone stopped".to_owned()))),
            },
            Some(text) = partial_rx.recv() => {
                partial = None;
                let _ = sink.send(RunEvent::Partial { text });
            }
            _ = ticker.tick() => {
                if partial.is_none() && recorder.heard() && recorder.len() < samples(PARTIAL_LIMIT) {
                    partial = Some(spawn_partial(&whisper, recorder.audio().to_vec(), partial_tx.clone()));
                }
            }
            now = interrupted(signal) => match now {
                Signal::Stop => break None,
                _ => break Some(Ok(Outcome::Cancelled)),
            },
        }
    };
    drop(capture);
    if let Some(flag) = partial {
        flag.store(true, Ordering::Relaxed);
    }
    if let Some(outcome) = outcome {
        return outcome;
    }
    if !recorder.heard() {
        return Ok(Outcome::Heard(String::new()));
    }
    let clip = recorder.into_clip();
    let finished =
        tokio::task::spawn_blocking(move || transcribe::transcribe(&whisper, &clip, None));
    tokio::select! {
        text = finished => text
            .map_err(|error| (Issue::Failed, error.to_string()))?
            .map(Outcome::Heard)
            .map_err(|message| (Issue::Failed, message)),
        Signal::Cancel = interrupted(signal) => Ok(Outcome::Cancelled),
    }
}

fn halted(now: Signal) -> Result<Outcome, Failure> {
    match now {
        Signal::Stop => Err((
            Issue::Loading,
            "the speech model is still loading".to_owned(),
        )),
        _ => Ok(Outcome::Cancelled),
    }
}

fn spawn_partial(
    whisper: &Arc<WhisperContext>,
    audio: Vec<f32>,
    out: mpsc::UnboundedSender<String>,
) -> Arc<AtomicBool> {
    let abort = Arc::new(AtomicBool::new(false));
    let flag = abort.clone();
    let whisper = whisper.clone();
    tokio::task::spawn_blocking(move || {
        if let Ok(text) = transcribe::transcribe(&whisper, &audio, Some(flag.clone())) {
            if !flag.load(Ordering::Relaxed) {
                let _ = out.send(text);
            }
        }
    });
    abort
}

struct Recorder {
    hands_free: bool,
    audio: Vec<f32>,
    pending: Vec<f32>,
    levels: VecDeque<f32>,
    first_voice: Option<usize>,
    last_voice: usize,
}

impl Recorder {
    fn new(hands_free: bool) -> Self {
        Self {
            hands_free,
            audio: Vec::new(),
            pending: Vec::new(),
            levels: VecDeque::from([MIN_LEVEL / LEVEL_OVER_NOISE]),
            first_voice: None,
            last_voice: 0,
        }
    }

    fn push(&mut self, samples: &[f32]) -> bool {
        let was_quiet = self.first_voice.is_none();
        self.pending.extend_from_slice(samples);
        let frames = self.pending.len() / FRAME;
        for frame in self.pending.chunks_exact(FRAME).take(frames) {
            let level = (frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt();
            if self.levels.len() == NOISE_WINDOW {
                self.levels.pop_front();
            }
            self.levels.push_back(level);
            let noise = self.levels.iter().copied().fold(f32::MAX, f32::min);
            let voiced = level > MIN_LEVEL.max(noise * LEVEL_OVER_NOISE);
            self.audio.extend_from_slice(frame);
            if voiced {
                self.first_voice.get_or_insert(self.audio.len() - FRAME);
                self.last_voice = self.audio.len();
            }
        }
        self.pending.drain(..frames * FRAME);
        was_quiet && self.first_voice.is_some()
    }

    fn heard(&self) -> bool {
        self.first_voice.is_some()
    }

    fn len(&self) -> usize {
        self.audio.len()
    }

    fn audio(&self) -> &[f32] {
        &self.audio
    }

    fn done(&self) -> bool {
        let len = self.audio.len();
        if !self.hands_free {
            return len >= samples(HOLD_LIMIT);
        }
        if len >= samples(HANDS_FREE_LIMIT) {
            return true;
        }
        match self.first_voice {
            Some(_) => len - self.last_voice >= samples(PAUSE),
            None => len >= samples(NO_SPEECH),
        }
    }

    fn into_clip(mut self) -> Vec<f32> {
        if !self.hands_free {
            return self.audio;
        }
        let start = self
            .first_voice
            .map_or(0, |first| first.saturating_sub(samples(PRE_ROLL)));
        let end = (self.last_voice + samples(PRE_ROLL)).min(self.audio.len());
        self.audio.truncate(end);
        self.audio.split_off(start)
    }
}
