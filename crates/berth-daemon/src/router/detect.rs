use super::DetectedError;
use crate::hooks::HookPayload;
use berth_core::{AgentKind, ProviderErrorKind};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const SCREEN_TAIL_ROWS: usize = 12;
const SCREEN_TAIL_BYTES: usize = 64 * 1024;
const TRANSCRIPT_TAIL_BYTES: u64 = 64 * 1024;
const MESSAGE_MAX_CHARS: usize = 200;

const COMMON_ANCHORS: &[&str] = &[
    "api error",
    "usage limit",
    "hit your limit",
    "limit reached",
    "rate limit",
    "overloaded",
];

fn anchors(kind: AgentKind) -> &'static [&'static str] {
    match kind {
        AgentKind::Codex => &[
            "stream error",
            "stream disconnected",
            "exceeded retry limit",
            "unexpected status",
        ],
        AgentKind::Claude => &["please run /login", "credit balance is too low"],
        AgentKind::Grok => &["request failed", "server error"],
        _ => &[],
    }
}

pub fn classify_status(status: u64) -> Option<ProviderErrorKind> {
    Some(match status {
        529 => ProviderErrorKind::Overloaded,
        429 => ProviderErrorKind::UsageLimit,
        401 | 403 => ProviderErrorKind::Auth,
        500..=599 => ProviderErrorKind::Server,
        _ => return None,
    })
}

pub fn classify(text: &str) -> Option<ProviderErrorKind> {
    let t = text.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| t.contains(n));
    if has(&["overloaded", "529"]) {
        return Some(ProviderErrorKind::Overloaded);
    }
    if has(&[
        "usage limit",
        "rate limit",
        "rate_limit",
        "hit your limit",
        "limit reached",
        "429",
        "quota",
        "too many requests",
    ]) {
        return Some(ProviderErrorKind::UsageLimit);
    }
    if has(&[
        "401",
        "403",
        "authentication",
        "unauthorized",
        "invalid api key",
        "/login",
        "billing",
        "credit balance",
    ]) {
        return Some(ProviderErrorKind::Auth);
    }
    if has(&[
        "econnreset",
        "connection error",
        "connection refused",
        "network",
        "timed out",
        "timeout",
        "fetch failed",
        "enotfound",
        "stream disconnected",
    ]) {
        return Some(ProviderErrorKind::Network);
    }
    if has(&[
        "internal server error",
        "bad gateway",
        "service unavailable",
        "server_error",
        "server error",
    ]) || (500..=599).any(|code| t.contains(&code.to_string()))
    {
        return Some(ProviderErrorKind::Server);
    }
    None
}

fn clip(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    line.chars().take(MESSAGE_MAX_CHARS).collect()
}

fn value_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn from_stop_failure(payload: &HookPayload) -> DetectedError {
    let text = [&payload.error, &payload.error_details]
        .into_iter()
        .flatten()
        .map(value_text)
        .collect::<Vec<_>>()
        .join(" ");
    DetectedError {
        kind: classify(&text).unwrap_or(ProviderErrorKind::Other),
        message: if text.trim().is_empty() {
            "the turn ended on an API error".into()
        } else {
            clip(&text)
        },
    }
}

pub fn from_screen_text(kind: AgentKind, text: &str) -> Option<DetectedError> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>();
    let tail = &lines[lines.len().saturating_sub(SCREEN_TAIL_ROWS)..];
    tail.iter().rev().find_map(|line| {
        let lower = line.to_lowercase();
        let anchored = COMMON_ANCHORS
            .iter()
            .chain(anchors(kind))
            .any(|a| lower.contains(a));
        if !anchored {
            return None;
        }
        Some(DetectedError {
            kind: classify(line)?,
            message: clip(line),
        })
    })
}

pub fn screen_text(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(SCREEN_TAIL_BYTES);
    let mut parser = vt100::Parser::new(50, 200, 0);
    parser.process(&bytes[start..]);
    parser.screen().contents()
}

pub fn from_transcript_lines(lines: &str) -> Option<DetectedError> {
    let last = lines
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| matches!(v["type"].as_str(), Some("assistant" | "user")))?;
    if last["isApiErrorMessage"].as_bool() != Some(true) {
        return None;
    }
    let text = last["message"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let status = ["apiErrorStatus", "api_error_status"]
        .iter()
        .find_map(|k| last[*k].as_u64());
    let kind = status
        .and_then(classify_status)
        .or_else(|| classify(&text))
        .or_else(|| last["error"].as_str().and_then(classify))
        .unwrap_or(ProviderErrorKind::Other);
    Some(DetectedError {
        kind,
        message: if text.is_empty() {
            "API error".into()
        } else {
            clip(&text)
        },
    })
}

pub fn from_transcript(path: &Path) -> Option<DetectedError> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TRANSCRIPT_TAIL_BYTES)))
        .ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    from_transcript_lines(&String::from_utf8_lossy(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_tail_ending_on_an_api_error_is_detected() {
        let lines = concat!(
            r#"{"type":"user","message":{"content":"go"}}"#,
            "\n",
            r#"{"type":"assistant","isApiErrorMessage":true,"message":{"content":[{"type":"text","text":"API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}"}]}}"#,
            "\n",
        );
        let found = from_transcript_lines(lines).unwrap();
        assert_eq!(found.kind, ProviderErrorKind::Overloaded);
    }

    #[test]
    fn transcript_that_recovered_is_not_an_error() {
        let lines = concat!(
            r#"{"type":"assistant","isApiErrorMessage":true,"message":{"content":[{"type":"text","text":"API Error: 500"}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"done"}]}}"#,
            "\n",
        );
        assert!(from_transcript_lines(lines).is_none());
    }

    #[test]
    fn screen_banners_classify_per_kind() {
        let claude = "some output\n  API Error (529 overloaded) Retrying in 5 seconds\n> ";
        assert_eq!(
            from_screen_text(AgentKind::Claude, claude).unwrap().kind,
            ProviderErrorKind::Overloaded
        );
        let limit = "You've hit your limit \u{b7} resets 5pm (Europe/Moscow)";
        assert_eq!(
            from_screen_text(AgentKind::Claude, limit).unwrap().kind,
            ProviderErrorKind::UsageLimit
        );
        let codex = "stream error: unexpected status 503 Service Unavailable; retrying 3/5";
        assert_eq!(
            from_screen_text(AgentKind::Codex, codex).unwrap().kind,
            ProviderErrorKind::Server
        );
        assert!(from_screen_text(AgentKind::Claude, "compiled 503 files").is_none());
    }

    #[test]
    fn stop_failure_payload_falls_back_to_other() {
        let payload: HookPayload =
            serde_json::from_str(r#"{"error":"rate_limit","error_details":"429"}"#).unwrap();
        assert_eq!(
            from_stop_failure(&payload).kind,
            ProviderErrorKind::UsageLimit
        );
        let empty = HookPayload::default();
        assert_eq!(from_stop_failure(&empty).kind, ProviderErrorKind::Other);
    }
}
