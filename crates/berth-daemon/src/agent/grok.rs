use super::{no_args, Hooks, Launch, Profile, Terminal};
use crate::hooks::{installer, AUTO_TITLE_INSTRUCTION};
use crate::launch;
use anyhow::Result;
use std::path::Path;

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
    }),
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
