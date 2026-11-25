use super::{no_env, History, Hooks, Launch, Limits, Profile, Terminal};
use crate::cli_title::{self, CodexIndex};
use crate::hooks::{claude_mod, installer};
use crate::messenger::answer;
use anyhow::Result;
use berth_core::{env, AgentKind};
use std::path::Path;

mod limits;

pub const KIND: AgentKind = AgentKind::Claude;

pub static PROFILE: Profile = Profile {
    terminal: Some(Terminal {
        program: "claude",
        args,
        leading_args,
        hooks: HOOKS,
        env: no_env,
        screen_errors: &["please run /login", "credit balance is too low"],
        boot_signal: true,
        prompt_on_resume: true,
        suggestion_scan: true,
        own_checkpoints: true,
    }),
    history: Some(History {
        transcripts: true,
        title: Some(title),
        last_answer: Some(answer::claude),
    }),
    limits: Some(Limits {
        enabled: |config| config.claude_limits,
        windows: limits::windows,
    }),
};

fn title(transcript: Option<&Path>, _session_id: &str, _codex: &mut CodexIndex) -> Option<String> {
    transcript.and_then(cli_title::claude_title)
}

pub const HOOKS: Hooks = Hooks {
    endpoint: "claude",
    install: install_hooks,
    context: true,
    foreground: false,
    windows: false,
};

pub const CLAUDE_WORKTREE_GUIDANCE: &str = "[berth] This session runs inside berth, which \
manages this project's git worktrees. When the user asks you to work in a worktree (\"do this in a \
worktree\", \"in a new worktree\", \"branch this off in its own checkout\"), do not use the \
EnterWorktree tool and do not run `git worktree add` yourself. Run this shell command instead, \
exactly once:\n\n  berth worktree <name>\n\nwhere <name> is the branch name the user gave, or a \
short kebab-case name for the task (`berth worktree` with no name invents one; `--base <ref>` picks \
the start point). berth creates the worktree, associates this session with it, and relocates the \
session into it once your current turn ends. So when the command succeeds, end your turn at once: \
tell the user in one line that the session is moving into the worktree, and make no further tool \
calls or edits — you will be resumed inside the worktree with a prompt to carry on there. If the \
command fails, report the error and carry on in the current checkout.";

pub fn pr_system_prompt(pr_url: &str) -> String {
    format!(
        "[berth] This session was created from the OPEN PRS row for {pr_url}. All work in this \
         session must be scoped to that pull request. Inspect the PR before acting, and do not \
         modify or report on unrelated work. Before editing, make sure changes are made on the \
         PR's head branch (using a dedicated worktree if necessary), never in an unrelated \
         checkout. Keep reviews, tests, commits, pushes, and CI actions limited to this PR."
    )
}

fn reports_through_mod() -> bool {
    env::var(env::AGENT_CMD).is_none() && claude_mod::claude_loads_mods()
}

fn install_hooks(worktree: &Path) -> Result<()> {
    if reports_through_mod() {
        claude_mod::install_mod(&claude_mod::plugin_dir())
            .and_then(|()| installer::install_claude_settings(worktree))
    } else {
        installer::install_claude_hooks(worktree)
    }
}

fn leading_args() -> Vec<String> {
    if reports_through_mod() {
        claude_mod::plugin_dir_args(&claude_mod::plugin_dir())
    } else {
        Vec::new()
    }
}

fn args(launch: &Launch) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(sid) = launch.session {
        args.extend(["--resume".to_string(), sid.to_string()]);
    }
    if let Some(m) = launch.model {
        args.extend(["--model".to_string(), m.to_string()]);
    }
    if let Some(e) = launch.effort {
        args.extend(["--effort".to_string(), e.to_string()]);
    }
    let mut system_prompt = vec![
        CLAUDE_WORKTREE_GUIDANCE,
        crate::sibling::CLAUDE_SPAWN_GUIDANCE,
        crate::enter_project::CLAUDE_ENTER_GUIDANCE,
        crate::self_end::CLAUDE_END_GUIDANCE,
    ];
    system_prompt.extend(launch.system_prompt);
    args.extend([
        "--append-system-prompt".to_string(),
        system_prompt.join("\n\n"),
    ]);
    args.extend(launch.prompt.map(String::from));
    args
}
