//! `berth spawn "<task>"` from inside an agent session: a new AGENT beside
//! the caller — same WORKTREE, same harness unless another is named — that
//! opens on the task as its STARTING PROMPT. The caller's own process is
//! never touched (unlike `berth worktree`, nothing here waits on a turn
//! end), so the model runs it, tells the user, and carries on.

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use berth_core::{AgentId, AgentKind, EntityId, ServerEvent, SpawnWorktree, Worktree, WorktreeId};

use crate::registry::{CreateAgentSpec, Daemon};

/// What berth appends to Claude's system prompt so "start a new berth
/// session that …" becomes one `berth spawn` call instead of the model
/// trying to launch an agent process itself. Claude only, like the worktree
/// guidance: codex has no system-prompt flag.
pub const CLAUDE_SPAWN_GUIDANCE: &str = "[berth] When the user asks you to start a new berth \
session (\"start a new berth session that …\", \"spin up another session to …\", \"open a new \
session for …\"), do not launch an agent process yourself. Run this shell command instead, exactly \
once:\n\n  berth spawn \"<task>\"\n\nwhere <task> is the work the user wants that session to do, \
in their own words — the new session opens on it as its first prompt, so make it self-contained. \
Add `--kind claude|codex|grok` only when the user names the harness; otherwise the new session \
matches this one. berth starts it beside this session, in the same worktree, unless the user wants \
it in another worktree or branch of this project: then add `--worktree <branch>`, which reuses that \
branch's worktree or creates the branch and its worktree when there is none, and `--base <ref>` \
only when the user names the start point of a new branch. The new session shows up in the \
sessions list on its own. This session is unaffected: carry on with whatever else the user \
asked, and if starting the session was the whole request, tell the user in one line that it is \
running. If the command fails, report the error.";

/// The first free `agent-N` among `taken` — the same default the TUI's
/// name prompt offers, which is what makes the new row eligible for
/// AUTO-TITLE (the daemon titles only rows created on the default name).
pub(crate) fn sibling_name(taken: &[String]) -> String {
    (1..)
        .map(|n| format!("agent-{n}"))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unbounded counter always finds a free name")
}

impl Daemon {
    /// The create spec for a session started beside `id`: its worktree, its
    /// harness (and model / effort) unless `kind` overrides — a different
    /// CLI cannot take this one's model name — a default `agent-N` name so
    /// AUTO-TITLE applies, and `starting_prompt` as the first prompt (which
    /// `create_agent` validates). Pure lookup, so it is unit-testable
    /// without a PTY.
    pub(crate) fn sibling_spec(
        &self,
        id: &AgentId,
        kind: Option<AgentKind>,
        starting_prompt: &str,
        target: Option<WorktreeId>,
    ) -> Result<CreateAgentSpec> {
        let caller = self.store.get_agent(id)?.context("agent not found")?;
        if caller.archived {
            bail!("agent is archived");
        }
        let worktree = target.unwrap_or_else(|| caller.worktree_id.clone());
        let (_, _, agents, _) = self.store.load_tree()?;
        let taken = agents
            .iter()
            .filter(|a| a.worktree_id == worktree)
            .map(|a| a.name.clone())
            .collect::<Vec<_>>();
        let kind = kind.unwrap_or(caller.kind);
        let (model, effort) = if kind == caller.kind {
            (caller.model.clone(), caller.effort.clone())
        } else {
            (None, None)
        };
        Ok(CreateAgentSpec {
            worktree,
            name: sibling_name(&taken),
            kind,
            model,
            effort,
            auto_title: true,
            starting_prompt: Some(starting_prompt.to_string()),
        })
    }

    /// `berth spawn`, run by the agent inside its own session: create and
    /// boot a new agent beside it with `starting_prompt` as its first
    /// prompt. Returns the new row's id; the upsert reaches every client
    /// through the ordinary create path.
    pub async fn spawn_sibling_agent(
        self: &Arc<Self>,
        id: &AgentId,
        kind: Option<AgentKind>,
        starting_prompt: &str,
        worktree: Option<&SpawnWorktree>,
    ) -> Result<EntityId> {
        crate::registry::validate_starting_prompt(starting_prompt)?;
        let target = match worktree {
            Some(wanted) => Some(self.spawn_worktree(id, wanted).await?.id),
            None => None,
        };
        let spec = self.sibling_spec(id, kind, starting_prompt, target)?;
        let created = self.create_agent(spec).await?;
        if let EntityId::Agent(agent) = &created {
            self.broadcast(ServerEvent::AgentSpawned {
                agent: agent.clone(),
                spawned_by: id.clone(),
            });
        }
        Ok(created)
    }

    async fn spawn_worktree(
        self: &Arc<Self>,
        id: &AgentId,
        wanted: &SpawnWorktree,
    ) -> Result<Worktree> {
        let branch = wanted.branch.trim();
        if branch.is_empty() {
            bail!("branch name is empty");
        }
        let caller = self.store.get_agent(id)?.context("agent not found")?;
        if caller.archived {
            bail!("agent is archived");
        }
        let current = self
            .store
            .get_worktree(&caller.worktree_id)?
            .context("worktree not found")?;
        let project = self
            .store
            .get_project(&current.project_id)?
            .context("project not found")?;
        let find = || -> Result<Option<Worktree>> {
            let (_, worktrees, _, _) = self.store.load_tree()?;
            Ok(worktrees
                .into_iter()
                .find(|w| w.project_id == project.id && w.branch == branch))
        };
        if let Some(found) = find()? {
            return Ok(found);
        }
        if let Err(error) = self.sync_project_worktrees(&project).await {
            tracing::warn!(project = %project.name, error = %error, "worktree sync before spawn failed");
        }
        if let Some(adopted) = find()? {
            return Ok(adopted);
        }
        let (created, warning) = self
            .create_worktree_from_base(&project.id, branch, wanted.base.as_deref())
            .await?;
        if let Some(warning) = warning {
            tracing::warn!(worktree = %created.id, warning = %warning, "spawn worktree base fallback");
        }
        Ok(created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::HookEnv;
    use crate::store::Store;
    use berth_core::{Agent, AgentStatus, Backend, Project, ProjectId, Worktree, WorktreeId};

    fn daemon() -> Arc<Daemon> {
        let daemon = Daemon::new(
            Arc::new(Store::open_in_memory().unwrap()),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        );
        daemon
            .store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: ProjectId("p".into()),
                name: "p".into(),
                repo_path: "/berth-test/p".into(),
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        for (id, is_main) in [("root", true), ("feat", false)] {
            daemon
                .store
                .insert_worktree(&Worktree {
                    id: WorktreeId(id.into()),
                    project_id: ProjectId("p".into()),
                    path: format!("/berth-test/p-{id}").into(),
                    branch: id.into(),
                    is_main,
                    sort_order: 0,
                })
                .unwrap();
        }
        daemon
    }

    fn agent(id: &str, worktree: &str, kind: AgentKind, model: Option<&str>) -> Agent {
        Agent {
            id: AgentId(id.into()),
            worktree_id: WorktreeId(worktree.into()),
            name: id.into(),
            status: AgentStatus::Running,
            archived: false,
            archived_at: 0,
            ended_at: 0,
            unseen: false,
            kind,
            model: model.map(str::to_string),
            effort: model.map(|_| "high".to_string()),
            session_id: Some("s1".into()),
            sort_order: 0,
            status_changed_at: 0,
            alive: false,
            suggested_prompt: None,
            provider_error: None,
            routed_from: None,
            route_suggestion: None,
            acp_provider: None,
            backend: Backend::Terminal,
        }
    }

    #[test]
    fn sibling_name_is_the_first_free_agent_n() {
        assert_eq!(sibling_name(&[]), "agent-1");
        let taken = ["agent-1", "Fix Login Redirect", "agent-3"]
            .map(String::from)
            .to_vec();
        assert_eq!(sibling_name(&taken), "agent-2");
    }

    #[test]
    fn sibling_spec_lands_in_the_callers_worktree_with_its_harness() {
        let daemon = daemon();
        daemon
            .store
            .insert_agent(&agent("agent-1", "feat", AgentKind::Claude, Some("opus")))
            .unwrap();
        // A row in another worktree does not take a name in this one.
        daemon
            .store
            .insert_agent(&agent("agent-2", "root", AgentKind::Codex, None))
            .unwrap();

        let spec = daemon
            .sibling_spec(
                &AgentId("agent-1".into()),
                None,
                "Fix the login redirect",
                None,
            )
            .unwrap();
        assert_eq!(spec.worktree.to_string(), "feat");
        assert_eq!(spec.name, "agent-2");
        assert_eq!(spec.kind, AgentKind::Claude);
        assert_eq!(spec.model.as_deref(), Some("opus"));
        assert_eq!(spec.effort.as_deref(), Some("high"));
        assert!(spec.auto_title, "a default name earns an auto-title");
        assert_eq!(
            spec.starting_prompt.as_deref(),
            Some("Fix the login redirect")
        );
    }

    #[test]
    fn a_named_harness_drops_the_callers_model_and_effort() {
        let daemon = daemon();
        daemon
            .store
            .insert_agent(&agent("agent-1", "feat", AgentKind::Claude, Some("opus")))
            .unwrap();
        let spec = daemon
            .sibling_spec(
                &AgentId("agent-1".into()),
                Some(AgentKind::Codex),
                "Run the tests",
                None,
            )
            .unwrap();
        assert_eq!(spec.kind, AgentKind::Codex);
        assert!(
            spec.model.is_none() && spec.effort.is_none(),
            "a Claude model name means nothing to codex"
        );
        // Naming the caller's own harness keeps its knobs.
        let same = daemon
            .sibling_spec(
                &AgentId("agent-1".into()),
                Some(AgentKind::Claude),
                "Run the tests",
                None,
            )
            .unwrap();
        assert_eq!(same.model.as_deref(), Some("opus"));
    }

    #[test]
    fn unknown_and_archived_callers_are_refused() {
        let daemon = daemon();
        // `CreateAgentSpec` is deliberately not Debug (it carries the
        // prompt), so the Err side is taken by hand.
        let missing = daemon
            .sibling_spec(&AgentId("nope".into()), None, "x", None)
            .err()
            .expect("an unknown caller is refused");
        assert!(missing.to_string().contains("agent not found"));

        let mut archived = agent("agent-1", "feat", AgentKind::Claude, None);
        archived.archived = true;
        daemon.store.insert_agent(&archived).unwrap();
        let err = daemon
            .sibling_spec(&AgentId("agent-1".into()), None, "x", None)
            .err()
            .expect("an archived caller is refused");
        assert!(err.to_string().contains("archived"));
    }

    /// The prompt itself is `create_agent`'s to validate (blank, NUL, too
    /// long), so a bad one fails there, before any worktree lookup.
    #[tokio::test]
    async fn spawn_sibling_agent_rejects_a_blank_prompt() {
        let daemon = daemon();
        daemon
            .store
            .insert_agent(&agent("agent-1", "feat", AgentKind::Claude, None))
            .unwrap();
        let err = daemon
            .spawn_sibling_agent(&AgentId("agent-1".into()), None, " \n ", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("is empty"), "{err}");
    }

    fn git_in(repo: &std::path::Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[tokio::test]
    async fn spawn_worktree_creates_once_reuses_and_adopts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = berth_core::paths::canonical_or_raw(tmp.path());
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git_in(&repo, &["init", "-b", "main"]);
        git_in(&repo, &["commit", "--allow-empty", "-m", "init"]);
        let daemon = Daemon::new(
            Arc::new(Store::open_in_memory().unwrap()),
            HookEnv {
                port: 0,
                token: String::new(),
            },
        );
        daemon
            .store
            .insert_project(&Project {
                workspace_id: Default::default(),
                id: ProjectId("p".into()),
                name: "p".into(),
                repo_path: repo.clone(),
                sort_order: 0,
                pin_order: None,
            })
            .unwrap();
        daemon
            .store
            .insert_worktree(&Worktree {
                id: WorktreeId("root".into()),
                project_id: ProjectId("p".into()),
                path: repo.clone(),
                branch: "main".into(),
                is_main: true,
                sort_order: 0,
            })
            .unwrap();
        daemon
            .store
            .insert_agent(&agent("agent-1", "root", AgentKind::Claude, None))
            .unwrap();
        let caller = AgentId("agent-1".into());
        let wanted = |branch: &str| SpawnWorktree {
            branch: branch.into(),
            base: Some("main".into()),
        };

        let created = daemon
            .spawn_worktree(&caller, &wanted("feat"))
            .await
            .unwrap();
        assert_eq!(created.branch, "feat");
        assert!(created.path.join(".git").exists(), "a real checkout");
        let again = daemon
            .spawn_worktree(&caller, &wanted(" feat "))
            .await
            .unwrap();
        assert_eq!(again.id, created.id, "the same branch reuses its worktree");

        let outside = root.join("outside");
        git_in(
            &repo,
            &["worktree", "add", &outside.to_string_lossy(), "-b", "other"],
        );
        let adopted = daemon
            .spawn_worktree(&caller, &wanted("other"))
            .await
            .unwrap();
        assert_eq!(adopted.branch, "other");
        assert!(
            !adopted.path.starts_with(root.join("repo-worktrees")),
            "a checkout made outside berth is taken, not duplicated: {}",
            adopted.path.display()
        );

        let spec = daemon
            .sibling_spec(&caller, None, "Run the tests", Some(created.id.clone()))
            .unwrap();
        assert_eq!(spec.worktree, created.id);
        assert_eq!(spec.name, "agent-1", "names are taken per worktree");
        let caller_row = daemon.store.get_agent(&caller).unwrap().unwrap();
        assert_eq!(caller_row.worktree_id.to_string(), "root");
    }
}
