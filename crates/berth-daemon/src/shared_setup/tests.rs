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

    let setup = setup_at(&source, EVERYTHING);
    let report = setup.sync(&project, &repo).unwrap();
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
        git(&repo, &["status", "--porcelain"]),
        "",
        "installed files stay out of git status"
    );

    write(&source.join("skills/review/SKILL.md"), "review v2");
    write(&repo.join(".claude/rules/style.md"), "My own rule.");
    setup.sync(&project, &repo).unwrap();
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
    setup.sync(&project, &worktree).unwrap();
    assert_eq!(
        read(&worktree.join(".claude/skills/review/SKILL.md")),
        "review v2"
    );
    assert!(worktree.join("AGENTS.md").exists());

    let setup = setup_at(&source, SKILL_ONLY);
    let report = setup.sync(&project, &worktree).unwrap();
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
    assert_eq!(
        read(&worktree.join(".claude/skills/review/SKILL.md")),
        "review v2"
    );
}
