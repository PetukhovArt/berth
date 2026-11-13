use anyhow::Result;
use berth_core::{ServerEvent, WorktreeId};

use crate::registry::Daemon;

pub const TASK_NOTE_MAX_BYTES: usize = 16 * 1024;

pub fn task_note_injection(text: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": format!("[berth] Task note: {text}"),
        }
    })
    .to_string()
}

impl Daemon {
    pub fn set_task_note(&self, worktree: &WorktreeId, text: &str) -> Result<()> {
        let text = text.trim();
        anyhow::ensure!(
            text.len() <= TASK_NOTE_MAX_BYTES,
            "Task note is longer than {TASK_NOTE_MAX_BYTES} bytes"
        );
        self.store.set_task_note(worktree, text)?;
        self.broadcast(ServerEvent::TaskNoteChanged {
            worktree: worktree.clone(),
            text: text.to_string(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use berth_core::{Project, ProjectId, Worktree};

    use super::*;
    use crate::store::Store;

    #[test]
    fn note_round_trips_and_empty_clears() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: ProjectId("p1".into()),
                name: "p".into(),
                repo_path: "/tmp/p".into(),
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        let w = WorktreeId("w1".into());
        store
            .insert_worktree(&Worktree {
                id: w.clone(),
                project_id: ProjectId("p1".into()),
                path: "/tmp/p".into(),
                branch: "main".into(),
                is_main: true,
                sort_order: 0,
            })
            .unwrap();
        assert_eq!(store.task_note(&w).unwrap(), "");
        store.set_task_note(&w, "use the v2 API").unwrap();
        store.set_task_note(&w, "use the v3 API").unwrap();
        assert_eq!(store.task_note(&w).unwrap(), "use the v3 API");
        store.set_task_note(&w, "").unwrap();
        assert_eq!(store.task_note(&w).unwrap(), "");
    }
}
