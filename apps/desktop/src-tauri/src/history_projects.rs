use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use tokio::task::spawn_blocking;

use crate::agent_sessions::{collect, Harness};

const HISTORY_DAYS: u32 = 365;

#[derive(Debug, Serialize)]
pub struct HistoryProject {
    path: String,
    name: String,
    conversations: u32,
    last_active: u64,
    harnesses: Vec<Harness>,
}

fn trimmed(cwd: &str) -> &str {
    let stripped = cwd.trim_end_matches(['/', '\\']);
    if stripped.is_empty() {
        cwd
    } else {
        stripped
    }
}

fn name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

fn gather() -> Vec<HistoryProject> {
    let mut found: HashMap<String, HistoryProject> = HashMap::new();
    for session in collect(Some(HISTORY_DAYS)) {
        let cwd = trimmed(&session.cwd);
        if cwd.is_empty() {
            continue;
        }
        let key = if cfg!(windows) {
            cwd.to_lowercase()
        } else {
            cwd.to_owned()
        };
        let entry = found.entry(key).or_insert_with(|| HistoryProject {
            path: cwd.to_owned(),
            name: name_of(cwd),
            conversations: 0,
            last_active: 0,
            harnesses: Vec::new(),
        });
        entry.conversations += 1;
        entry.last_active = entry.last_active.max(session.updated_at);
        if !entry.harnesses.contains(&session.harness) {
            entry.harnesses.push(session.harness);
        }
    }
    let mut projects: Vec<HistoryProject> = found
        .into_values()
        .filter(|project| Path::new(&project.path).join(".git").exists())
        .collect();
    projects.sort_by_key(|project| std::cmp::Reverse(project.last_active));
    projects
}

pub async fn history_projects() -> Result<Vec<HistoryProject>, String> {
    spawn_blocking(gather).await.map_err(|e| e.to_string())
}
