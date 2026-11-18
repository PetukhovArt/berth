//! Sessions that were live when the daemon stopped are spawned again when
//! it starts, so a daemon update does not leave every row to be reopened by
//! hand. The list is written on a clean shutdown only: after a crash or a
//! reboot the sessions come back the lazy way, on the next attach.

use std::sync::Arc;
use std::time::Duration;

use berth_core::protocol::SessionRef;

use crate::registry::Daemon;
use crate::terminal_resume::LiveSession;

/// Gap between two boots: a
/// burst of CLI starts at once is slower than the starts in turn.
const STAGGER: Duration = Duration::from_millis(1500);
/// Grid a restored PTY starts with; the first attach resizes it.
const COLS: u16 = 120;
const ROWS: u16 = 32;

impl Daemon {
    /// Write the live sessions down. Called before the PTYs are killed.
    pub fn record_live_sessions(&self) {
        let live: Vec<SessionRef> = self
            .session_pids()
            .into_iter()
            .filter(|(_, _, prewarm)| prewarm.is_none())
            .map(|(sref, _, _)| sref)
            .chain(self.live_acp_agents().into_iter().map(SessionRef::Agent))
            .collect();
        let running = self.terminals_running_claude();
        let entries = crate::terminal_resume::recorded(live, &running);
        let path = berth_core::paths::live_sessions_path();
        let written = serde_json::to_vec(&entries)
            .map_err(std::io::Error::other)
            .and_then(|json| std::fs::write(&path, json));
        if let Err(err) = written {
            tracing::warn!(
                "could not record live sessions in {}: {err}",
                path.display()
            );
        }
    }

    pub fn take_live_sessions(&self) -> Vec<LiveSession> {
        let path = berth_core::paths::live_sessions_path();
        let Ok(raw) = std::fs::read(&path) else {
            return Vec::new();
        };
        let _ = std::fs::remove_file(&path);
        if !crate::config::Config::load().restore_sessions {
            return Vec::new();
        }
        let live: Vec<LiveSession> = serde_json::from_slice(&raw).unwrap_or_default();
        crate::terminal_resume::expect_claude(&live);
        live
    }

    pub async fn restore_live_sessions(self: Arc<Self>, live: Vec<LiveSession>) {
        if live.is_empty() {
            return;
        }
        tracing::info!(count = live.len(), "restoring live sessions");
        for sref in live.into_iter().map(|entry| entry.session_ref()) {
            if self.is_alive(&sref) {
                continue;
            }
            let daemon = self.clone();
            let target = sref.clone();
            let spawned = tokio::task::spawn_blocking(move || match &target {
                SessionRef::Agent(id) if daemon.is_acp_agent(id) => daemon.restore_acp_agent(id),
                _ => daemon.ensure_session(&target, COLS, ROWS).map(|_| ()),
            })
            .await;
            match spawned {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::debug!(session = ?sref, error = %e, "session restore failed")
                }
                Err(e) => tracing::debug!(session = ?sref, error = %e, "session restore panicked"),
            }
            tokio::time::sleep(STAGGER).await;
        }
    }
}
