use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

struct BuiltIn {
    id: &'static str,
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
}

const BUILT_IN: [BuiltIn; 6] = [
    BuiltIn {
        id: "cursor",
        name: "Cursor",
        command: "agent",
        args: &["acp"],
    },
    BuiltIn {
        id: "opencode",
        name: "OpenCode",
        command: "opencode",
        args: &["acp"],
    },
    BuiltIn {
        id: "antigravity",
        name: "Antigravity",
        command: "agy",
        args: &["acp"],
    },
    BuiltIn {
        id: "gemini",
        name: "Gemini",
        command: "gemini",
        args: &["--acp"],
    },
    BuiltIn {
        id: "kiro",
        name: "Kiro",
        command: "kiro-cli",
        args: &["acp"],
    },
    BuiltIn {
        id: "pi",
        name: "Pi",
        command: "npx",
        args: &["-y", "pi-acp"],
    },
];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub acp: BTreeMap<String, AcpProviderOverride>,
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
}
