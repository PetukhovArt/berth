//! User settings, read from `paths::config_path()` (JSON). Loaded fresh at
//! each use so edits apply without restarting the daemon. A missing file or
//! unknown fields fall back to defaults; a malformed file is logged and
//! ignored rather than failing the operation that read it.

use berth_core::daemon_defaults::daemon_defaults;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Run `git init` after AddProject creates a missing directory.
    pub git_init_on_create: bool,
    /// Pre-spawn agent CLIs while the user is still naming the session so
    /// creation feels instant. Costs one idle CLI process.
    pub prewarm: bool,
    /// Kill idle session PTYs in worktrees no client is looking at once
    /// they've gone unwatched this long: "1m" | "5m" | "15m" | "30m" | "1h"
    /// ("off" disables reaping entirely; any `<n>s`/`<n>m`/`<n>h` works).
    /// Bounds what prewarming and walked-away-from sessions cost. Pinned
    /// agents, running or feedback-waiting agents, and terminals with a
    /// command running are spared; a reaped session revives on the next
    /// attach (agents resume their conversation). Malformed
    /// values fall back to the 5m default.
    pub session_idle_timeout: String,
    /// Spawn the sessions that were live when the daemon stopped again when
    /// it starts, so a daemon restart brings them back without an attach.
    pub restore_sessions: bool,
    pub claude_limits: bool,
    pub webhooks: bool,
    pub webhook_port: u16,
    pub webhook_expose: bool,
    pub hub_url: Option<String>,
    pub hub_token: Option<String>,
    pub hub_ca: Option<String>,
    pub channel_chain_limit: u32,
    pub checkpoints_claude: bool,
    pub messenger_telegram_token: Option<String>,
    pub messenger_telegram_chat: Option<serde_json::Value>,
    pub messenger_telegram_owner: Option<serde_json::Value>,
    pub messenger_webhook_url: Option<String>,
    pub worktree_base: std::collections::BTreeMap<String, String>,
    pub providers: crate::acp::providers::ProvidersConfig,
    pub shared_setup: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let shared = daemon_defaults();
        Self {
            git_init_on_create: shared.git_init_on_create,
            prewarm: shared.prewarm,
            session_idle_timeout: shared.session_idle_timeout.clone(),
            restore_sessions: shared.restore_sessions,
            claude_limits: shared.claude_limits,
            webhooks: false,
            webhook_port: DEFAULT_WEBHOOK_PORT,
            webhook_expose: false,
            hub_url: None,
            hub_token: None,
            hub_ca: None,
            channel_chain_limit: 4,
            checkpoints_claude: shared.checkpoints_claude,
            worktree_base: shared.worktree_base.clone(),
            messenger_telegram_token: None,
            messenger_telegram_chat: None,
            messenger_telegram_owner: None,
            messenger_webhook_url: None,
            providers: Default::default(),
            shared_setup: None,
        }
    }
}

pub const DEFAULT_WEBHOOK_PORT: u16 = 47821;

impl Config {
    pub fn load() -> Self {
        let path = berth_core::paths::config_path();
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_else(|err| {
            tracing::warn!("ignoring malformed {}: {err}", path.display());
            Self::default()
        })
    }

    /// `session_idle_timeout` parsed to a duration; None = reaping disabled.
    pub fn session_idle_timeout(&self) -> Option<std::time::Duration> {
        parse_timeout(&self.session_idle_timeout).unwrap_or_else(|| {
            parse_timeout(&daemon_defaults().session_idle_timeout).expect("default parses")
        })
    }
}

const RENAMED_KEYS: [(&str, &str); 1] = [("prewarm_agents", "prewarm")];
const RETIRED_KEYS: [&str; 1] = ["prewarm_sessions"];

pub fn adopt_renamed_keys(path: &std::path::Path) -> anyhow::Result<()> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) if !raw.trim().is_empty() => raw,
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let serde_json::Value::Object(mut map) = serde_json::from_str(&raw)? else {
        return Ok(());
    };
    let mut changed = false;
    for (old, new) in RENAMED_KEYS {
        if let Some(value) = map.remove(old) {
            map.entry(new).or_insert(value);
            changed = true;
        }
    }
    for key in RETIRED_KEYS {
        changed |= map.remove(key).is_some();
    }
    if changed {
        let temp = path.with_extension("json.berth-tmp");
        std::fs::write(&temp, serde_json::to_string_pretty(&map)? + "\n")?;
        std::fs::rename(&temp, path)?;
    }
    Ok(())
}

/// "off"/"0" → Some(None); "<n>s"/"<n>m"/"<n>h" → Some(Some(d));
/// malformed → None (caller falls back to the default).
#[allow(clippy::option_option)]
fn parse_timeout(s: &str) -> Option<Option<std::time::Duration>> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("off") || s == "0" {
        return Some(None);
    }
    let (digits, unit_secs) = match s.strip_suffix(['s', 'S']) {
        Some(d) => (d, 1),
        None => match s.strip_suffix(['m', 'M']) {
            Some(d) => (d, 60),
            None => (s.strip_suffix(['h', 'H'])?, 3_600),
        },
    };
    let n: u64 = digits.trim().parse().ok()?;
    if n == 0 {
        return Some(None);
    }
    Some(Some(std::time::Duration::from_secs(n * unit_secs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_git_init() {
        assert!(Config::default().git_init_on_create);
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert!(cfg.git_init_on_create);
        let cfg: Config = serde_json::from_str(r#"{"git_init_on_create": false}"#).unwrap();
        assert!(!cfg.git_init_on_create);
    }

    #[test]
    fn defaults_enable_prewarm_and_allow_opt_out() {
        assert!(Config::default().prewarm);
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert!(cfg.prewarm);
        let cfg: Config = serde_json::from_str(r#"{"prewarm": false}"#).unwrap();
        assert!(!cfg.prewarm);
    }

    fn adopted(json: &str) -> serde_json::Value {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, json).unwrap();
        adopt_renamed_keys(&path).unwrap();
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    #[test]
    fn a_prewarm_turned_off_under_its_old_name_stays_off() {
        let cfg = adopted(
            r#"{"prewarm_agents": false, "prewarm_sessions": true, "session_idle_timeout": "1m"}"#,
        );
        assert_eq!(
            cfg,
            serde_json::json!({"prewarm": false, "session_idle_timeout": "1m"})
        );
        assert!(!serde_json::from_value::<Config>(cfg).unwrap().prewarm);
        assert_eq!(
            adopted(r#"{"prewarm": true, "prewarm_agents": false}"#),
            serde_json::json!({"prewarm": true})
        );
    }

    #[test]
    fn session_idle_timeout_parses_and_falls_back() {
        use std::time::Duration;
        let timeout = |v: &str| {
            let cfg: Config =
                serde_json::from_str(&format!(r#"{{"session_idle_timeout": "{v}"}}"#)).unwrap();
            cfg.session_idle_timeout()
        };
        assert_eq!(timeout("1m"), Some(Duration::from_secs(60)));
        assert_eq!(timeout("5m"), Some(Duration::from_secs(300)));
        assert_eq!(timeout("15m"), Some(Duration::from_secs(900)));
        assert_eq!(timeout("30m"), Some(Duration::from_secs(1800)));
        assert_eq!(timeout("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(
            timeout("2s"),
            Some(Duration::from_secs(2)),
            "seconds for tests"
        );
        assert_eq!(timeout("off"), None);
        assert_eq!(timeout("0"), None);
        // Malformed values fall back to the default, not to disabled.
        assert_eq!(timeout("soon"), Some(Duration::from_secs(300)));
        assert_eq!(
            Config::default().session_idle_timeout(),
            Some(Duration::from_secs(300))
        );
    }
}
