use crate::cli_title::CodexIndex;
use crate::config::Config;
use crate::registry::Daemon;
use anyhow::Result;
use berth_core::{Agent, AgentKind, UsageWindow};
use std::path::Path;

pub mod chat;
pub mod claude;
pub mod codex;
pub mod grok;
pub mod setup;

use setup::Setup;

pub struct Launch<'a> {
    pub session: Option<&'a str>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub prompt: Option<&'a str>,
    pub system_prompt: Option<&'a str>,
}

impl Launch<'_> {
    pub fn resumed(&self) -> bool {
        self.session.is_some()
    }
}

pub struct Hooks {
    pub endpoint: &'static str,
    pub install: fn(&Path) -> Result<()>,
    pub context: bool,
    pub foreground: bool,
    pub windows: bool,
}

pub struct Terminal {
    pub program: &'static str,
    pub args: fn(&Launch) -> Vec<String>,
    pub leading_args: fn() -> Vec<String>,
    pub hooks: Hooks,
    pub env: fn() -> Vec<(String, String)>,
    pub screen_errors: &'static [&'static str],
    pub boot_signal: bool,
    pub prompt_on_resume: bool,
    pub suggestion_scan: bool,
    pub own_checkpoints: bool,
}

pub type TitleFn = fn(Option<&Path>, &str, &mut CodexIndex) -> Option<String>;
pub type AnswerFn = fn(&Daemon, &Agent) -> Option<String>;

pub struct History {
    pub transcripts: bool,
    pub title: Option<TitleFn>,
    pub last_answer: Option<AnswerFn>,
}

pub struct Limits {
    pub enabled: fn(&Config) -> bool,
    pub windows: fn() -> Vec<UsageWindow>,
}

pub struct Profile {
    pub terminal: Option<Terminal>,
    pub history: Option<History>,
    pub limits: Option<Limits>,
    pub setup: &'static Setup,
}

pub const ALL: [AgentKind; 6] = [
    AgentKind::Claude,
    AgentKind::Codex,
    AgentKind::Cursor,
    AgentKind::OpenCode,
    AgentKind::Grok,
    AgentKind::Acp,
];

pub fn of(kind: AgentKind) -> &'static Profile {
    match kind {
        AgentKind::Claude => &claude::PROFILE,
        AgentKind::Codex => &codex::PROFILE,
        AgentKind::Grok => &grok::PROFILE,
        AgentKind::Cursor => &chat::CURSOR,
        AgentKind::OpenCode => &chat::OPENCODE,
        AgentKind::Acp => &chat::ACP,
    }
}

pub fn terminal(kind: AgentKind) -> Option<&'static Terminal> {
    of(kind).terminal.as_ref()
}

pub fn history(kind: AgentKind) -> Option<&'static History> {
    of(kind).history.as_ref()
}

pub fn transcripts(kind: AgentKind) -> bool {
    history(kind).is_some_and(|h| h.transcripts)
}

fn no_args() -> Vec<String> {
    Vec::new()
}

fn no_env() -> Vec<(String, String)> {
    Vec::new()
}

impl Terminal {
    pub fn command(
        &self,
        launch: &Launch,
        cmd_override: Option<&str>,
    ) -> (String, Vec<String>, bool) {
        let Some(cmd) = cmd_override else {
            return (
                self.program.to_string(),
                (self.args)(launch),
                launch.resumed(),
            );
        };
        let mut parts = cmd.split_whitespace().map(String::from).collect::<Vec<_>>();
        if parts.is_empty() {
            parts.push(self.program.into());
        }
        let program = parts.remove(0);
        (program, parts, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude::CLAUDE_WORKTREE_GUIDANCE;

    fn plain<'a>() -> Launch<'a> {
        Launch {
            session: None,
            model: None,
            effort: None,
            prompt: None,
            system_prompt: None,
        }
    }

    fn spawn(
        kind: AgentKind,
        launch: Launch,
        cmd_override: Option<&str>,
    ) -> (String, Vec<String>, bool) {
        terminal(kind).unwrap().command(&launch, cmd_override)
    }

    fn spawn_command(
        kind: AgentKind,
        session_id: Option<&str>,
        model: Option<&str>,
        effort: Option<&str>,
        cmd_override: Option<&str>,
    ) -> (String, Vec<String>, bool) {
        spawn(
            kind,
            Launch {
                session: session_id,
                model,
                effort,
                ..plain()
            },
            cmd_override,
        )
    }

    fn guided(args: &[&str]) -> Vec<String> {
        args.iter()
            .map(|s| s.to_string())
            .chain([
                "--append-system-prompt".to_string(),
                [
                    CLAUDE_WORKTREE_GUIDANCE,
                    crate::sibling::CLAUDE_SPAWN_GUIDANCE,
                    crate::enter_project::CLAUDE_ENTER_GUIDANCE,
                    crate::self_end::CLAUDE_END_GUIDANCE,
                ]
                .join("\n\n"),
            ])
            .collect()
    }

    #[test]
    fn spawn_command_per_kind_resume_shapes() {
        assert_eq!(
            spawn_command(AgentKind::Claude, None, None, None, None),
            ("claude".into(), guided(&[]), false)
        );
        assert_eq!(
            spawn_command(AgentKind::Codex, None, None, None, None),
            ("codex".into(), vec!["--yolo".to_string()], false)
        );
        assert_eq!(
            spawn_command(AgentKind::Claude, Some("sid-1"), None, None, None),
            ("claude".into(), guided(&["--resume", "sid-1"]), true)
        );
        assert_eq!(
            spawn_command(AgentKind::Codex, Some("sid-2"), None, None, None),
            (
                "codex".into(),
                vec![
                    "resume".to_string(),
                    "sid-2".to_string(),
                    "--yolo".to_string()
                ],
                true
            )
        );
        assert_eq!(
            spawn_command(
                AgentKind::Claude,
                Some("sid"),
                None,
                None,
                Some("/bin/sh -i")
            ),
            ("/bin/sh".into(), vec!["-i".to_string()], false)
        );
        assert_eq!(
            spawn_command(AgentKind::Codex, Some("sid"), None, None, Some("/bin/sh")),
            ("/bin/sh".into(), vec![], false)
        );
    }

    #[test]
    fn spawn_command_model_and_effort_flags() {
        assert_eq!(
            spawn_command(AgentKind::Claude, None, Some("opus"), Some("high"), None),
            (
                "claude".into(),
                guided(&["--model", "opus", "--effort", "high"]),
                false
            )
        );
        assert_eq!(
            spawn_command(AgentKind::Claude, None, None, Some("max"), None),
            ("claude".into(), guided(&["--effort", "max"]), false)
        );
        assert_eq!(
            spawn_command(AgentKind::Codex, None, Some("gpt-5.5"), Some("high"), None),
            (
                "codex".into(),
                vec![
                    "--yolo".to_string(),
                    "--model".to_string(),
                    "gpt-5.5".to_string(),
                    "-c".to_string(),
                    "model_reasoning_effort=high".to_string()
                ],
                false
            )
        );
        assert_eq!(
            spawn_command(AgentKind::Claude, Some("sid"), Some("sonnet"), None, None),
            (
                "claude".into(),
                guided(&["--resume", "sid", "--model", "sonnet"]),
                true
            )
        );
        assert_eq!(
            spawn_command(AgentKind::Claude, None, Some("opus"), None, Some("/bin/sh")),
            ("/bin/sh".into(), vec![], false)
        );
    }

    #[test]
    fn spawn_command_grok_shapes() {
        let title = crate::hooks::AUTO_TITLE_INSTRUCTION;
        assert_eq!(
            spawn(
                AgentKind::Grok,
                Launch {
                    model: Some("grok-4.6"),
                    effort: Some("high"),
                    prompt: Some("fix it"),
                    ..plain()
                },
                None
            ),
            (
                "grok".into(),
                vec![
                    "--always-approve",
                    "-m",
                    "grok-4.6",
                    "--reasoning-effort",
                    "high",
                    "--rules",
                    title,
                    "fix it"
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                false
            )
        );
        assert_eq!(
            spawn_command(AgentKind::Grok, Some("sid-5"), None, None, None),
            (
                "grok".into(),
                vec![
                    "--resume".to_string(),
                    "sid-5".to_string(),
                    "--always-approve".to_string()
                ],
                true
            )
        );
    }

    #[test]
    fn spawn_command_initial_prompt_is_the_trailing_positional_argument() {
        let (_, args, resumed) = spawn(
            AgentKind::Claude,
            Launch {
                session: Some("sid"),
                model: Some("opus"),
                prompt: Some("carry on"),
                ..plain()
            },
            None,
        );
        assert!(resumed);
        let mut expected = guided(&["--resume", "sid", "--model", "opus"]);
        expected.push("carry on".into());
        assert_eq!(args, expected);
        assert_eq!(
            spawn(
                AgentKind::Codex,
                Launch {
                    session: Some("sid"),
                    prompt: Some("carry on"),
                    ..plain()
                },
                None
            )
            .1,
            vec!["resume", "sid", "--yolo", "carry on"]
        );
        let mut expected = guided(&["--model", "opus", "--effort", "high"]);
        expected.push("fix auth".into());
        assert_eq!(
            spawn(
                AgentKind::Claude,
                Launch {
                    model: Some("opus"),
                    effort: Some("high"),
                    prompt: Some("fix auth"),
                    ..plain()
                },
                None
            )
            .1,
            expected
        );
        assert_eq!(
            spawn(
                AgentKind::Codex,
                Launch {
                    model: Some("gpt-5.5"),
                    effort: Some("high"),
                    prompt: Some("fix auth"),
                    ..plain()
                },
                None
            )
            .1,
            vec![
                "--yolo",
                "--model",
                "gpt-5.5",
                "-c",
                "model_reasoning_effort=high",
                "fix auth"
            ]
        );
        assert_eq!(
            spawn(
                AgentKind::Claude,
                Launch {
                    prompt: Some("carry on"),
                    ..plain()
                },
                Some("/bin/sh -i")
            ),
            ("/bin/sh".into(), vec!["-i".to_string()], false)
        );
    }

    #[test]
    fn spawn_command_keeps_pr_scope_and_url_in_claudes_system_prompt() {
        let pr_url = "https://github.com/PetukhovArt/berth/pull/42";
        let pr_prompt = claude::pr_system_prompt(pr_url);
        let (_, args, resumed) = spawn(
            AgentKind::Claude,
            Launch {
                session: Some("sid"),
                system_prompt: Some(&pr_prompt),
                ..plain()
            },
            None,
        );
        assert!(resumed);
        let prompts = args
            .windows(2)
            .filter(|pair| pair[0] == "--append-system-prompt")
            .map(|pair| pair[1].as_str())
            .collect::<Vec<_>>();
        assert_eq!(prompts.len(), 1, "Claude gets one composed system prompt");
        assert!(prompts[0].contains(CLAUDE_WORKTREE_GUIDANCE));
        assert!(prompts[0].contains(crate::sibling::CLAUDE_SPAWN_GUIDANCE));
        assert!(prompts[0].contains(crate::self_end::CLAUDE_END_GUIDANCE));
        assert!(prompts[0].contains("All work in this session must be scoped"));
        assert!(prompts[0].contains(pr_url));
    }
}
