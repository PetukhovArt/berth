use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonDefaults {
    pub session_idle_timeout: String,
    pub session_archive_after: String,
    pub restore_sessions: bool,
    pub git_init_on_create: bool,
    pub prewarm: bool,
    pub claude_limits: bool,
    pub grok_limits: bool,
    pub checkpoints_claude: bool,
    pub worktree_base: BTreeMap<String, String>,
    pub worktree_default_bases: Vec<String>,
}

pub fn daemon_defaults() -> &'static DaemonDefaults {
    static DEFAULTS: OnceLock<DaemonDefaults> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        serde_json::from_str(include_str!("../daemon-defaults.json"))
            .expect("daemon-defaults.json matches DaemonDefaults")
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_key_in_the_shared_file_has_a_field() {
        assert_eq!(super::daemon_defaults().session_idle_timeout, "5m");
    }
}
