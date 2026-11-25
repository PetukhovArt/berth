use super::setup::{HookFormat, Mcp, McpFormat, Setup, AGENTS_SKILLS};
use super::{no_args, Hooks, Launch, Limits, Profile, Terminal};
use crate::hooks::{installer, AUTO_TITLE_INSTRUCTION};
use crate::launch;
use anyhow::Result;
use berth_core::AgentKind;
use std::path::Path;

mod limits;

pub static PROFILE: Profile = Profile {
    terminal: Some(Terminal {
        program: "grok",
        args,
        leading_args: no_args,
        hooks: HOOKS,
        env: launch::grok_hook_shell_env,
        screen_errors: &[
            "request failed",
            "server error",
            "weekly limit",
            "spending cap",
            "credit limit",
        ],
        boot_signal: false,
        prompt_on_resume: false,
        suggestion_scan: false,
        own_checkpoints: false,
    }),
    history: None,
    limits: Some(Limits {
        enabled: |config| config.grok_limits,
        windows: limits::windows,
    }),
    setup: &Setup {
        skills: AGENTS_SKILLS,
        rule: None,
        reads_agents_md: true,
        mcp: Some(Mcp {
            file: ".grok/config.toml",
            format: McpFormat::Toml,
            approved_in_settings: false,
        }),
        mcp_over_acp: false,
        hooks: Some(HookFormat::Grok),
        imports_hooks_of: &[AgentKind::Claude, AgentKind::Cursor],
        plugins: false,
    },
};

pub const HOOKS: Hooks = Hooks {
    endpoint: "grok",
    install: install_hooks,
    context: false,
    foreground: true,
    windows: false,
};

fn install_hooks(_worktree: &Path) -> Result<()> {
    installer::install_grok_hooks(&installer::grok_home())
}

fn args(launch: &Launch) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(sid) = launch.session {
        args.extend(["--resume".to_string(), sid.to_string()]);
    }
    args.push("--always-approve".to_string());
    if let Some(m) = launch.model {
        args.extend(["-m".to_string(), m.to_string()]);
    }
    if let Some(e) = launch.effort {
        args.extend(["--reasoning-effort".to_string(), e.to_string()]);
    }
    if !launch.resumed() {
        args.extend(["--rules".to_string(), AUTO_TITLE_INSTRUCTION.to_string()]);
    }
    args.extend(launch.prompt.map(String::from));
    args
}
