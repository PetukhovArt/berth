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
    "selected model",
    "model is not supported",
    "unknown model",
    "couldn't set model",
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
    if has(&[
        "issue with the selected model",
        "model is not supported",
        "unknown model",
        "couldn't set model",
        "model_not_found",
        "model not found",
    ]) {
        return Some(ProviderErrorKind::Model);
    }
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

fn unit_secs(word: &str) -> Option<i64> {
    let w = word.trim_end_matches(|c: char| !c.is_alphabetic());
    Some(match w {
        "s" | "sec" | "secs" | "second" | "seconds" => 1,
        "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
        "d" | "day" | "days" => 86400,
        _ => return None,
    })
}

pub fn retry_after_secs(text: &str) -> Option<i64> {
    let t = text.to_lowercase();
    let at = [
        "try again in",
        "retrying in",
        "retry in",
        "retry after",
        "retry-after:",
    ]
    .iter()
    .find_map(|k| t.find(k).map(|i| i + k.len()))?;
    let mut tokens = Vec::new();
    for word in t[at..].split_whitespace().take(8) {
        let digits = word.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 {
            tokens.push(word[..digits].to_string());
            if digits < word.len() {
                tokens.push(word[digits..].to_string());
            }
        } else {
            tokens.push(word.to_string());
        }
    }
    let mut total = 0;
    let mut found = false;
    let mut i = 0;
    while i < tokens.len() {
        let Ok(n) = tokens[i].parse::<i64>() else {
            if found && tokens[i] != "and" {
                break;
            }
            i += 1;
            continue;
        };
        match tokens.get(i + 1).and_then(|u| unit_secs(u)) {
            Some(unit) => {
                total += n * unit;
                found = true;
                i += 2;
            }
            None if !found && t[at..].trim_start().starts_with(&tokens[i]) => {
                return Some(n);
            }
            None => break,
        }
    }
    found.then_some(total)
}

fn detected(kind: ProviderErrorKind, message: String) -> DetectedError {
    let resets_at_ms = retry_after_secs(&message).map(|s| super::now_ms() + s * 1000);
    DetectedError {
        kind,
        message,
        resets_at_ms,
    }
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
    detected(
        classify(&text).unwrap_or(ProviderErrorKind::Other),
        if text.trim().is_empty() {
            "the turn ended on an API error".into()
        } else {
            clip(&text)
        },
    )
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
        Some(detected(classify(line)?, clip(line)))
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
    Some(detected(
        kind,
        if text.is_empty() {
            "API error".into()
        } else {
            clip(&text)
        },
    ))
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
    fn real_bad_model_lines_classify_as_model() {
        let codex = "ERROR: The 'no-such-model-xyz' model is not supported when using Codex with a ChatGPT account.";
        assert_eq!(
            from_screen_text(AgentKind::Codex, codex).unwrap().kind,
            ProviderErrorKind::Model
        );
        let grok = "Couldn't set model 'x': Invalid params: \"unknown model id\". Run 'grok models' to see available models.";
        assert_eq!(
            from_screen_text(AgentKind::Grok, grok).unwrap().kind,
            ProviderErrorKind::Model
        );
        let claude = "There's an issue with the selected model (x). It may not exist or you may not have access to it.";
        assert_eq!(
            from_screen_text(AgentKind::Claude, claude).unwrap().kind,
            ProviderErrorKind::Model
        );
    }

    #[test]
    fn retry_after_reads_relative_durations() {
        assert_eq!(
            retry_after_secs("API Error (529) Retrying in 5 seconds"),
            Some(5)
        );
        assert_eq!(
            retry_after_secs("Rate limit reached. Try again in 2 hours 3 minutes."),
            Some(7380)
        );
        assert_eq!(retry_after_secs("try again in 1h 30m"), Some(5400));
        assert_eq!(retry_after_secs("retry-after: 30"), Some(30));
        assert_eq!(retry_after_secs("resets 5pm"), None);
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
