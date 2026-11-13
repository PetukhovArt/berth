use berth_core::{env, AgentKind, UsageWindow};
use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CACHE_FOR: Duration = Duration::from_secs(60);
const CODEX_TAIL_BYTES: u64 = 256 * 1024;
const FIVE_HOURS: u32 = 300;
const WEEK: u32 = 10080;
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

static CACHE: Mutex<Option<(Instant, Vec<UsageWindow>)>> = Mutex::new(None);

pub fn read() -> Vec<UsageWindow> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, windows)) = cache.as_ref() {
        if at.elapsed() < CACHE_FOR {
            return windows.clone();
        }
    }
    let now = now_ms();
    let mut windows = Vec::new();
    if let Some(home) = env::home_dir() {
        windows.extend(claude_windows(&home));
        windows.extend(codex_windows(&home));
    }
    windows.retain(|w| w.resets_at_ms > now);
    *cache = Some((Instant::now(), windows.clone()));
    windows
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OauthToken>,
}

#[derive(Deserialize)]
struct OauthToken {
    #[serde(rename = "accessToken")]
    access_token: String,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    five_hour: Option<ClaudeWindow>,
    seven_day: Option<ClaudeWindow>,
}

#[derive(Deserialize)]
struct ClaudeWindow {
    utilization: f32,
    resets_at: String,
}

fn claude_windows(home: &Path) -> Vec<UsageWindow> {
    let Ok(text) = std::fs::read_to_string(home.join(".claude").join(".credentials.json")) else {
        return Vec::new();
    };
    let Some(token) = serde_json::from_str::<Credentials>(&text)
        .ok()
        .and_then(|c| c.oauth)
        .map(|o| o.access_token)
        .filter(|t| !t.contains(['"', '\\', '\n', '\r']))
    else {
        return Vec::new();
    };
    let Some(body) = fetch_usage(&token) else {
        return Vec::new();
    };
    parse_claude(&body)
}

fn fetch_usage(token: &str) -> Option<String> {
    let mut command = Command::new("curl");
    command
        .args(["-sS", "--fail", "--max-time", "8", "-K", "-", USAGE_URL])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let mut child = command.spawn().ok()?;
    let config = format!(
        "header = \"Authorization: Bearer {token}\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\n"
    );
    child.stdin.take()?.write_all(config.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse_claude(body: &str) -> Vec<UsageWindow> {
    let Ok(usage) = serde_json::from_str::<ClaudeUsage>(body) else {
        return Vec::new();
    };
    [(usage.five_hour, FIVE_HOURS), (usage.seven_day, WEEK)]
        .into_iter()
        .filter_map(|(window, minutes)| {
            let window = window?;
            Some(UsageWindow {
                kind: AgentKind::Claude,
                window_minutes: minutes,
                used_percent: window.utilization.clamp(0.0, 100.0),
                resets_at_ms: parse_rfc3339_ms(&window.resets_at)?,
            })
        })
        .collect()
}

fn parse_rfc3339_ms(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once('T')?;
    let mut d = date.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let zone_at = rest.find(['Z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(zone_at);
    let mut c = clock.split(':');
    let hour: i64 = c.next()?.parse().ok()?;
    let minute: i64 = c.next()?.parse().ok()?;
    let seconds: f64 = c.next().unwrap_or("0").parse().ok()?;
    let offset_minutes = match zone.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let (h, m) = zone[1..].split_once(':').unwrap_or((&zone[1..], "0"));
            let total = h.parse::<i64>().ok()? * 60 + m.parse::<i64>().ok()?;
            if sign == '-' {
                -total
            } else {
                total
            }
        }
        _ => 0,
    };
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + hour * 3600 + minute * 60 - offset_minutes * 60;
    Some(secs * 1000 + (seconds * 1000.0).round() as i64)
}

fn codex_windows(home: &Path) -> Vec<UsageWindow> {
    let Some(rollout) = newest_rollout(&home.join(".codex").join("sessions")) else {
        return Vec::new();
    };
    let Ok(mut file) = std::fs::File::open(&rollout) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file.seek(SeekFrom::Start(len.saturating_sub(CODEX_TAIL_BYTES))).is_err() {
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
    fn rfc3339_with_offset_and_fraction() {
        assert_eq!(parse_rfc3339_ms("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(
            parse_rfc3339_ms("2025-06-01T14:00:00.500+02:00"),
            Some(1_748_779_200_500)
        );
    }

    #[test]
    fn claude_body_gives_both_windows_and_skips_null() {
        let body = r#"{"five_hour":{"utilization":72.0,"resets_at":"2025-06-01T14:00:00+00:00"},"seven_day":null}"#;
        let windows = parse_claude(body);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window_minutes, 300);
        assert_eq!(windows[0].used_percent, 72.0);
    }

    #[test]
    fn codex_line_reads_primary_and_secondary() {
        let line = r#"{"payload":{"rate_limits":{"primary":{"used_percent":3.0,"window_minutes":300,"resets_at":1796546157},"secondary":{"used_percent":40.0,"window_minutes":10080,"resets_at":1796900000}}}}"#;
        let windows = parse_codex(line).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].window_minutes, 10080);
        assert_eq!(windows[0].resets_at_ms, 1796546157000);
    }
}
