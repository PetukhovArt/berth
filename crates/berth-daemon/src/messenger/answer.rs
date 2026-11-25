use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use berth_core::acp::AcpItem;
use berth_core::{Agent, Backend};
use serde_json::Value;

use crate::agent;
use crate::registry::Daemon;

const TAIL_BYTES: u64 = 512 * 1024;
const SUMMARY_CHARS: usize = 700;
const SPOKEN_CHARS: usize = 400;
const CODE_NOTE: &str = "(code on screen)";

impl Daemon {
    pub(crate) fn last_answer(&self, agent: &Agent) -> Option<String> {
        let text = if agent.backend == Backend::Chat {
            self.acp_transcript(&agent.id, 0)
                .ok()?
                .into_iter()
                .rev()
                .find_map(|at| match at.item {
                    AcpItem::Assistant { text } => Some(text),
                    _ => None,
                })
        } else {
            let last_answer = agent::history(agent.kind)?.last_answer?;
            last_answer(self, agent)
        }?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    fn claude_transcript(&self, agent: &Agent) -> Option<PathBuf> {
        if let Some(path) = self.transcript_path(&agent.id) {
            return Some(path);
        }
        let session = agent.session_id.as_deref()?;
        let cwd = self.store.get_worktree(&agent.worktree_id).ok()??.path;
        crate::orphans::transcript_path(&cwd, session)
            .or_else(|| crate::orphans::find_transcript(session))
    }
}

pub(crate) fn claude(daemon: &Daemon, agent: &Agent) -> Option<String> {
    claude_answer(&tail(&daemon.claude_transcript(agent)?)?)
}

pub(crate) fn codex(_daemon: &Daemon, agent: &Agent) -> Option<String> {
    codex_answer(&tail(&codex_transcript(agent.session_id.as_deref()?)?)?)
}

fn tail(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    Some(match (start > 0, text.find('\n')) {
        (true, Some(cut)) => text[cut + 1..].to_owned(),
        _ => text,
    })
}

fn joined_text(blocks: &Value) -> String {
    match blocks {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter(|block| matches!(block["type"].as_str(), Some("text" | "output_text") | None))
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn claude_answer(lines: &str) -> Option<String> {
    lines
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| {
            line["type"].as_str() == Some("assistant")
                && line["isSidechain"].as_bool() != Some(true)
        })
        .map(|line| joined_text(&line["message"]["content"]))
        .find(|text| !text.trim().is_empty())
}

fn codex_answer(lines: &str) -> Option<String> {
    lines
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| {
            line["type"].as_str() == Some("response_item")
                && line["payload"]["type"].as_str() == Some("message")
                && line["payload"]["role"].as_str() == Some("assistant")
        })
        .map(|line| joined_text(&line["payload"]["content"]))
        .find(|text| !text.trim().is_empty())
}

fn newest_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs
}

fn codex_transcript(session_id: &str) -> Option<PathBuf> {
    if session_id.is_empty() || session_id.contains(['/', '\\', ':']) {
        return None;
    }
    let suffix = format!("{session_id}.jsonl");
    let root = crate::hooks::installer::codex_home().join("sessions");
    for year in newest_dirs(&root) {
        for month in newest_dirs(&year) {
            for day in newest_dirs(&month) {
                let found = std::fs::read_dir(&day)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|entry| entry.path())
                    .find(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.ends_with(&suffix))
                    });
                if found.is_some() {
                    return found;
                }
            }
        }
    }
    None
}

fn without_code_blocks(text: &str) -> String {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if !fenced {
                out.push(CODE_NOTE.to_owned());
            }
            fenced = !fenced;
            continue;
        }
        if !fenced {
            out.push(line.to_owned());
        }
    }
    out.join("\n")
}

fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let head: String = text.chars().take(max).collect();
    let sentence = head
        .rfind(['.', '!', '?', '\n'])
        .filter(|end| *end > head.len() / 2)
        .map(|end| end + 1);
    let word = head.rfind(char::is_whitespace);
    let end = sentence.or(word).unwrap_or(head.len());
    format!("{}…", head[..end].trim_end())
}

pub(crate) fn summary(answer: &str) -> String {
    let text = without_code_blocks(answer);
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() && lines.last().is_some_and(|last| last.trim().is_empty()) {
            continue;
        }
        lines.push(line);
    }
    cut(lines.join("\n").trim(), SUMMARY_CHARS)
}

fn without_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut inside = false;
    for c in line.chars() {
        if c == '`' {
            inside = !inside;
        } else if !inside {
            out.push(c);
        }
    }
    out
}

pub(crate) fn spoken(answer: &str) -> String {
    let text = without_code_blocks(answer);
    let words: Vec<String> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('|'))
        .map(without_inline_code)
        .flat_map(|line| {
            line.split_whitespace()
                .filter(|word| !word.contains("://"))
                .map(|word| {
                    word.trim_matches(|c: char| matches!(c, '#' | '*' | '_' | '>' | '[' | ']'))
                        .to_owned()
                })
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>()
        })
        .collect();
    cut(&words.join(" "), SPOKEN_CHARS)
}
