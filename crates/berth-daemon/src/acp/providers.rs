use anyhow::{bail, Context, Result};
use berth_core::acp::{BUILT_IN_PROVIDERS as BUILT_IN, NATIVE_OFF_FOR_NEW_INSTALLS};
use berth_core::AgentKind;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub acp: BTreeMap<String, AcpProviderOverride>,
    #[serde(flatten)]
    pub native: BTreeMap<String, serde_json::Value>,
}

pub fn native_enabled(config: &ProvidersConfig, kind: AgentKind) -> bool {
    config
        .native
        .get(kind.as_str())
        .and_then(|entry| entry.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(!NATIVE_OFF_FOR_NEW_INSTALLS.contains(&kind))
}

pub fn ensure_native_enabled(config: &ProvidersConfig, kind: AgentKind) -> Result<()> {
    if !native_enabled(config, kind) {
        bail!("{} is turned off in Settings", kind.as_str());
    }
    Ok(())
}

pub fn keep_native_kinds_in_use(path: &Path, kinds_in_use: &[AgentKind]) -> Result<()> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) if !raw.trim().is_empty() => raw,
        Ok(_) => "{}".to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(e) => return Err(e.into()),
    };
    let mut root: serde_json::Value = serde_json::from_str(&raw)?;
    let Some(root_map) = root.as_object_mut() else {
        return Ok(());
    };
    let providers = root_map
        .entry("providers")
        .or_insert_with(|| serde_json::json!({}));
    let Some(providers) = providers.as_object_mut() else {
        return Ok(());
    };
    let mut changed = false;
    for kind in NATIVE_OFF_FOR_NEW_INSTALLS {
        if kinds_in_use.contains(&kind) && !providers.contains_key(kind.as_str()) {
            providers.insert(kind.as_str().into(), serde_json::json!({"enabled": true}));
            changed = true;
        }
    }
    if changed {
        std::fs::write(path, serde_json::to_string_pretty(&root)?)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AcpProviderOverride {
    pub enabled: Option<bool>,
    pub name: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpProvider {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

impl AcpProvider {
    pub fn program(&self) -> Result<PathBuf> {
        berth_core::platform::resolve_program(&self.command)
            .with_context(|| format!("{}: `{}` is not installed", self.name, self.command))
    }
}

pub fn resolve(config: &ProvidersConfig, id: &str) -> Option<AcpProvider> {
    let built_in = BUILT_IN.iter().find(|b| b.id == id);
    let custom = config.acp.get(id);
    let (name, command, args) = match (built_in, custom) {
        (Some(b), o) => (
            o.and_then(|o| o.name.clone())
                .unwrap_or_else(|| b.name.into()),
            o.and_then(|o| o.command.clone())
                .unwrap_or_else(|| b.command.into()),
            o.and_then(|o| o.args.clone())
                .unwrap_or_else(|| b.args.iter().map(|a| a.to_string()).collect()),
        ),
        (None, Some(o)) => (
            o.name.clone().unwrap_or_else(|| id.to_string()),
            o.command.clone()?,
            o.args.clone().unwrap_or_default(),
        ),
        (None, None) => return None,
    };
    let enabled_unless_said = built_in.is_none();
    Some(AcpProvider {
        id: id.to_string(),
        name,
        command,
        args,
        enabled: custom
            .and_then(|o| o.enabled)
            .unwrap_or(enabled_unless_said),
    })
}

pub fn resolve_enabled(config: &ProvidersConfig, id: &str) -> Result<AcpProvider> {
    let Some(provider) = resolve(config, id) else {
        bail!("unknown ACP provider `{id}`");
    };
    if !provider.enabled {
        bail!("{} is turned off in Settings", provider.name);
    }
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(json: &str) -> ProvidersConfig {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn built_ins_are_off_until_enabled_and_keep_their_default_command() {
        let none = ProvidersConfig::default();
        let pi = resolve(&none, "pi").unwrap();
        assert!(!pi.enabled);
        assert_eq!(pi.command, "npx");
        assert_eq!(pi.args, ["-y", "pi-acp"]);
        assert!(resolve_enabled(&none, "gemini").is_err());

        let on = config(r#"{"acp": {"gemini": {"enabled": true}}}"#);
        let gemini = resolve_enabled(&on, "gemini").unwrap();
        assert_eq!(gemini.command, "gemini");
        assert_eq!(gemini.args, ["--acp"]);
    }

    #[test]
    fn a_custom_entry_needs_a_command_and_is_on_unless_turned_off() {
        let c = config(
            r#"{"acp": {"mine": {"command": "my-agent", "args": ["--stdio"]}, "bare": {}}}"#,
        );
        let mine = resolve_enabled(&c, "mine").unwrap();
        assert_eq!(mine.name, "mine");
        assert_eq!(mine.args, ["--stdio"]);
        assert!(resolve(&c, "bare").is_none());
        assert!(resolve(&c, "nope").is_none());
    }

    #[test]
    fn native_cursor_and_opencode_are_off_unless_rows_kept_them_on() {
        let none = ProvidersConfig::default();
        assert!(native_enabled(&none, AgentKind::Claude));
        assert!(native_enabled(&none, AgentKind::Grok));
        assert!(!native_enabled(&none, AgentKind::Cursor));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"prewarm_agents": false, "providers": {"opencode": {"enabled": false}}}"#,
        )
        .unwrap();
        keep_native_kinds_in_use(&path, &[AgentKind::Cursor, AgentKind::OpenCode]).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["prewarm_agents"], false);
        let kept: ProvidersConfig = serde_json::from_value(written["providers"].clone()).unwrap();
        assert!(native_enabled(&kept, AgentKind::Cursor));
        assert!(!native_enabled(&kept, AgentKind::OpenCode));
    }
}
