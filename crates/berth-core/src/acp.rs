use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcpItem {
    User {
        text: String,
        #[serde(default)]
        attachments: Vec<Attachment>,
        #[serde(default)]
        rewind: Option<Rewind>,
    },
    Assistant {
        text: String,
    },
    Thought {
        text: String,
    },
    Tool(AcpTool),
    Plan {
        entries: Vec<AcpPlanEntry>,
    },
    Notice {
        text: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewind {
    pub checkpoint: Option<u32>,
    pub after: Option<String>,
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
pub struct AcpCommand {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpItemAt {
    pub seq: u64,
    pub item: AcpItem,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptInput {
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

impl From<&str> for PromptInput {
    fn from(text: &str) -> Self {
        Self {
            text: text.into(),
            attachments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attachment {
    Image {
        blob: String,
        mime: String,
        name: String,
    },
    File {
        blob: String,
        mime: String,
        name: String,
    },
    WorktreeFile {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentUpload {
    Image {
        mime: String,
        name: String,
        data: String,
    },
    File {
        mime: String,
        name: String,
        text: String,
    },
    WorktreeFile {
        path: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSessionState {
    pub caps: ChatCaps,
    pub options: Vec<ConfigOption>,
    pub commands: Vec<AcpCommand>,
    pub queue: Vec<QueuedPrompt>,
    pub mcp: Vec<McpServer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCaps {
    pub images: bool,
    pub embedded_files: bool,
    pub questions: Questions,
    pub mcp: McpAccess,
    pub rewind: bool,
    pub steer: bool,
    pub terminal_view: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Questions {
    #[default]
    None,
    Choice,
    Typed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAccess {
    #[default]
    None,
    List,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionCategory {
    Model,
    ThoughtLevel,
    Mode,
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applies {
    #[default]
    Now,
    NextStart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOption {
    pub id: String,
    pub name: String,
    pub category: OptionCategory,
    pub current: String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub applies: Applies,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QueueId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedPrompt {
    pub id: QueueId,
    pub prompt: PromptInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionForm {
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub header: String,
    pub text: String,
    pub choices: Vec<Choice>,
    pub multi_select: bool,
    pub free_text_allowed: bool,
}

pub type Answers = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackAnswer {
    Option(u8),
    Form(Answers),
}

pub const ACP_DRIVER: &str = "acp";

pub struct BuiltInProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub driver: &'static str,
    pub on_by_default: bool,
}

pub const BUILT_IN_PROVIDERS: [BuiltInProvider; 9] = [
    BuiltInProvider {
        id: "cursor",
        name: "Cursor",
        command: "cursor-agent",
        args: &["acp"],
        driver: ACP_DRIVER,
        on_by_default: false,
    },
    BuiltInProvider {
        id: "opencode",
        name: "OpenCode",
        command: "opencode",
        args: &["acp"],
        driver: ACP_DRIVER,
        on_by_default: false,
    },
    BuiltInProvider {
        id: "antigravity",
        name: "Antigravity",
        command: "agy",
        args: &["acp"],
        driver: ACP_DRIVER,
        on_by_default: false,
    },
    BuiltInProvider {
        id: "gemini",
        name: "Gemini",
        command: "gemini",
        args: &["--acp"],
        driver: ACP_DRIVER,
        on_by_default: false,
    },
    BuiltInProvider {
        id: "kiro",
        name: "Kiro",
        command: "kiro-cli",
        args: &["acp"],
        driver: ACP_DRIVER,
        on_by_default: false,
    },
    BuiltInProvider {
        id: "pi",
        name: "Pi",
        command: "npx",
        args: &["-y", "pi-acp"],
        driver: ACP_DRIVER,
        on_by_default: false,
    },
    BuiltInProvider {
        id: "claude",
        name: "Claude",
        command: "claude",
        args: &[],
        driver: "claude",
        on_by_default: true,
    },
    BuiltInProvider {
        id: "codex",
        name: "Codex (protocol)",
        command: "codex",
        args: &[],
        driver: "codex",
        on_by_default: false,
    },
    BuiltInProvider {
        id: "grok",
        name: "Grok",
        command: "grok",
        args: &["agent", "stdio"],
        driver: ACP_DRIVER,
        on_by_default: true,
    },
];
