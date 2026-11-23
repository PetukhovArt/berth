use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use serde_json::Value;

use super::message::Harness;
use super::reader::{read_tail, records};
use super::text::{content_text, owned, prompt_of, shown_user_text, single_line, valid_id};

const HEAD_LINES: usize = 50;
const HEAD_BYTES: u64 = 512 * 1024;
const TAIL_BYTES: u64 = 64 * 1024;
const TITLE_CHARS: usize = 120;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Last {
    #[default]
    Unknown,
    Running,
    Finished,
}

#[derive(Clone, Debug, Default)]
pub struct Head {
    id: Option<String>,
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub custom: Option<String>,
    pub ai: Option<String>,
    pub summary: Option<String>,
    pub prompt: Option<String>,
    pub automated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Tail {
    pub last: Last,
    pub custom: Option<String>,
    pub ai: Option<String>,
    pub branch: Option<String>,
    pub messages: Option<u32>,
    pub counted: u32,
}

fn claude_head(head: &mut Head, line: &Value) {
    let kind = line["type"].as_str().unwrap_or_default();
    match kind {
        "custom-title" => head.custom = head.custom.take().or_else(|| owned(&line["customTitle"])),
        "ai-title" => head.ai = head.ai.take().or_else(|| owned(&line["aiTitle"])),
        "summary" => head.summary = head.summary.take().or_else(|| owned(&line["summary"])),
        _ => {}
    }
    head.id = head.id.take().or_else(|| owned(&line["sessionId"]));
    head.cwd = head.cwd.take().or_else(|| owned(&line["cwd"]));
    head.branch = head.branch.take().or_else(|| owned(&line["gitBranch"]));
    if line["entrypoint"]
        .as_str()
        .is_some_and(|entry| entry.starts_with("sdk"))
    {
        head.automated = true;
    }
    if kind == "user" && head.prompt.is_none() && line["isMeta"].as_bool() != Some(true) {
        head.prompt = content_text(&line["message"]["content"]).and_then(|text| prompt_of(&text));
    }
}

pub fn renamed_title(harness: Harness, line: &Value) -> Option<String> {
    match harness {
        Harness::Claude if line["type"].as_str() == Some("custom-title") => {
            owned(&line["customTitle"])
        }
        Harness::Codex
            if line["type"].as_str() == Some("event_msg")
                && line["payload"]["type"].as_str() == Some("thread_name_updated") =>
        {
            owned(&line["payload"]["thread_name"])
        }
        _ => None,
    }
}

pub fn title_line(title: &str) -> String {
    single_line(title, TITLE_CHARS)
}

fn claude_tail(tail: &mut Tail, line: &Value) {
    if let Some(branch) = owned(&line["gitBranch"]) {
        tail.branch = Some(branch);
    }
    if let Some(title) = renamed_title(Harness::Claude, line) {
        tail.custom = Some(title);
    }
    match line["type"].as_str().unwrap_or_default() {
        "ai-title" => tail.ai = owned(&line["aiTitle"]).or(tail.ai.take()),
        "system" => match line["subtype"].as_str() {
            Some("turn_duration") => {
                tail.last = Last::Finished;
                if let Some(count) = line["messageCount"].as_u64() {
                    tail.messages = Some(u32::try_from(count).unwrap_or(u32::MAX));
                }
            }
            Some("stop_hook_summary") => tail.last = Last::Finished,
            _ => {}
        },
        "assistant" => {
            tail.counted += 1;
            let blocks = line["message"]["content"].as_array();
            let has = |wanted: &str| {
                blocks.is_some_and(|blocks| {
                    blocks
                        .iter()
                        .any(|block| block["type"].as_str() == Some(wanted))
                })
            };
            if has("tool_use") {
                tail.last = Last::Running;
            } else if has("text") {
                tail.last = Last::Finished;
            }
        }
        "user" if line["isMeta"].as_bool() != Some(true) => {
            tail.counted += 1;
            let content = &line["message"]["content"];
            let result = content.as_array().is_some_and(|blocks| {
                blocks
                    .iter()
                    .any(|block| block["type"].as_str() == Some("tool_result"))
            });
            if result {
                tail.last = Last::Running;
                return;
            }
            let text = content_text(content).unwrap_or_default();
            let text = text.trim_start();
            if text.starts_with("[Request interrupted")
                || text.starts_with("<local-command-stdout>")
            {
                tail.last = Last::Finished;
            } else if !text.starts_with("<command-name>")
                && !text.starts_with("<local-command-caveat>")
            {
                tail.last = Last::Running;
            }
        }
        _ => {}
    }
}

fn codex_head(head: &mut Head, line: &Value) {
    let payload = &line["payload"];
    match (line["type"].as_str(), payload["type"].as_str()) {
        (Some("session_meta"), _) => {
            head.id = head.id.take().or_else(|| owned(&payload["id"]));
            head.cwd = head.cwd.take().or_else(|| owned(&payload["cwd"]));
            head.branch = head
                .branch
                .take()
                .or_else(|| owned(&payload["git"]["branch"]));
            let originator = payload["originator"].as_str().unwrap_or_default();
            if originator.contains("exec") || payload["source"].as_str() == Some("exec") {
                head.automated = true;
            }
        }
        (Some("turn_context"), _) => head.cwd = head.cwd.take().or_else(|| owned(&payload["cwd"])),
        (Some("event_msg"), Some("user_message")) if head.prompt.is_none() => {
            head.prompt = owned(&payload["message"]).and_then(|text| shown_user_text(&text));
        }
        (Some("response_item"), Some("message"))
            if head.prompt.is_none() && payload["role"].as_str() == Some("user") =>
        {
            head.prompt = content_text(&payload["content"]).and_then(|text| shown_user_text(&text));
        }
        _ => {}
    }
}

fn codex_tail(tail: &mut Tail, line: &Value) {
    if let Some(title) = renamed_title(Harness::Codex, line) {
        tail.custom = Some(title);
    }
    let payload = &line["payload"];
    match (line["type"].as_str(), payload["type"].as_str()) {
        (Some("event_msg"), Some("task_complete" | "turn_aborted")) => tail.last = Last::Finished,
        (Some("event_msg"), Some("task_started")) => tail.last = Last::Running,
        (Some("event_msg"), Some("user_message")) => {
            tail.counted += 1;
            tail.last = Last::Running;
        }
        (Some("event_msg"), Some("agent_message")) => {
            tail.counted += 1;
            tail.last = Last::Finished;
        }
        (
            Some("response_item"),
            Some(
                "function_call"
                | "custom_tool_call"
                | "local_shell_call"
                | "web_search_call"
                | "function_call_output"
                | "custom_tool_call_output",
            ),
        ) => tail.last = Last::Running,
        _ => {}
    }
}

fn read_head(path: &Path, harness: Harness) -> Head {
    let mut head = Head::default();
    let Ok(file) = File::open(path) else {
        return head;
    };
    let mut reader = BufReader::new(file.take(HEAD_BYTES));
    let mut line = Vec::new();
    for _ in 0..HEAD_LINES {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        match harness {
            Harness::Claude => claude_head(&mut head, &value),
            Harness::Codex => codex_head(&mut head, &value),
        }
    }
    head
}

pub fn read_tail_facts(path: &Path, harness: Harness) -> Tail {
    let mut tail = Tail::default();
    let Some(window) = read_tail(path, TAIL_BYTES) else {
        return tail;
    };
    for (_, line) in records(&window.buffer, window.base) {
        match harness {
            Harness::Claude => claude_tail(&mut tail, &line),
            Harness::Codex => codex_tail(&mut tail, &line),
        }
    }
    tail
}

pub fn id_of(harness: Harness, path: &Path, head: &Head) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    match harness {
        Harness::Claude => stem,
        Harness::Codex => head
            .id
            .clone()
            .filter(|id| valid_id(id))
            .unwrap_or_else(|| {
                let start = stem.len().saturating_sub(36);
                stem.get(start..).unwrap_or(&stem).to_owned()
            }),
    }
}

fn localize_cwd(head: &mut Head, transcript: &Path) {
    let local = head
        .cwd
        .as_deref()
        .and_then(|cwd| berth_core::claude_store::local_cwd(cwd, transcript));
    if let Some(local) = local {
        head.cwd = Some(local.to_string_lossy().into_owned());
    }
}

pub fn head_of(harness: Harness, path: &Path) -> Head {
    let mut head = read_head(path, harness);
    if harness == Harness::Claude {
        localize_cwd(&mut head, path);
    }
    head
}

pub fn title_of(head: &Head, tail: &Tail, id: &str) -> String {
    [
        &tail.custom,
        &head.custom,
        &tail.ai,
        &head.ai,
        &head.summary,
        &head.prompt,
    ]
    .into_iter()
    .find_map(Option::as_deref)
    .map(|title| single_line(title, TITLE_CHARS))
    .unwrap_or_else(|| id.chars().take(8).collect())
}

pub struct SessionFacts {
    pub id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub branch: Option<String>,
}

pub fn session_facts(harness: Harness, path: &Path) -> SessionFacts {
    let head = head_of(harness, path);
    let title = [&head.custom, &head.ai, &head.summary, &head.prompt]
        .into_iter()
        .find_map(Option::as_deref)
        .map(|title| single_line(title, TITLE_CHARS));
    SessionFacts {
        id: id_of(harness, path, &head),
        title,
        cwd: head.cwd,
        branch: head.branch,
    }
}
