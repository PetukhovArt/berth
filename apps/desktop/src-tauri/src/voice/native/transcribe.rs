use std::borrow::Cow;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::capture::SAMPLE_RATE;

const MIN_SAMPLES: usize = SAMPLE_RATE as usize * 11 / 10;
const MAX_THREADS: usize = 8;

static QUIET: Once = Once::new();

pub fn load(path: &Path) -> Result<WhisperContext, String> {
    QUIET.call_once(whisper_rs::install_logging_hooks);
    let path = path
        .to_str()
        .ok_or_else(|| format!("{} is not a valid path", path.display()))?;
    WhisperContext::new_with_params(path, WhisperContextParameters::default())
        .map_err(|error| format!("could not load the speech model: {error}"))
}

pub fn transcribe(
    context: &WhisperContext,
    samples: &[f32],
    abort: Option<Arc<AtomicBool>>,
) -> Result<String, String> {
    let mut state = context.create_state().map_err(|error| error.to_string())?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(MAX_THREADS);
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("auto"));
    params.set_n_threads(threads as i32);
    params.set_translate(false);
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if let Some(flag) = abort {
        params.set_abort_callback_safe(move || flag.load(Ordering::Relaxed));
    }
    let audio: Cow<'_, [f32]> = if samples.len() < MIN_SAMPLES {
        let mut padded = samples.to_vec();
        padded.resize(MIN_SAMPLES, 0.0);
        Cow::Owned(padded)
    } else {
        Cow::Borrowed(samples)
    };
    state
        .full(params, &audio)
        .map_err(|error| error.to_string())?;
    let text: Vec<String> = state
        .as_iter()
        .filter_map(|segment| segment.to_str_lossy().ok().map(|text| text.into_owned()))
        .collect();
    Ok(clean(&text.join(" ")))
}

fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
