use anyhow::{bail, Result};
use berth_core::acp::{ACP_DRIVER, BUILT_IN_PROVIDERS as BUILT_IN};
use berth_core::AgentKind;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub acp: BTreeMap<String, AcpProviderOverride>,
    #[serde(flatten)]
    pub native: BTreeMap<String, serde_json::Value>,
}

pub fn native_enabled(config: &ProvidersConfig, kind: AgentKind) -> bool {
    kind.program().is_some()
        && config
            .native
            .get(kind.as_str())
            .and_then(|entry| entry.get("enabled"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
}

pub fn ensure_native_enabled(config: &ProvidersConfig, kind: AgentKind) -> Result<()> {
    if kind.program().is_none() {
        bail!("{} runs only in the chat view", kind.as_str());
    }
    if !native_enabled(config, kind) {
        bail!("{} is turned off in Settings", kind.as_str());
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
    pub driver: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpProvider {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub driver: String,
    pub enabled: bool,
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
    let enabled_unless_said = built_in.is_none_or(|b| b.on_by_default);
    Some(AcpProvider {
        id: id.to_string(),
        name,
        command,
        args,
        driver: custom
            .and_then(|o| o.driver.clone())
            .or_else(|| built_in.map(|b| b.driver.to_string()))
            .unwrap_or_else(|| ACP_DRIVER.to_string()),
        enabled: custom
            .and_then(|o| o.enabled)
            .unwrap_or(enabled_unless_said),
    })
}

pub fn default_chat(config: &ProvidersConfig, kind: AgentKind) -> Option<AcpProvider> {
    BUILT_IN
        .iter()
        .find(|built_in| built_in.id == kind.as_str() && built_in.on_by_default)
        .and_then(|built_in| resolve(config, built_in.id))
        .filter(|provider| provider.enabled)
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
        assert!(!resolve(&none, "codex").unwrap().enabled);

        let on = config(r#"{"acp": {"gemini": {"enabled": true}}}"#);
        let gemini = resolve_enabled(&on, "gemini").unwrap();
        assert_eq!(gemini.command, "gemini");
        assert_eq!(gemini.args, ["--acp"]);
    }

    #[test]
    fn grok_is_on_over_acp_until_turned_off() {
        let none = ProvidersConfig::default();
        let grok = resolve_enabled(&none, "grok").unwrap();
        assert_eq!(grok.command, "grok");
        assert_eq!(grok.args, ["agent", "stdio"]);
        assert_eq!(grok.driver, ACP_DRIVER);
        let off = config(r#"{"acp": {"grok": {"enabled": false}}}"#);
        assert!(resolve_enabled(&off, "grok").is_err());
    }

    #[test]
    fn claude_and_grok_open_in_the_chat_view_and_codex_in_its_terminal() {
        let none = ProvidersConfig::default();
        assert_eq!(default_chat(&none, AgentKind::Claude).unwrap().id, "claude");
        assert_eq!(default_chat(&none, AgentKind::Grok).unwrap().id, "grok");
        assert!(default_chat(&none, AgentKind::Codex).is_none());
        let off = config(r#"{"acp": {"claude": {"enabled": false}}}"#);
        assert!(default_chat(&off, AgentKind::Claude).is_none());
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
    fn cursor_and_opencode_have_no_terminal_even_when_a_config_turns_them_on() {
        let on = config(r#"{"cursor": {"enabled": true}, "opencode": {"enabled": true}}"#);
        assert!(native_enabled(&on, AgentKind::Claude));
        assert!(!native_enabled(&on, AgentKind::Cursor));
        assert!(ensure_native_enabled(&on, AgentKind::OpenCode).is_err());
    }
}
