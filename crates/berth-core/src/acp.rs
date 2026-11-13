use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcpItem {
    User { text: String },
    Assistant { text: String },
    Thought { text: String },
    Tool(AcpTool),
    Plan { entries: Vec<AcpPlanEntry> },
    Notice { text: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpTool {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub detail: Option<String>,
    pub diff: Option<AcpDiff>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpDiff {
    pub path: String,
    pub old: Option<String>,
    pub new: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpItemAt {
    pub seq: u64,
    pub item: AcpItem,
}

pub struct BuiltInProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

pub const BUILT_IN_PROVIDERS: [BuiltInProvider; 6] = [
    BuiltInProvider {
        id: "cursor",
        name: "Cursor",
        command: "agent",
        args: &["acp"],
    },
    BuiltInProvider {
        id: "opencode",
        name: "OpenCode",
        command: "opencode",
        args: &["acp"],
    },
    BuiltInProvider {
        id: "antigravity",
        name: "Antigravity",
        command: "agy",
        args: &["acp"],
    },
    BuiltInProvider {
        id: "gemini",
        name: "Gemini",
        command: "gemini",
        args: &["--acp"],
    },
    BuiltInProvider {
        id: "kiro",
        name: "Kiro",
        command: "kiro-cli",
        args: &["acp"],
    },
    BuiltInProvider {
        id: "pi",
        name: "Pi",
        command: "npx",
        args: &["-y", "pi-acp"],
    },
];

pub const NATIVE_OFF_FOR_NEW_INSTALLS: [crate::AgentKind; 2] =
    [crate::AgentKind::Cursor, crate::AgentKind::OpenCode];
