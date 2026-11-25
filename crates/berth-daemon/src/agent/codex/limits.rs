use berth_core::{env, AgentKind, UsageWindow};
use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const CODEX_TAIL_BYTES: u64 = 256 * 1024;

pub fn windows() -> Vec<UsageWindow> {
    env::home_dir()
        .map(|home| codex_windows(&home))
        .unwrap_or_default()
}

fn codex_windows(home: &Path) -> Vec<UsageWindow> {
    let Some(rollout) = newest_rollout(&home.join(".codex").join("sessions")) else {
        return Vec::new();
    };
    let Ok(mut file) = std::fs::File::open(&rollout) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file
        .seek(SeekFrom::Start(len.saturating_sub(CODEX_TAIL_BYTES)))
        .is_err()
    {
        return Vec::new();
    }
    let mut tail = Vec::new();
    if file.read_to_end(&mut tail).is_err() {
        return Vec::new();
    }
    let tail = String::from_utf8_lossy(&tail);
    tail.lines()
        .rev()
        .filter(|line| line.contains("\"rate_limits\""))
        .find_map(parse_codex)
        .unwrap_or_default()
}

fn newest_rollout(root: &Path) -> Option<PathBuf> {
    let mut dir = root.to_path_buf();
    for _ in 0..3 {
        dir = last_entry(&dir, true)?;
    }
    last_entry(&dir, false)
}

fn last_entry(dir: &Path, want_dir: bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_dir() == want_dir)
                && (want_dir || e.file_name().to_string_lossy().starts_with("rollout-"))
        })
        .map(|e| e.path())
        .max()
}

#[derive(Deserialize)]
struct CodexLine {
    payload: Option<CodexPayload>,
}

#[derive(Deserialize)]
struct CodexPayload {
    rate_limits: Option<CodexLimits>,
}

#[derive(Deserialize)]
struct CodexLimits {
    primary: Option<CodexWindow>,
    secondary: Option<CodexWindow>,
}

#[derive(Deserialize)]
struct CodexWindow {
    used_percent: f32,
    window_minutes: u32,
    resets_at: i64,
}

fn parse_codex(line: &str) -> Option<Vec<UsageWindow>> {
    let limits = serde_json::from_str::<CodexLine>(line)
        .ok()?
        .payload?
        .rate_limits?;
    let windows: Vec<UsageWindow> = [limits.primary, limits.secondary]
        .into_iter()
        .flatten()
        .map(|w| UsageWindow {
            kind: AgentKind::Codex,
            window_minutes: w.window_minutes,
            used_percent: w.used_percent.clamp(0.0, 100.0),
            resets_at_ms: w.resets_at * 1000,
        })
        .collect();
    (!windows.is_empty()).then_some(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_line_reads_primary_and_secondary() {
        let line = r#"{"payload":{"rate_limits":{"primary":{"used_percent":3.0,"window_minutes":300,"resets_at":1796546157},"secondary":{"used_percent":40.0,"window_minutes":10080,"resets_at":1796900000}}}}"#;
        let windows = parse_codex(line).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].window_minutes, 10080);
        assert_eq!(windows[0].resets_at_ms, 1796546157000);
    }
}
