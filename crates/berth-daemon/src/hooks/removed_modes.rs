use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const OPENCODE_PLUGIN_FILE: &str = "berth.js";
const OPENCODE_PLUGIN_ENDPOINT: &str = "/api/hooks/opencode";

fn opencode_plugin_dir() -> PathBuf {
    berth_core::env::non_empty("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            berth_core::env::home_dir()
                .unwrap_or_default()
                .join(".config")
        })
        .join("opencode")
        .join("plugin")
}

fn prune_opencode_plugin(dir: &Path) -> Result<()> {
    let path = dir.join(OPENCODE_PLUGIN_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) if text.contains(OPENCODE_PLUGIN_ENDPOINT) => {
            std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

pub fn prune(worktrees: impl IntoIterator<Item = PathBuf>) {
    if let Err(error) = prune_opencode_plugin(&opencode_plugin_dir()) {
        tracing::warn!(%error, "could not remove berth's OpenCode plugin");
    }
    for cwd in worktrees {
        if let Err(error) = super::installer::prune_cursor_worktree_hooks(&cwd) {
            tracing::warn!(%error, cwd = %cwd.display(), "could not remove berth's Cursor hooks");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_users_own_cursor_hooks_and_opencode_plugins_survive() {
        let tmp = tempfile::tempdir().unwrap();
        let cursor = tmp.path().join(".cursor");
        std::fs::create_dir_all(cursor.join("rules")).unwrap();
        std::fs::write(
            cursor.join("hooks.json"),
            r#"{"version": 1, "hooks": {
                "stop": [
                    {"command": "say done"},
                    {"command": "curl $BERTH_API_URL/api/hooks/cursor?agentId=$BERTH_AGENT_ID", "_berthManaged": true}
                ],
                "sessionStart": [{"command": "x", "_berthManaged": true}]
            }}"#,
        )
        .unwrap();
        std::fs::write(cursor.join("rules/berth-title.mdc"), "title").unwrap();
        std::fs::write(cursor.join("rules/mine.mdc"), "mine").unwrap();

        let plugins = tmp.path().join("plugin");
        std::fs::create_dir_all(&plugins).unwrap();
        std::fs::write(plugins.join("berth.js"), "fetch(`${u}/api/hooks/opencode`)").unwrap();
        std::fs::write(plugins.join("mine.js"), "mine").unwrap();

        for _ in 0..2 {
            super::super::installer::prune_cursor_worktree_hooks(tmp.path()).unwrap();
            prune_opencode_plugin(&plugins).unwrap();
        }

        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cursor.join("hooks.json")).unwrap())
                .unwrap();
        assert_eq!(hooks["version"], 1);
        assert_eq!(hooks["hooks"]["stop"].as_array().unwrap().len(), 1);
        assert_eq!(hooks["hooks"]["stop"][0]["command"], "say done");
        assert!(hooks["hooks"].get("sessionStart").is_none());
        assert!(!cursor.join("rules/berth-title.mdc").exists());
        assert!(cursor.join("rules/mine.mdc").exists());
        assert!(!plugins.join("berth.js").exists());
        assert!(plugins.join("mine.js").exists());
    }
}
