use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, SampleFormat, SizedSample, StreamConfig};
use tokio::sync::mpsc;

use super::super::Issue;
use super::models::Failure;

pub const SAMPLE_RATE: u32 = 16_000;
const PARK: Duration = Duration::from_millis(100);

pub enum Captured {
    Samples(Vec<f32>),
    Lost(String),
}

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

pub fn open() -> Result<(Capture, mpsc::UnboundedReceiver<Captured>), Failure> {
    let (tx, rx) = mpsc::unbounded_channel();
    let (opened_tx, opened_rx) = std_mpsc::channel::<Result<(), Failure>>();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("berth-voice-capture".to_owned())
        .spawn(move || match start(tx) {
            Ok(stream) => {
                let _ = opened_tx.send(Ok(()));
                while !stop_flag.load(Ordering::Relaxed) {
                    std::thread::park_timeout(PARK);
                }
                drop(stream);
            }
            Err(failure) => {
                let _ = opened_tx.send(Err(failure));
            }
        })
        .map_err(|error| (Issue::Failed, error.to_string()))?;
    let capture = Capture {
        stop,
        thread: Some(thread),
    };
    match opened_rx.recv() {
        Ok(Ok(())) => Ok((capture, rx)),
        Ok(Err(failure)) => Err(failure),
        Err(_) => Err((Issue::Failed, "the microphone thread stopped".to_owned())),
    }
}

fn classify(error: cpal::Error) -> Failure {
    let issue = match error.kind() {
        ErrorKind::PermissionDenied => Issue::Denied,
        _ => Issue::Failed,
    };
    (issue, error.to_string())
}

fn start(tx: mpsc::UnboundedSender<Captured>) -> Result<cpal::Stream, Failure> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| (Issue::Failed, "no microphone found".to_owned()))?;
    let supported = device.default_input_config().map_err(classify)?;
    let format = supported.sample_format();
    let config = supported.config();
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, tx),
        SampleFormat::I16 => build::<i16>(&device, config, tx),
        SampleFormat::U16 => build::<u16>(&device, config, tx),
        SampleFormat::I32 => build::<i32>(&device, config, tx),
        SampleFormat::I8 => build::<i8>(&device, config, tx),
        SampleFormat::U8 => build::<u8>(&device, config, tx),
        SampleFormat::F64 => build::<f64>(&device, config, tx),
        other => {
            return Err((
                Issue::Failed,
                format!("the microphone delivers {other:?} samples"),
            ))
        }
    }?;
    stream.play().map_err(classify)?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    tx: mpsc::UnboundedSender<Captured>,
) -> Result<cpal::Stream, Failure>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels.max(1));
    let mut resampler = Resampler::new(config.sample_rate);
    let lost = tx.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| {
                        frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>()
                            / frame.len() as f32
                    })
                    .collect();
                let mut out = Vec::with_capacity(mono.len());
                resampler.push(&mono, &mut out);
                if !out.is_empty() {
                    let _ = tx.send(Captured::Samples(out));
                }
            },
            move |error: cpal::Error| {
                if matches!(
                    error.kind(),
                    ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated
                ) {
                    let _ = lost.send(Captured::Lost(error.to_string()));
                }
            },
            None,
        )
        .map_err(classify)
}

struct Resampler {
    step: f64,
    pos: f64,
    prev: f32,
}

impl Resampler {
    fn new(rate: u32) -> Self {
        Self {
            step: f64::from(rate) / f64::from(SAMPLE_RATE),
            pos: 1.0,
            prev: 0.0,
        }
    }

    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let Some(&last) = input.last() else {
            return;
        };
        let prev = self.prev;
        let at = |i: usize| if i == 0 { prev } else { input[i - 1] };
        let span = input.len() as f64;
        while self.pos < span {
            let i = self.pos.floor() as usize;
            let frac = (self.pos - i as f64) as f32;
            let (a, b) = (at(i), at(i + 1));
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.pos -= span;
        self.prev = last;
    }
}
