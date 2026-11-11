//! Sessions that were live when the daemon stopped are spawned again when
//! it starts, so a daemon update does not leave every row to be reopened by
//! hand. The list is written on a clean shutdown only: after a crash or a
//! reboot the sessions come back the lazy way, on the next attach.

use std::sync::Arc;
use std::time::Duration;

use berth_core::protocol::SessionRef;

use crate::registry::Daemon;

/// Gap between two boots, the same reason as a worktree prewarm sweep: a
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
            .collect();
        let path = berth_core::paths::live_sessions_path();
        let written = serde_json::to_vec(&live)
            .map_err(std::io::Error::other)
            .and_then(|json| std::fs::write(&path, json));
        if let Err(err) = written {
            tracing::warn!(
                "could not record live sessions in {}: {err}",
                path.display()
            );
        }
    }

    /// Spawn what `record_live_sessions` wrote, one session at a time. The
    /// list is consumed either way, so a later start never replays an old one.
    pub async fn restore_live_sessions(self: Arc<Self>) {
        let path = berth_core::paths::live_sessions_path();
        let Ok(raw) = std::fs::read(&path) else {
            return;
        };
        let _ = std::fs::remove_file(&path);
        if !crate::config::Config::load().restore_sessions {
            return;
        }
        let live: Vec<SessionRef> = serde_json::from_slice(&raw).unwrap_or_default();
        tracing::info!(count = live.len(), "restoring live sessions");
        for sref in live {
            if self.is_alive(&sref) {
                continue;
            }
            let daemon = self.clone();
            let target = sref.clone();
            let spawned = tokio::task::spawn_blocking(move || {
                daemon.ensure_session(&target, COLS, ROWS).map(|_| ())
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
