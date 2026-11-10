//! Names the user gives a conversation inside the agent CLI (Claude Code's
//! and Codex's `/rename`), adopted as the session's name.
//!
//! Neither CLI tells anyone about a rename: no hook fires. Each leaves it on
//! disk instead, so the daemon reads it there — on the hooks that do fire
//! and on the metrics sampler's tick — and [`Store::adopt_cli_title`]
//! decides whether it is news. Only reads: the files are the CLI's.
//!
//! A terminal gets the same for a Claude Code the user started in it, so
//! the session's stored name stays the one name every client shows.
//!
//! Cursor's CLI has no rename, so it has nothing here.
//!
//! [`Store::adopt_cli_title`]: crate::store::Store::adopt_cli_title

use crate::registry::{sanitize_title, Daemon};
use anyhow::{Context, Result};
use berth_core::{AgentId, AgentKind, Entity, ServerEvent, SessionRef, TerminalId};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// Claude Code keeps the current `/rename` beside the transcript, in
/// `<transcript stem>/custom-title.json`, rewritten on every rename. The
/// transcript's own `custom-title` lines say the same; `crate::usage` reads
/// those for the Claude versions that predate this file.
pub fn claude_title(transcript: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct File {
        #[serde(rename = "customTitle")]
        custom_title: Option<String>,
    }
    let text =
        std::fs::read_to_string(transcript.with_extension("").join("custom-title.json")).ok()?;
    let file: File = serde_json::from_str(&text).ok()?;
    file.custom_title.filter(|t| !t.trim().is_empty())
}

/// Codex appends one `{"id", "thread_name", "updated_at"}` line to
/// `$CODEX_HOME/session_index.jsonl` per rename, so the last line for an
/// id is its name.
pub fn parse_codex_index(text: &str) -> HashMap<String, String> {
    #[derive(Deserialize)]
    struct Line {
        id: String,
        thread_name: Option<String>,
    }
    let mut names = HashMap::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        match line.thread_name.filter(|t| !t.trim().is_empty()) {
            Some(name) => names.insert(line.id, name),
            None => names.remove(&line.id),
        };
    }
    names
}

fn codex_index_path() -> PathBuf {
    crate::hooks::installer::codex_home().join("session_index.jsonl")
}

/// The Codex index, re-read only when the file changed.
#[derive(Default)]
pub struct CodexIndex {
    stamp: Option<(SystemTime, u64)>,
    names: HashMap<String, String>,
}

impl CodexIndex {
    pub fn title(&mut self, session_id: &str) -> Option<String> {
        let path = codex_index_path();
        let stamp = std::fs::metadata(&path)
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        if stamp != self.stamp {
            self.names = std::fs::read_to_string(&path)
                .map(|text| parse_codex_index(&text))
                .unwrap_or_default();
            self.stamp = stamp;
        }
        self.names.get(session_id).cloned()
    }
}

/// The name the CLI holds for one conversation, from whatever a hook
/// reported about it. Used on the hook path, where it must answer before
/// the auto-title instruction goes out.
pub fn lookup(transcript: Option<&str>, session_id: Option<&str>) -> Option<String> {
    transcript
        .and_then(|t| claude_title(Path::new(t)))
        .or_else(|| session_id.and_then(|sid| CodexIndex::default().title(sid)))
}

/// One live agent, as much as finding its CLI name needs.
pub struct Probe {
    agent: AgentId,
    kind: AgentKind,
    session_id: String,
    transcript: Option<PathBuf>,
}

/// The live agents worth probing. Point lookups, as `crate::sampler` does
/// for transcripts.
pub fn probes(daemon: &Daemon, live: &[SessionRef]) -> Vec<Probe> {
    live.iter()
        .filter_map(|sref| match sref {
            SessionRef::Agent(id) => Some(id),
            SessionRef::Terminal(_) => None,
        })
        .filter_map(|id| {
            let agent = daemon.store.get_agent(id).ok().flatten()?;
            if !matches!(agent.kind, AgentKind::Claude | AgentKind::Codex) {
                return None;
            }
            Some(Probe {
                agent: id.clone(),
                kind: agent.kind,
                session_id: agent.session_id?,
                transcript: daemon.transcript_path(id),
            })
        })
        .collect()
}

/// Blocking: reads the CLIs' files.
pub fn titles(probes: &[Probe], codex: &mut CodexIndex) -> Vec<(AgentId, String)> {
    probes
        .iter()
        .filter_map(|p| {
            let title = match p.kind {
                AgentKind::Claude => p.transcript.as_deref().and_then(claude_title),
                AgentKind::Codex => codex.title(&p.session_id),
                _ => None,
            }?;
            Some((p.agent.clone(), title))
        })
        .collect()
}

/// Where a running Claude Code keeps `<pid>.json`, its live record.
pub fn claude_sessions_dir() -> PathBuf {
    berth_core::env::non_empty("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            berth_core::env::home_dir()
                .unwrap_or_default()
                .join(".claude")
        })
        .join("sessions")
}

/// A `/rename` in a Claude Code process's live record: `name` with
/// `nameSource: "user"`, and `nameSince`, when it was given. A name Claude
/// derived itself is not one.
fn claude_pid_title(dir: &Path, pid: u32) -> Option<(u64, String)> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        name: Option<String>,
        name_source: Option<String>,
        name_since: Option<u64>,
    }
    let file: File =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{pid}.json"))).ok()?).ok()?;
    if file.name_source.as_deref() != Some("user") {
        return None;
    }
    let name = file.name.filter(|n| !n.trim().is_empty())?;
    Some((file.name_since.unwrap_or(0), name))
}

/// Blocking. The `/rename` of a Claude Code a user started by hand inside a
/// terminal, found through the terminal's process subtree in `table`; the
/// newest one when several run there. Codex keeps no record by pid, so a
/// Codex in a terminal is not found.
pub fn terminal_titles(
    table: &str,
    terminals: &[(TerminalId, u32)],
    dir: &Path,
) -> Vec<(TerminalId, String)> {
    newest_per_terminal(table, terminals, dir, claude_pid_title)
}

/// The conversation of a Claude Code record: `sessionId`, ordered by
/// `startedAt`.
fn claude_pid_session(dir: &Path, pid: u32) -> Option<(u64, String)> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        session_id: Option<String>,
        started_at: Option<u64>,
    }
    let file: File =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{pid}.json"))).ok()?).ok()?;
    let session_id = file.session_id.filter(|id| !id.trim().is_empty())?;
    Some((file.started_at.unwrap_or(0), session_id))
}

/// Blocking. The conversation of the Claude Code a user started by hand
/// inside a terminal, the newest one when several run there.
pub fn terminal_sessions(
    table: &str,
    terminals: &[(TerminalId, u32)],
    dir: &Path,
) -> Vec<(TerminalId, String)> {
    newest_per_terminal(table, terminals, dir, claude_pid_session)
}

fn newest_per_terminal(
    table: &str,
    terminals: &[(TerminalId, u32)],
    dir: &Path,
    read: fn(&Path, u32) -> Option<(u64, String)>,
) -> Vec<(TerminalId, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let recorded: HashSet<u32> = entries
        .filter_map(|e| {
            let name = e.ok()?.file_name();
            name.to_str()?.strip_suffix(".json")?.parse().ok()
        })
        .collect();
    if recorded.is_empty() {
        return Vec::new();
    }
    terminals
        .iter()
        .filter_map(|(id, root)| {
            let (_, value) = crate::metrics::subtree_pids(table, *root)
                .into_iter()
                .filter(|pid| recorded.contains(pid))
                .filter_map(|pid| read(dir, pid))
                .max_by_key(|(order, _)| *order)?;
            Some((id.clone(), value))
        })
        .collect()
}

impl Daemon {
    /// [`Self::adopt_cli_title`] for a terminal.
    pub fn adopt_terminal_cli_title(self: &Arc<Self>, id: &TerminalId, raw: &str) -> Result<bool> {
        let title = sanitize_title(raw);
        if title.is_empty() || !self.store.adopt_terminal_cli_title(id, &title)? {
            return Ok(false);
        }
        tracing::info!(terminal = %id, title = %title, "adopted the CLI's session name");
        let mut term = self.store.get_terminal(id)?.context("terminal not found")?;
        term.alive = self.is_alive(&SessionRef::Terminal(id.clone()));
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Terminal(term),
        });
        Ok(true)
    }

    /// Record the conversation a terminal runs and tell every client.
    pub fn record_terminal_session(
        self: &Arc<Self>,
        id: &TerminalId,
        session_id: &str,
    ) -> Result<bool> {
        if !self.store.record_terminal_session(id, session_id)? {
            return Ok(false);
        }
        let mut term = self.store.get_terminal(id)?.context("terminal not found")?;
        term.alive = self.is_alive(&SessionRef::Terminal(id.clone()));
        self.broadcast(ServerEvent::EntityUpserted {
            entity: Entity::Terminal(term),
        });
        Ok(true)
    }

    /// Take a name the agent CLI reports, if it is a new one (see
    /// [`crate::store::Store::adopt_cli_title`]), and tell every client.
    pub fn adopt_cli_title(self: &Arc<Self>, id: &AgentId, raw: &str) -> Result<bool> {
        let title = sanitize_title(raw);
        if title.is_empty() || !self.store.adopt_cli_title(id, &title)? {
            return Ok(false);
        }
        tracing::info!(agent = %id, title = %title, "adopted the CLI's session name");
        self.broadcast_agent(id)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_index_takes_the_last_name_per_thread() {
        let text = concat!(
            r#"{"id":"a","thread_name":"first","updated_at":"2026-11-07T10:00:00Z"}"#,
            "\n",
            r#"{"id":"b","thread_name":"other","updated_at":"2026-11-07T10:00:01Z"}"#,
            "\n",
            "not json\n",
            r#"{"id":"a","thread_name":"second","updated_at":"2026-11-07T10:00:02Z"}"#,
            "\n",
            r#"{"id":"b","thread_name":"","updated_at":"2026-11-07T10:00:03Z"}"#,
            "\n",
        );
        let names = parse_codex_index(text);
        assert_eq!(names.get("a").map(String::as_str), Some("second"));
        assert_eq!(names.get("b"), None);
    }

    #[test]
    fn terminal_title_is_the_newest_user_name_in_its_subtree() {
        let tmp = tempfile::tempdir().unwrap();
        let record = |pid: u32, json: &str| {
            std::fs::write(tmp.path().join(format!("{pid}.json")), json).unwrap()
        };
        record(
            30,
            r#"{"pid":30,"name":"libraries-2","nameSource":"user","nameSince":200,"formerNames":[{"name":"scada"}]}"#,
        );
        record(
            31,
            r#"{"pid":31,"name":"older","nameSource":"user","nameSince":100}"#,
        );
        record(
            40,
            r#"{"pid":40,"name":"frontend-c6","nameSource":"derived","nameSince":300}"#,
        );
        record(
            50,
            r#"{"pid":50,"name":"elsewhere","nameSource":"user","nameSince":900}"#,
        );
        let table = "10 1 0 0\n20 10 0 0\n30 20 0 0\n31 10 0 0\n11 1 0 0\n40 11 0 0\n50 1 0 0\n";
        let terminals = [(TerminalId("a".into()), 10), (TerminalId("b".into()), 11)];
        assert_eq!(
            terminal_titles(table, &terminals, tmp.path()),
            vec![(TerminalId("a".into()), "libraries-2".to_string())]
        );
    }

    #[test]
    fn terminal_session_is_the_newest_claude_in_its_subtree() {
        let tmp = tempfile::tempdir().unwrap();
        let record = |pid: u32, json: &str| {
            std::fs::write(tmp.path().join(format!("{pid}.json")), json).unwrap()
        };
        record(20, r#"{"pid":20,"sessionId":"old","startedAt":100}"#);
        record(30, r#"{"pid":30,"sessionId":"new","startedAt":200}"#);
        record(50, r#"{"pid":50,"sessionId":"elsewhere","startedAt":900}"#);
        let table = "10 1 0 0\n20 10 0 0\n30 20 0 0\n50 1 0 0\n";
        let terminals = [(TerminalId("a".into()), 10)];
        assert_eq!(
            terminal_sessions(table, &terminals, tmp.path()),
            vec![(TerminalId("a".into()), "new".to_string())]
        );
    }

    #[test]
    fn claude_title_reads_the_file_beside_the_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let transcript = tmp.path().join("bbc696c0.jsonl");
        std::fs::write(&transcript, "").unwrap();
        assert_eq!(claude_title(&transcript), None);
        let dir = tmp.path().join("bbc696c0");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("custom-title.json"),
            r#"{"customTitle":"review-angles"}"#,
        )
        .unwrap();
        assert_eq!(claude_title(&transcript).as_deref(), Some("review-angles"));
        std::fs::write(dir.join("custom-title.json"), r#"{"customTitle":"  "}"#).unwrap();
        assert_eq!(claude_title(&transcript), None);
    }
}
