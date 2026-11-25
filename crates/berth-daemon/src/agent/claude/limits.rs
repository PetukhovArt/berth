use crate::usage_limits::{fetch, parse_rfc3339_ms, WEEK};
use berth_core::{env, AgentKind, UsageWindow};
use serde::Deserialize;
use std::path::Path;

const FIVE_HOURS: u32 = 300;
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub fn windows() -> Vec<UsageWindow> {
    env::home_dir()
        .map(|home| claude_windows(&home))
        .unwrap_or_default()
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
    let headers = [
        format!("Authorization: Bearer {token}"),
        "anthropic-beta: oauth-2025-04-20".to_owned(),
    ];
    let Some(body) = fetch(USAGE_URL, &headers) else {
        return Vec::new();
    };
    parse_claude(&body)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_body_gives_both_windows_and_skips_null() {
        let body = r#"{"five_hour":{"utilization":72.0,"resets_at":"2025-06-01T14:00:00+00:00"},"seven_day":null}"#;
        let windows = parse_claude(body);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window_minutes, 300);
        assert_eq!(windows[0].used_percent, 72.0);
    }
}
