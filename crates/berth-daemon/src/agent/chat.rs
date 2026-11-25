use super::setup::{HookFormat, Mcp, McpFormat, Rule, RuleFormat, Setup, AGENTS_SKILLS};
use super::Profile;
use berth_core::AgentKind;

const fn chat_only(setup: &'static Setup) -> Profile {
    Profile {
        terminal: None,
        history: None,
        limits: None,
        setup,
    }
}

pub static CURSOR: Profile = chat_only(&Setup {
    skills: AGENTS_SKILLS,
    rule: Some(Rule {
        dir: ".cursor/rules",
        ext: "mdc",
        format: RuleFormat::AlwaysApply,
    }),
    reads_agents_md: false,
    mcp: Some(Mcp {
        file: ".cursor/mcp.json",
        format: McpFormat::Json {
            typed_remote: false,
        },
        approved_in_settings: false,
    }),
    mcp_over_acp: false,
    hooks: Some(HookFormat::Cursor),
    imports_hooks_of: &[AgentKind::Claude],
    plugins: false,
});

pub static OPENCODE: Profile = chat_only(&Setup {
    skills: AGENTS_SKILLS,
    rule: None,
    reads_agents_md: true,
    mcp: Some(Mcp {
        file: ".opencode/opencode.json",
        format: McpFormat::OpenCode,
        approved_in_settings: false,
    }),
    mcp_over_acp: false,
    hooks: None,
    imports_hooks_of: &[],
    plugins: false,
});

pub static ACP: Profile = chat_only(&Setup {
    skills: AGENTS_SKILLS,
    rule: None,
    reads_agents_md: true,
    mcp: None,
    mcp_over_acp: true,
    hooks: None,
    imports_hooks_of: &[],
    plugins: false,
});
