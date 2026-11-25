use crate::agent;
use berth_core::{AgentKind, UsageWindow};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CACHE_FOR: Duration = Duration::from_secs(60);
pub(crate) const WEEK: u32 = 10080;

static CACHE: Mutex<Option<(Instant, Vec<UsageWindow>)>> = Mutex::new(None);

pub fn cached() -> Vec<UsageWindow> {
    let now = now_ms();
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|(_, windows)| {
            windows
                .iter()
                .filter(|w| w.resets_at_ms > now)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub fn read() -> Vec<UsageWindow> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, windows)) = cache.as_ref() {
        if at.elapsed() < CACHE_FOR {
            return windows.clone();
        }
    }
    let now = now_ms();
    let config = crate::config::Config::load();
    let mut windows: Vec<UsageWindow> = AgentKind::NATIVE
        .into_iter()
        .filter_map(|kind| agent::of(kind).limits.as_ref())
        .filter(|limits| (limits.enabled)(&config))
        .flat_map(|limits| (limits.windows)())
        .collect();
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

pub(crate) fn fetch(url: &str, headers: &[String]) -> Option<String> {
    let mut command = Command::new("curl");
    command
        .args(["-sS", "--fail", "--max-time", "8", "-K", "-", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let mut child = command.spawn().ok()?;
    let config: String = headers
        .iter()
        .map(|header| format!("header = \"{header}\"\n"))
        .collect();
    child.stdin.take()?.write_all(config.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(crate) fn parse_rfc3339_ms(text: &str) -> Option<i64> {
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
}
