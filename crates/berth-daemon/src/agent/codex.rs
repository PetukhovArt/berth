use super::{no_env, Hooks, Launch, Profile, Terminal};
use crate::hooks::installer;
use anyhow::Result;
use berth_core::env;
use std::path::Path;

pub static PROFILE: Profile = Profile {
    terminal: Some(Terminal {
        program: "codex",
        args,
        leading_args,
        hooks: HOOKS,
        env: no_env,
        screen_errors: &[
            "stream error",
            "stream disconnected",
            "exceeded retry limit",
            "unexpected status",
        ],
        boot_signal: false,
        prompt_on_resume: false,
        suggestion_scan: false,
    }),
};

pub const HOOKS: Hooks = Hooks {
    endpoint: "codex",
    install: install_hooks,
    context: true,
    foreground: false,
    windows: true,
};

fn install_hooks(worktree: &Path) -> Result<()> {
    installer::install_codex_hooks(&installer::codex_home())
        .and_then(|()| installer::prune_codex_worktree_hooks(worktree))
}

fn leading_args() -> Vec<String> {
    if env::var(env::AGENT_CMD).is_none() {
        crate::codex_server::own_server_args()
    } else {
        Vec::new()
    }
}

fn args(launch: &Launch) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(sid) = launch.session {
        args.extend(["resume".to_string(), sid.to_string()]);
    }
    args.push("--yolo".to_string());
    if let Some(m) = launch.model {
        args.extend(["--model".to_string(), m.to_string()]);
    }
    if let Some(e) = launch.effort {
        args.extend(["-c".to_string(), format!("model_reasoning_effort={e}")]);
    }
    args.extend(launch.prompt.map(String::from));
    args
}
