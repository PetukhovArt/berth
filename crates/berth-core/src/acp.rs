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
