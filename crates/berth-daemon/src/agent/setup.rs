use berth_core::AgentKind;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HookFormat {
    Claude,
    Cursor,
    Codex,
    Grok,
}

impl HookFormat {
    pub const ALL: [HookFormat; 4] = [Self::Claude, Self::Cursor, Self::Codex, Self::Grok];
}

pub enum RuleFormat {
    Plain,
    AlwaysApply,
}

pub struct Rule {
    pub dir: &'static str,
    pub ext: &'static str,
    pub format: RuleFormat,
}

pub enum McpFormat {
    Json { typed_remote: bool },
    OpenCode,
    Toml,
}

pub struct Mcp {
    pub file: &'static str,
    pub format: McpFormat,
    pub approved_in_settings: bool,
}

pub struct Setup {
    pub skills: &'static str,
    pub rule: Option<Rule>,
    pub reads_agents_md: bool,
    pub mcp: Option<Mcp>,
    pub mcp_over_acp: bool,
    pub hooks: Option<HookFormat>,
    pub imports_hooks_of: &'static [AgentKind],
    pub plugins: bool,
}

pub const AGENTS_SKILLS: &str = ".agents/skills";
