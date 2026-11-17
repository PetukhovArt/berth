use std::path::Path;

use berth_core::{Project, ProjectId};
use serde_json::Value;

use super::{SetupFile, SharedSetup};

fn git(repo: &Path, args: &[&str]) -> String {
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
    assert!(out.status.success(), "git {args:?}: {:?}", out.stderr);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn setup_at(root: &Path, toml: &str) -> SharedSetup {
    write(&root.join(super::SETUP_FILE), toml);
    SharedSetup {
        root: root.to_path_buf(),
        file: SetupFile::parse(toml).unwrap(),
    }
}

const EVERYTHING: &str = r#"
[[set]]
skills = ["review"]
instructions = ["style"]
mcp = ["docs"]
hooks = ["fmt"]

[[set]]
kinds = ["claude"]
projects = ["another-project"]
skills = ["private"]

[mcp.docs]
command = "npx"
args = ["-y", "docs-mcp"]
"#;

const SKILL_ONLY: &str = r#"
[[set]]
kinds = ["claude"]
skills = ["review"]
"#;

#[tokio::test]
async fn sync_installs_per_kind_tracks_what_it_wrote_and_leaves_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    let base = berth_core::paths::canonical_or_raw(tmp.path());
    let source = base.join("setup");
    write(&source.join("skills/review/SKILL.md"), "review v1");
    write(&source.join("skills/private/SKILL.md"), "private");
    write(&source.join("instructions/style.md"), "Use short names.");
    write(
        &source.join("hooks/fmt.json"),
        r#"{"PostToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"fmt"}]}]}"#,
    );

    let repo = base.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    write(&repo.join(".gitignore"), ".claude/settings.local.json\n");
    write(&repo.join(".cursor/mcp.json"), "{\"mcpServers\":{}}\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-m", "init"]);
    write(
        &repo.join(".claude/settings.local.json"),
        r#"{"hooks":{"PostToolUse":[{"_berthManaged":true,"hooks":[]},{"matcher":"Bash","hooks":[]}]}}"#,
    );
    let project = Project {
        workspace_id: Default::default(),
        id: ProjectId("p".into()),
        name: "repo".into(),
        repo_path: repo.clone(),
        sort_order: 0,
        pin_order: None,
    };

    let codex_home = base.join("codex-home");
    std::fs::create_dir_all(&codex_home).unwrap();
    let setup = setup_at(&source, EVERYTHING);
    let report = setup
        .sync_with_codex_home(&project, &repo, &codex_home)
        .unwrap();
    assert_eq!(
        read(&repo.join(".claude/skills/review/SKILL.md")),
        "review v1"
    );
    assert_eq!(
        read(&repo.join(".agents/skills/review/SKILL.md")),
        "review v1"
    );
    assert!(!repo.join(".claude/skills/private").exists());
    assert_eq!(
        read(&repo.join(".claude/rules/style.md")),
        "Use short names."
    );
    assert!(read(&repo.join(".cursor/rules/style.mdc")).contains("alwaysApply: true"));
    assert!(read(&repo.join("AGENTS.md")).contains("Use short names."));
    assert!(read(&repo.join(".mcp.json")).contains("docs-mcp"));
    assert!(read(&repo.join(".codex/config.toml")).contains("[mcp_servers.docs]"));
    assert!(read(&repo.join(".grok/config.toml")).contains("[mcp_servers.docs]"));
    let opencode: Value =
        serde_json::from_str(&read(&repo.join(".opencode/opencode.json"))).unwrap();
    assert_eq!(
        opencode["mcp"]["servers"]["docs"]["command"],
        serde_json::json!(["npx", "-y", "docs-mcp"])
    );
    assert!(
        !repo.join(".cursor/hooks.json").exists(),
        "Cursor reads Claude's hooks itself"
    );
    assert!(
        !repo.join(".grok/hooks").exists(),
        "Grok reads Claude's hooks itself"
    );
    assert_eq!(read(&repo.join(".git/berth-shared-hooks")), "fmt\n");
    assert!(read(&codex_home.join("hooks.json")).contains("berth-shared-hooks"));
    assert_eq!(
        read(&repo.join(".cursor/mcp.json")),
        "{\"mcpServers\":{}}\n"
    );
    assert_eq!(report.kept_as_found, vec![".cursor/mcp.json".to_string()]);
    let settings: Value =
        serde_json::from_str(&read(&repo.join(".claude/settings.local.json"))).unwrap();
    let groups = settings["hooks"]["PostToolUse"].as_array().unwrap();
    assert_eq!(groups.len(), 3, "managed, the user's and the shared group");
    assert_eq!(
        settings["enabledMcpjsonServers"],
        serde_json::json!(["docs"])
    );
    assert_eq!(
        git(&repo, &["status", "--porcelain"]),
        "",
        "installed files stay out of git status"
    );

    write(&source.join("skills/review/SKILL.md"), "review v2");
    write(&repo.join(".claude/rules/style.md"), "My own rule.");
    setup
        .sync_with_codex_home(&project, &repo, &codex_home)
        .unwrap();
    assert_eq!(
        read(&repo.join(".claude/skills/review/SKILL.md")),
        "review v2"
    );
    assert_eq!(read(&repo.join(".claude/rules/style.md")), "My own rule.");
    let settings: Value =
        serde_json::from_str(&read(&repo.join(".claude/settings.local.json"))).unwrap();
    assert_eq!(
        settings["hooks"]["PostToolUse"].as_array().unwrap().len(),
        3
    );

    let worktree = base.join("wt");
    git(
        &repo,
        &["worktree", "add", "-b", "topic", worktree.to_str().unwrap()],
    );
    crate::worktree_seed::seed(&repo, &worktree).await;
    setup
        .sync_with_codex_home(&project, &worktree, &codex_home)
        .unwrap();
    assert_eq!(
        read(&worktree.join(".claude/skills/review/SKILL.md")),
        "review v2"
    );
    assert!(worktree.join("AGENTS.md").exists());

    let setup = setup_at(&source, SKILL_ONLY);
    let report = setup
        .sync_with_codex_home(&project, &worktree, &codex_home)
        .unwrap();
    assert!(
        report.removed.contains(&"AGENTS.md".to_string()),
        "{report:?}"
    );
    assert!(!worktree.join("AGENTS.md").exists());
    assert!(!worktree.join(".agents").exists());
    assert!(!worktree.join(".codex").exists());
    let settings: Value =
        serde_json::from_str(&read(&worktree.join(".claude/settings.local.json"))).unwrap();
    let groups = settings["hooks"]["PostToolUse"].as_array().unwrap();
    assert_eq!(
        groups.len(),
        2,
        "seeded managed and user groups stay: {settings}"
    );
    assert!(groups.iter().all(|g| g.get("_berthShared").is_none()));
    assert!(
        settings.get("enabledMcpjsonServers").is_none(),
        "{settings}"
    );
    assert!(!worktree.join(".opencode").exists());
    assert!(!repo.join(".git/berth-shared-hooks").exists());
    assert!(!read(&codex_home.join("hooks.json")).contains("_berthShared"));
    assert_eq!(
        read(&worktree.join(".claude/skills/review/SKILL.md")),
        "review v2"
    );
}

const MIXED_HOOKS: &str = r#"{
  "PreToolUse": [{"matcher": "Bash|Glob", "hooks": [{"type": "command", "command": "check", "timeout": 5}]}],
  "PostToolUse": [{"matcher": "Edit|Write|mcp__docs__search", "hooks": [
    {"type": "command", "command": "fmt"},
    {"type": "http", "url": "http://localhost/hook"}
  ]}],
  "Notification": [{"hooks": [{"type": "command", "command": "notify"}]}],
  "PostToolUseFailure": [{"matcher": "Glob", "hooks": [{"type": "command", "command": "glob"}]}]
}"#;

#[test]
fn hooks_translate_per_kind_and_name_what_a_kind_lacks() {
    let item = super::hooks::HookItem::parse("mixed", MIXED_HOOKS).unwrap();

    let mut skipped = Vec::new();
    let codex = item.codex(&mut skipped);
    let events: Vec<&str> = codex.iter().map(|(e, _)| e.as_str()).collect();
    assert_eq!(events, ["PostToolUse", "PreToolUse"]);
    let command = codex[1].1["hooks"][0]["command"].as_str().unwrap();
    assert!(command.starts_with("grep -qxF 'mixed' "), "{command}");
    assert!(command.ends_with("|| exit 0; check"), "{command}");
    assert_eq!(codex[1].1["matcher"], "Bash|Glob");
    assert_eq!(codex[0].1["hooks"].as_array().unwrap().len(), 1);
    assert!(codex.iter().all(|(_, g)| g["_berthShared"] == true));
    assert!(skipped.contains(&"hook `mixed`: codex has no `Notification` hook".to_string()));
    assert!(skipped.contains(&"hook `mixed`: codex has no `PostToolUseFailure` hook".to_string()));
    assert!(skipped
        .contains(&"hook `mixed`: codex cannot run a `http` handler in `PostToolUse`".to_string()));

    let mut skipped = Vec::new();
    let cursor = item.cursor(&mut skipped);
    assert_eq!(cursor.len(), 2, "{cursor:?}");
    assert_eq!(cursor[1].0, "preToolUse");
    assert_eq!(
        cursor[1].1,
        serde_json::json!({"command": "check", "matcher": "Shell", "timeout": 5, "_berthShared": true})
    );
    assert_eq!(cursor[0].0, "postToolUse");
    assert_eq!(cursor[0].1["matcher"], "Write|MCP:search");
    assert!(skipped.contains(&"hook `mixed`: cursor has no `Notification` hook".to_string()));
    assert!(skipped.contains(
        &"hook `mixed`: cursor has no tool matching `Glob` in `PostToolUseFailure`".to_string()
    ));
    assert!(skipped.contains(
        &"hook `mixed`: cursor cannot run a `http` handler in `PostToolUse`".to_string()
    ));

    let mut skipped = Vec::new();
    let grok = item.grok(&mut skipped);
    assert_eq!(grok.len(), 4);
    assert!(skipped.is_empty(), "{skipped:?}");
    assert!(grok.iter().all(|(_, g)| g.get("_berthShared").is_none()));
}

const ROUTED: &str = r#"
[[set]]
kinds = ["cursor", "codex", "grok"]
hooks = ["guard"]

[[set]]
kinds = ["grok"]
hooks = ["solo"]

[[set]]
kinds = ["claude"]
mcp = ["docs"]
plugins = ["review", "lint"]

[mcp.docs]
command = "npx"

[plugins.review]
marketplace = "team"
source = { source = "github", repo = "me/plugins" }

[plugins.lint]
marketplace = "team"
plugin = "linter"
"#;

#[test]
fn sync_routes_hooks_plugins_and_approvals_and_takes_them_back() {
    let tmp = tempfile::tempdir().unwrap();
    let base = berth_core::paths::canonical_or_raw(tmp.path());
    let source = base.join("setup");
    write(
        &source.join("hooks/guard.json"),
        r#"{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"guard"}]}],"Notification":[{"hooks":[{"type":"command","command":"n"}]}]}"#,
    );
    write(
        &source.join("hooks/solo.json"),
        r#"{"Stop":[{"hooks":[{"type":"command","command":"solo"}]}]}"#,
    );
    let repo = base.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    write(
        &repo.join(".claude/settings.json"),
        r#"{"model":"opus","enabledPlugins":{"other@x":true}}"#,
    );
    let codex_home = base.join("codex-home");
    write(
        &codex_home.join("hooks.json"),
        r#"{"hooks":{"Stop":[{"_berthManaged":true,"hooks":[]}]}}"#,
    );
    let project = Project {
        workspace_id: Default::default(),
        id: ProjectId("p".into()),
        name: "repo".into(),
        repo_path: repo.clone(),
        sort_order: 0,
        pin_order: None,
    };

    let report = setup_at(&source, ROUTED)
        .sync_with_codex_home(&project, &repo, &codex_home)
        .unwrap();
    assert!(report
        .skipped
        .contains(&"hook `guard`: cursor has no `Notification` hook".to_string()));

    let cursor: Value = serde_json::from_str(&read(&repo.join(".cursor/hooks.json"))).unwrap();
    assert_eq!(cursor["version"], 1);
    assert_eq!(cursor["hooks"]["preToolUse"][0]["matcher"], "Shell");

    let grok: Value =
        serde_json::from_str(&read(&repo.join(".grok/hooks/berth-shared.json"))).unwrap();
    assert_eq!(
        grok["hooks"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["Stop"],
        "guard reaches Grok through .cursor/hooks.json: {grok}"
    );

    assert_eq!(read(&repo.join(".git/berth-shared-hooks")), "guard\n");
    let codex: Value = serde_json::from_str(&read(&codex_home.join("hooks.json"))).unwrap();
    assert_eq!(codex["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(codex["hooks"]["PreToolUse"][0]["_berthShared"], true);
    assert!(codex["hooks"].get("Notification").is_none());

    let settings: Value = serde_json::from_str(&read(&repo.join(".claude/settings.json"))).unwrap();
    assert_eq!(settings["model"], "opus");
    assert_eq!(
        settings["enabledPlugins"],
        serde_json::json!({"other@x": true, "review@team": true, "linter@team": true})
    );
    assert_eq!(
        settings["extraKnownMarketplaces"]["team"]["source"]["repo"],
        "me/plugins"
    );
    let local: Value =
        serde_json::from_str(&read(&repo.join(".claude/settings.local.json"))).unwrap();
    assert_eq!(local["enabledMcpjsonServers"], serde_json::json!(["docs"]));

    let mut settings = settings;
    settings["enabledPlugins"]["review@team"] = serde_json::json!(false);
    write(
        &repo.join(".claude/settings.json"),
        &serde_json::to_string(&settings).unwrap(),
    );
    let report = SharedSetup::default()
        .sync_with_codex_home(&project, &repo, &codex_home)
        .unwrap();
    assert!(report
        .removed
        .contains(&".claude/settings.local.json".to_string()));
    let settings: Value = serde_json::from_str(&read(&repo.join(".claude/settings.json"))).unwrap();
    assert_eq!(
        settings,
        serde_json::json!({"model": "opus", "enabledPlugins": {"other@x": true, "review@team": false}})
    );
    assert!(!repo.join(".claude/settings.local.json").exists());
    assert!(!repo.join(".cursor").exists());
    assert!(!repo.join(".grok").exists());
    assert!(!repo.join(".git/berth-shared-hooks").exists());
    let codex: Value = serde_json::from_str(&read(&codex_home.join("hooks.json"))).unwrap();
    assert_eq!(
        codex,
        serde_json::json!({"hooks": {"Stop": [{"_berthManaged": true, "hooks": []}]}})
    );
}
