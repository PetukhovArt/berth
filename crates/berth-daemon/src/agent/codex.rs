use super::setup::{HookFormat, Mcp, McpFormat, Setup, AGENTS_SKILLS};
use super::{no_env, History, Hooks, Launch, Limits, Profile, Terminal};
use crate::cli_title::CodexIndex;
use crate::hooks::installer;
use crate::messenger::answer;
use anyhow::Result;
use berth_core::env;
use std::path::Path;

mod limits;

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
        own_checkpoints: false,
    }),
    history: Some(History {
        transcripts: false,
        title: Some(title),
        last_answer: Some(answer::codex),
    }),
    limits: Some(Limits {
        enabled: |_| true,
        windows: limits::windows,
    }),
    setup: &Setup {
        skills: AGENTS_SKILLS,
        rule: None,
        reads_agents_md: true,
        mcp: Some(Mcp {
            file: ".codex/config.toml",
            format: McpFormat::Toml,
            approved_in_settings: false,
        }),
        mcp_over_acp: false,
        hooks: Some(HookFormat::Codex),
        imports_hooks_of: &[],
        plugins: false,
    },
};

fn title(_transcript: Option<&Path>, session_id: &str, codex: &mut CodexIndex) -> Option<String> {
    codex.title(session_id)
}

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
