use berth_core::platform::NoWindow;
use berth_core::{Agent, ProviderError};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const TRANSCRIPT_HEAD_BYTES: u64 = 256 * 1024;
const TRANSCRIPT_TAIL_BYTES: u64 = 512 * 1024;
const TASK_MAX_CHARS: usize = 1500;
const TURN_MAX_CHARS: usize = 500;
const RECENT_TURNS: usize = 3;
const GIT_MAX_CHARS: usize = 1500;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Conversation {
    pub first_prompt: Option<String>,
    pub recent: Vec<String>,
}

pub struct Handoff<'a> {
    pub old: &'a Agent,
    pub task_note: Option<String>,
    pub conversation: Conversation,
    pub git: Option<String>,
    pub transcript: Option<PathBuf>,
    pub error: Option<&'a ProviderError>,
    pub reason: &'a str,
}

pub fn flat(text: &str, max: usize) -> String {
    let joined = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let cleaned: String = joined
        .chars()
        .map(|c| match c {
            '"' | '`' => '\'',
            '%' | '^' | '!' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let mut out: String = cleaned.chars().take(max).collect();
    if cleaned.chars().count() > max {
        out.push_str(" ...");
    }
    out
}

fn message_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter(|p| {
                matches!(
                    p["type"].as_str(),
                    Some("text" | "input_text" | "output_text")
                )
            })
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn turn(line: &serde_json::Value) -> Option<(bool, String)> {
    let (role, content) = match line["type"].as_str() {
        Some(role @ ("user" | "assistant")) => (role, &line["message"]["content"]),
        Some("response_item") if line["payload"]["type"] == "message" => (
            line["payload"]["role"].as_str()?,
            &line["payload"]["content"],
        ),
        _ => return None,
    };
    let text = message_text(content);
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') {
        return None;
    }
    match role {
        "user" => Some((true, text.to_string())),
        "assistant" => Some((false, text.to_string())),
        _ => None,
    }
}

pub fn conversation_from_lines(head: &str, tail: &str) -> Conversation {
    let parse = |text: &str| -> Vec<(bool, String)> {
        text.lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| turn(&v))
            .collect()
    };
    let first_prompt = parse(head)
        .into_iter()
        .find(|(user, _)| *user)
        .map(|(_, text)| text);
    let mut recent = parse(tail)
        .into_iter()
        .rev()
        .filter(|(user, _)| !*user)
        .take(RECENT_TURNS)
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    recent.reverse();
    Conversation {
        first_prompt,
        recent,
    }
}

fn read_range(file: &mut std::fs::File, start: u64, len: u64) -> Option<String> {
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.take(len).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

pub fn conversation(path: &Path) -> Conversation {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Conversation::default();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let head = read_range(&mut file, 0, TRANSCRIPT_HEAD_BYTES).unwrap_or_default();
    let tail_start = len.saturating_sub(TRANSCRIPT_TAIL_BYTES);
    let tail = read_range(&mut file, tail_start, TRANSCRIPT_TAIL_BYTES).unwrap_or_default();
    conversation_from_lines(&head, &tail)
}

async fn git_out(dir: &Path, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .no_window()
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn git_state(dir: &Path) -> Option<String> {
    let status = git_out(dir, &["status", "--short"]).await?;
    if status.is_empty() {
        return Some("clean, nothing uncommitted".into());
    }
    let stat = git_out(dir, &["diff", "HEAD", "--shortstat"])
        .await
        .unwrap_or_default();
    Some(format!("{stat}; files: {status}"))
}

pub fn handoff_prompt(h: &Handoff) -> String {
    let program = h.old.kind.cli_program();
    let mut parts = vec![format!(
        "[berth] Continue the task the {program} agent '{}' was working on in this worktree ({}).",
        flat(&h.old.name, 80),
        h.reason
    )];
    if let Some(e) = h.error {
        parts.push(format!("It stopped on: {}.", flat(&e.message, 200)));
    }
    let task = h
        .task_note
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .or(h.conversation.first_prompt.as_deref());
    if let Some(task) = task {
        parts.push(format!("TASK: {}", flat(task, TASK_MAX_CHARS)));
    }
    if !h.conversation.recent.is_empty() {
        let recent = h
            .conversation
            .recent
            .iter()
            .map(|t| flat(t, TURN_MAX_CHARS))
            .collect::<Vec<_>>()
            .join(" || ");
        parts.push(format!("ITS LAST REPLIES: {recent}"));
    }
    if let Some(git) = &h.git {
        parts.push(format!("GIT STATE: {}", flat(git, GIT_MAX_CHARS)));
    }
    match (&h.transcript, h.old.session_id.as_deref()) {
        (Some(path), _) => parts.push(format!(
            "FULL CONVERSATION: {} (read its tail if you need more).",
            path.display()
        )),
        (None, Some(sid)) => parts.push(format!("FULL CONVERSATION: {program} session {sid}.")),
        (None, None) => {}
    }
    parts.push(
        "Run git status and git diff to see what is already done, keep the work that is there, \
then carry on from where it stopped. Do not redo finished steps."
            .into(),
    );
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_core::{AgentId, AgentKind, AgentStatus, ProviderErrorKind, WorktreeId};

    fn agent() -> Agent {
        Agent {
            id: AgentId("a".into()),
            worktree_id: WorktreeId("w".into()),
            name: "fix \"login\"".into(),
            status: AgentStatus::Finished,
            archived: false,
            archived_at: 0,
            unseen: false,
            status_changed_at: 0,
            kind: AgentKind::Claude,
            model: None,
            effort: None,
            session_id: Some("sid".into()),
            cloud_session_id: None,
            sort_order: 0,
            alive: false,
            cloud_mirroring: false,
            suggested_prompt: None,
            provider_error: None,
            routed_from: None,
            route_suggestion: None,
        }
    }

    #[test]
    fn conversation_reads_claude_and_codex_lines() {
        let claude = concat!(
            r#"{"type":"user","message":{"content":"<command-name>x</command-name>"}}"#,
            "\n",
            r#"{"type":"user","message":{"content":"make login work"}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"plan: 1. a 2. b"}]}}"#,
            "\n",
        );
        let c = conversation_from_lines(claude, claude);
        assert_eq!(c.first_prompt.as_deref(), Some("make login work"));
        assert_eq!(c.recent, vec!["plan: 1. a 2. b".to_string()]);
        let codex = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"add tests"}]}}"#;
        assert_eq!(
            conversation_from_lines(codex, codex)
                .first_prompt
                .as_deref(),
            Some("add tests")
        );
    }

    #[test]
    fn prompt_carries_task_replies_and_diff_on_one_line() {
        let old = agent();
        let error = ProviderError {
            kind: ProviderErrorKind::UsageLimit,
            since_ms: 0,
            message: "You've hit your limit".into(),
            resets_at_ms: None,
        };
        let prompt = handoff_prompt(&Handoff {
            old: &old,
            task_note: None,
            conversation: Conversation {
                first_prompt: Some("make login work\nwith 100% coverage".into()),
                recent: vec!["did step 1".into()],
            },
            git: Some(" 1 file changed\n M src/a.rs".into()),
            transcript: None,
            error: Some(&error),
            reason: "usage limit",
        });
        assert!(prompt.contains("TASK: make login work / with 100  coverage"));
        assert!(prompt.contains("ITS LAST REPLIES: did step 1"));
        assert!(prompt.contains("M src/a.rs"));
        assert!(prompt.contains("claude session sid"));
        assert!(!prompt.contains('\n') && !prompt.contains('"'));
    }
}
