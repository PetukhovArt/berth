use berth_core::{env, AgentKind, UsageWindow};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{fetch, parse_rfc3339_ms, WEEK};

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const DEFAULT_SCOPE: &str = "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828";
const MONTH: u32 = 43_200;

#[derive(Deserialize)]
struct GrokAuth {
    key: String,
    #[serde(default)]
    auth_mode: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Deserialize)]
struct Billing {
    config: Option<BillingConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BillingConfig {
    credit_usage_percent: Option<f32>,
    current_period: Option<Period>,
}

#[derive(Deserialize)]
struct Period {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    start: Option<String>,
    end: String,
}

fn auth_json_path() -> PathBuf {
    env::non_empty("GROK_AUTH_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::hooks::installer::grok_home().join("auth.json"))
}

fn header_safe(text: &str) -> bool {
    !text.is_empty() && !text.contains(['"', '\\', '\n', '\r'])
}

fn session(text: &str) -> Option<GrokAuth> {
    let mut store = serde_json::from_str::<BTreeMap<String, GrokAuth>>(text).ok()?;
    let usable = |auth: &GrokAuth| {
        auth.auth_mode.as_deref() != Some("api_key")
            && auth.user_id.as_deref().is_some_and(header_safe)
            && header_safe(&auth.key)
    };
    if let Some(auth) = store.remove(DEFAULT_SCOPE).filter(usable) {
        return Some(auth);
    }
    store.into_values().find(usable)
}

pub(super) fn windows() -> Vec<UsageWindow> {
    let Ok(text) = std::fs::read_to_string(auth_json_path()) else {
        return Vec::new();
    };
    let Some(auth) = session(&text) else {
        return Vec::new();
    };
    let headers = [
        format!("Authorization: Bearer {}", auth.key),
        "X-XAI-Token-Auth: xai-grok-cli".to_owned(),
        format!("x-userid: {}", auth.user_id.unwrap_or_default()),
    ];
    fetch(BILLING_URL, &headers)
        .map(|body| parse(&body))
        .unwrap_or_default()
}

fn parse(body: &str) -> Vec<UsageWindow> {
    let Some(config) = serde_json::from_str::<Billing>(body)
        .ok()
        .and_then(|b| b.config)
    else {
        return Vec::new();
    };
    let (Some(used), Some(period)) = (config.credit_usage_percent, config.current_period) else {
        return Vec::new();
    };
    let Some(resets_at_ms) = parse_rfc3339_ms(&period.end) else {
        return Vec::new();
    };
    let window_minutes = match period.kind.as_str() {
        "USAGE_PERIOD_TYPE_WEEKLY" => WEEK,
        "USAGE_PERIOD_TYPE_MONTHLY" => MONTH,
        _ => period
            .start
            .as_deref()
            .and_then(parse_rfc3339_ms)
            .and_then(|start| u32::try_from((resets_at_ms - start) / 60_000).ok())
            .unwrap_or(0),
    };
    if window_minutes == 0 {
        return Vec::new();
    }
    vec![UsageWindow {
        kind: AgentKind::Grok,
        window_minutes,
        used_percent: used.clamp(0.0, 100.0),
        resets_at_ms,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_billing_body_gives_one_grok_window() {
        let body = r#"{"config":{"creditUsagePercent":42.5,"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"1970-01-01T00:00:00Z","end":"1970-01-08T00:00:00Z"}},"subscriptionTier":"SuperGrok"}"#;
        let windows = parse(body);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].kind, AgentKind::Grok);
        assert_eq!(windows[0].window_minutes, WEEK);
        assert_eq!(windows[0].used_percent, 42.5);
        assert_eq!(windows[0].resets_at_ms, 7 * 86_400_000);
    }

    #[test]
    fn api_key_entries_have_no_subscription_usage() {
        let store = r#"{"xai::api_key":{"key":"xai-1","auth_mode":"api_key","user_id":"u"}}"#;
        assert!(session(store).is_none());
        let store = r#"{"xai::api_key":{"key":"xai-1","auth_mode":"api_key"},"https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828":{"key":"jwt","auth_mode":"oidc","user_id":"u1"}}"#;
        assert_eq!(session(store).unwrap().key, "jwt");
    }
}
