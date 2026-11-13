use berth_core::AgentKind;
use std::path::{Path, PathBuf};

pub const FRESH_START_NOTICE: &str =
    "\x1b[2m[berth: claude never saved the previous conversation, starting a new one]\x1b[0m\r\n";

pub fn claude_projects_root() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir).join("projects")),
        _ => crate::orphans::claude_projects_dir(),
    }
}

pub fn claude_transcript_missing(kind: AgentKind, session_id: Option<&str>, root: &Path) -> bool {
    let Some(sid) = session_id else {
        return false;
    };
    if kind != AgentKind::Claude || sid.is_empty() || sid.contains(['/', '\\', ':', '.']) {
        return false;
    }
    let Ok(dirs) = std::fs::read_dir(root) else {
        return false;
    };
    let file = format!("{sid}.jsonl");
    !dirs.flatten().any(|dir| dir.path().join(&file).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_only_when_no_project_dir_holds_the_transcript() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("C--Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        let sid = "294dfb75-9367-448e-aabf-069d6616dee2";
        assert!(claude_transcript_missing(
            AgentKind::Claude,
            Some(sid),
            root.path()
        ));

        std::fs::write(project.join(format!("{sid}.jsonl")), "{}\n").unwrap();
        assert!(!claude_transcript_missing(
            AgentKind::Claude,
            Some(sid),
            root.path()
        ));

        assert!(!claude_transcript_missing(
            AgentKind::Codex,
            Some("x"),
            root.path()
        ));
        assert!(!claude_transcript_missing(
            AgentKind::Claude,
            None,
            root.path()
        ));
        assert!(!claude_transcript_missing(
            AgentKind::Claude,
            Some("other"),
            &root.path().join("absent")
        ));
    }
}
