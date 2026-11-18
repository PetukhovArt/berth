use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use berth_core::protocol::SessionRef;
use berth_core::TerminalId;
use serde::{Deserialize, Serialize};

use crate::pty::PtySession;
use crate::registry::Daemon;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LiveSession {
    ClaudeTerminal {
        terminal: TerminalId,
        claude_session_id: String,
    },
    Session(SessionRef),
}

impl LiveSession {
    pub fn session_ref(&self) -> SessionRef {
        match self {
            Self::ClaudeTerminal { terminal, .. } => SessionRef::Terminal(terminal.clone()),
            Self::Session(sref) => sref.clone(),
        }
    }

    pub fn claude_session_id(&self) -> Option<&str> {
        match self {
            Self::ClaudeTerminal {
                claude_session_id, ..
            } => Some(claude_session_id),
            Self::Session(_) => None,
        }
    }
}

pub fn recorded(
    live: Vec<SessionRef>,
    running_claude: &[(TerminalId, String)],
) -> Vec<LiveSession> {
    live.into_iter()
        .map(|sref| match &sref {
            SessionRef::Terminal(id) => running_claude
                .iter()
                .find(|(terminal, _)| terminal == id)
                .map(|(terminal, session_id)| LiveSession::ClaudeTerminal {
                    terminal: terminal.clone(),
                    claude_session_id: session_id.clone(),
                })
                .unwrap_or(LiveSession::Session(sref)),
            SessionRef::Agent(_) => LiveSession::Session(sref),
        })
        .collect()
}

fn resume_command(session_id: &str) -> Option<String> {
    let typeable = !session_id.is_empty()
        && session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    typeable.then(|| format!("claude --resume {session_id}\r"))
}

static EXPECTED_CLAUDE: Mutex<Option<HashMap<TerminalId, String>>> = Mutex::new(None);

pub fn expect_claude(live: &[LiveSession]) {
    let expected: HashMap<TerminalId, String> = live
        .iter()
        .filter_map(|entry| match entry {
            LiveSession::ClaudeTerminal {
                terminal,
                claude_session_id,
            } => Some((terminal.clone(), claude_session_id.clone())),
            LiveSession::Session(_) => None,
        })
        .collect();
    *EXPECTED_CLAUDE.lock().unwrap_or_else(|e| e.into_inner()) = Some(expected);
}

fn take_expected_claude(terminal: &TerminalId) -> Option<String> {
    EXPECTED_CLAUDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()?
        .remove(terminal)
}

pub fn resume_expected_claude(terminal: &TerminalId, shell: &PtySession) {
    let Some(session_id) = take_expected_claude(terminal) else {
        return;
    };
    let Some(command) = resume_command(&session_id) else {
        return;
    };
    match shell.write_input(command.as_bytes()) {
        Ok(()) => {
            tracing::info!(terminal = %terminal, session_id, "resuming Claude in the restored terminal")
        }
        Err(e) => {
            tracing::warn!(terminal = %terminal, error = %e, "resuming Claude in the restored terminal failed")
        }
    }
}

fn claude_pid_conversation(dir: &Path, pid: u32) -> Option<(u64, (String, PathBuf))> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        session_id: Option<String>,
        started_at: Option<u64>,
        cwd: Option<PathBuf>,
    }
    let file: File =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{pid}.json"))).ok()?).ok()?;
    let session_id = file.session_id.filter(|id| !id.trim().is_empty())?;
    Some((file.started_at.unwrap_or(0), (session_id, file.cwd?)))
}

fn resumes_from(claude_cwd: &Path, terminal_dir: &Path) -> bool {
    let claude_cwd = berth_core::paths::canonical_or_raw(claude_cwd);
    let terminal_dir = berth_core::paths::canonical_or_raw(terminal_dir);
    berth_core::paths::contains(&claude_cwd, &terminal_dir)
        && berth_core::paths::contains(&terminal_dir, &claude_cwd)
}

impl Daemon {
    pub(crate) fn terminals_running_claude(&self) -> Vec<(TerminalId, String)> {
        let terminals: Vec<(TerminalId, u32)> = self
            .session_pids()
            .into_iter()
            .filter_map(|(sref, pid, _)| match sref {
                SessionRef::Terminal(id) => Some((id, pid)),
                SessionRef::Agent(_) => None,
            })
            .collect();
        if terminals.is_empty() {
            return Vec::new();
        }
        let table = berth_core::platform::process_table();
        let dir = crate::cli_title::claude_sessions_dir();
        crate::cli_title::newest_per_terminal(&table, &terminals, &dir, claude_pid_conversation)
            .into_iter()
            .filter(|(terminal, (_, cwd))| {
                self.terminal_dir(terminal)
                    .is_some_and(|dir| resumes_from(cwd, &dir))
            })
            .map(|(terminal, (session_id, _))| (terminal, session_id))
            .collect()
    }

    fn terminal_dir(&self, terminal: &TerminalId) -> Option<PathBuf> {
        let terminal = self.store.get_terminal(terminal).ok()??;
        let worktree = self.store.get_worktree(&terminal.worktree_id).ok()??;
        Some(worktree.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_core::AgentId;

    fn terminal(id: &str) -> TerminalId {
        TerminalId(id.into())
    }

    #[test]
    fn an_old_file_parses_as_plain_sessions() {
        let parsed: Vec<LiveSession> =
            serde_json::from_str(r#"[{"Terminal":"t1"},{"Agent":"a1"}]"#).unwrap();
        assert_eq!(
            parsed,
            vec![
                LiveSession::Session(SessionRef::Terminal(terminal("t1"))),
                LiveSession::Session(SessionRef::Agent(AgentId("a1".into()))),
            ]
        );
    }

    #[test]
    fn only_the_terminal_running_claude_is_marked() {
        let live = vec![
            SessionRef::Terminal(terminal("t1")),
            SessionRef::Terminal(terminal("t2")),
            SessionRef::Agent(AgentId("a1".into())),
        ];
        let entries = recorded(live, &[(terminal("t2"), "abc-123".into())]);
        assert_eq!(entries[0].claude_session_id(), None);
        assert_eq!(entries[1].claude_session_id(), Some("abc-123"));
        assert_eq!(
            entries[1].session_ref(),
            SessionRef::Terminal(terminal("t2"))
        );
        assert_eq!(entries[2].claude_session_id(), None);
        let json = serde_json::to_string(&entries).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<LiveSession>>(&json).unwrap(),
            entries
        );
    }

    #[test]
    fn the_first_spawn_of_a_restored_terminal_takes_its_claude_once() {
        let live = recorded(
            vec![
                SessionRef::Terminal(terminal("plain")),
                SessionRef::Terminal(terminal("with-claude")),
            ],
            &[(terminal("with-claude"), "abc-123".into())],
        );
        expect_claude(&live);
        assert_eq!(take_expected_claude(&terminal("plain")), None);
        assert_eq!(
            take_expected_claude(&terminal("with-claude")).as_deref(),
            Some("abc-123")
        );
        assert_eq!(take_expected_claude(&terminal("with-claude")), None);
    }

    #[test]
    fn only_a_claude_started_in_the_terminal_dir_resumes_there() {
        let root = Path::new("/no-such-root/repo");
        assert!(resumes_from(root, root));
        assert!(!resumes_from(&root.join("site"), root));
        assert!(!resumes_from(Path::new("/no-such-root/other"), root));
    }

    #[test]
    fn resume_command_refuses_what_a_shell_would_interpret() {
        assert_eq!(resume_command("a b"), None);
        assert_eq!(resume_command("a;b"), None);
        assert_eq!(resume_command("a\"b"), None);
        assert_eq!(resume_command(""), None);
        assert_eq!(
            resume_command("0b3f9c1e-5d2a-4e7b-8c6d-1a2b3c4d5e6f").as_deref(),
            Some("claude --resume 0b3f9c1e-5d2a-4e7b-8c6d-1a2b3c4d5e6f\r")
        );
    }
}
