//! Archive marks on EXTERNAL SESSIONS: Claude Code / Codex conversations
//! berth did not start, which clients find on disk themselves. The daemon
//! only keeps the mark, so every client (the desktop app and its remote
//! viewers) agrees on what is archived and it survives restarts. Nothing
//! here touches the transcript or a process running it.

use anyhow::{bail, Result};
use berth_core::{AgentKind, ServerEvent};

use crate::registry::Daemon;

fn validate(session_id: &str) -> Result<()> {
    if session_id.trim().is_empty() {
        bail!("external session id is empty");
    }
    Ok(())
}

impl Daemon {
    pub fn archive_external_session(&self, kind: AgentKind, session_id: &str) -> Result<()> {
        validate(session_id)?;
        let session = self.store.archive_external_session(kind, session_id)?;
        self.broadcast(ServerEvent::ExternalSessionArchived { session });
        Ok(())
    }

    pub fn unarchive_external_session(&self, kind: AgentKind, session_id: &str) -> Result<()> {
        validate(session_id)?;
        self.store.unarchive_external_session(kind, session_id)?;
        self.broadcast(ServerEvent::ExternalSessionUnarchived {
            kind,
            session_id: session_id.to_string(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::hooks::HookEnv;
    use crate::store::Store;

    fn daemon() -> Arc<Daemon> {
        Daemon::new(
            Arc::new(Store::open_in_memory().unwrap()),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        )
    }

    #[test]
    fn archive_and_restore_broadcast_and_reach_the_snapshot() {
        let daemon = daemon();
        let mut events = daemon.events.subscribe();

        daemon
            .archive_external_session(AgentKind::Claude, "sess-1")
            .unwrap();
        match events.try_recv().unwrap() {
            ServerEvent::ExternalSessionArchived { session } => {
                assert_eq!(session.kind, AgentKind::Claude);
                assert_eq!(session.session_id, "sess-1");
            }
            other => panic!("unexpected event {other:?}"),
        }
        let ServerEvent::Snapshot {
            archived_external_sessions,
            ..
        } = daemon.snapshot().unwrap()
        else {
            panic!("not a snapshot");
        };
        assert_eq!(archived_external_sessions.len(), 1);

        daemon
            .unarchive_external_session(AgentKind::Claude, "sess-1")
            .unwrap();
        assert!(matches!(
            events.try_recv().unwrap(),
            ServerEvent::ExternalSessionUnarchived { kind: AgentKind::Claude, ref session_id } if session_id == "sess-1"
        ));
        let ServerEvent::Snapshot {
            archived_external_sessions,
            ..
        } = daemon.snapshot().unwrap()
        else {
            panic!("not a snapshot");
        };
        assert!(archived_external_sessions.is_empty());
    }

    #[test]
    fn an_empty_session_id_is_refused() {
        let daemon = daemon();
        assert!(daemon
            .archive_external_session(AgentKind::Codex, " ")
            .is_err());
        assert!(daemon
            .store
            .load_archived_external_sessions()
            .unwrap()
            .is_empty());
    }
}
