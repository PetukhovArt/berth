use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    #[default]
    Claude,
    Codex,
    Cursor,
    #[serde(rename = "opencode")]
    OpenCode,
    Grok,
    Acp,
}

impl AgentKind {
    pub const NATIVE: [AgentKind; 3] = [AgentKind::Claude, AgentKind::Codex, AgentKind::Grok];

    pub fn as_str(&self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Cursor => "cursor",
            AgentKind::OpenCode => "opencode",
            AgentKind::Grok => "grok",
            AgentKind::Acp => "acp",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "claude" => AgentKind::Claude,
            "codex" => AgentKind::Codex,
            "cursor" => AgentKind::Cursor,
            "opencode" => AgentKind::OpenCode,
            "grok" => AgentKind::Grok,
            "acp" => AgentKind::Acp,
            _ => return None,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            AgentKind::Claude => "Claude",
            AgentKind::Codex => "Codex",
            AgentKind::Cursor => "Cursor",
            AgentKind::OpenCode => "OpenCode",
            AgentKind::Grok => "Grok",
            AgentKind::Acp => "ACP",
        }
    }

    pub fn of_provider(provider_id: &str) -> AgentKind {
        Self::NATIVE
            .into_iter()
            .find(|kind| kind.as_str() == provider_id)
            .unwrap_or(AgentKind::Acp)
    }

    pub fn of_chat_row(self, provider_id: Option<&str>) -> AgentKind {
        match (self, provider_id) {
            (AgentKind::Acp, Some(id)) => Self::of_provider(id),
            _ => self,
        }
    }

    pub fn program(&self) -> Option<&'static str> {
        match self {
            AgentKind::Claude => Some("claude"),
            AgentKind::Codex => Some("codex"),
            AgentKind::Grok => Some("grok"),
            AgentKind::Cursor | AgentKind::OpenCode | AgentKind::Acp => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Chat,
    #[default]
    Terminal,
}

impl Backend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Backend::Chat => "chat",
            Backend::Terminal => "terminal",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "chat" => Backend::Chat,
            "terminal" => Backend::Terminal,
            _ => return None,
        })
    }

    pub fn of(kind: AgentKind, stored: Option<&str>) -> Self {
        let before_the_column = if kind == AgentKind::Acp {
            Backend::Chat
        } else {
            Backend::Terminal
        };
        stored.and_then(Self::parse).unwrap_or(before_the_column)
    }
}
