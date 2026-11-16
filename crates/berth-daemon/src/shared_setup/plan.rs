use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use berth_core::{AgentKind, Project};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const SETUP_FILE: &str = "berth-setup.toml";

pub(super) const SHARED_HOOK_TAG: &str = "_berthShared";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupFile {
    #[serde(default, rename = "set")]
    sets: Vec<Set>,
    #[serde(default)]
    mcp: BTreeMap<String, McpServer>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    kinds: Option<Vec<AgentKind>>,
    projects: Option<Vec<String>>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    instructions: Vec<String>,
    #[serde(default)]
    mcp: Vec<String>,
    #[serde(default)]
    hooks: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct McpServer {
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    url: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct Plan {
    pub whole_files: BTreeMap<String, Vec<u8>>,
    pub claude_hook_groups: Vec<(String, Value)>,
}

const ALL_KINDS: [AgentKind; 6] = [
    AgentKind::Claude,
    AgentKind::Codex,
    AgentKind::Cursor,
    AgentKind::OpenCode,
    AgentKind::Grok,
    AgentKind::Acp,
];

fn reads_agents_md(kind: AgentKind) -> bool {
    !matches!(kind, AgentKind::Claude | AgentKind::Cursor)
}

type SelectedInOrder = Vec<(String, Vec<AgentKind>)>;

impl SetupFile {
    pub fn parse(text: &str) -> Result<Self> {
        let file: SetupFile = toml::from_str(text)?;
        for set in &file.sets {
            for name in set
                .skills
                .iter()
                .chain(&set.instructions)
                .chain(&set.hooks)
                .chain(&set.mcp)
            {
                check_single_path_component(name)?;
            }
            if let Some(name) = set.mcp.iter().find(|n| !file.mcp.contains_key(*n)) {
                bail!("a set names the MCP server `{name}`, but there is no [mcp.{name}]");
            }
        }
        for (name, server) in &file.mcp {
            if server.command.is_some() == server.url.is_some() {
                bail!("[mcp.{name}] needs exactly one of `command` and `url`");
            }
        }
        Ok(file)
    }

    pub(super) fn plan(&self, root: &Path, project: &Project) -> Result<Plan> {
        let folder = Project::folder_name(&project.repo_path);
        let mut skills = SelectedInOrder::new();
        let mut instructions = SelectedInOrder::new();
        let mut mcp = SelectedInOrder::new();
        let mut hooks = SelectedInOrder::new();
        for set in &self.sets {
            let in_project = set
                .projects
                .as_ref()
                .is_none_or(|names| names.iter().any(|n| *n == project.name || *n == folder));
            if !in_project {
                continue;
            }
            let kinds = set.kinds.as_deref().unwrap_or(&ALL_KINDS);
            select(&mut skills, &set.skills, kinds);
            select(&mut instructions, &set.instructions, kinds);
            select(&mut mcp, &set.mcp, kinds);
            select(&mut hooks, &set.hooks, kinds);
        }

        let mut plan = Plan::default();
        for (name, kinds) in &skills {
            let src = root.join("skills").join(name);
            if !src.is_dir() {
                bail!("skill `{name}`: {} is not a directory", src.display());
            }
            if kinds.contains(&AgentKind::Claude) {
                copy_dir(
                    &src,
                    &format!(".claude/skills/{name}"),
                    &mut plan.whole_files,
                )?;
            }
            if kinds.iter().any(|k| *k != AgentKind::Claude) {
                copy_dir(
                    &src,
                    &format!(".agents/skills/{name}"),
                    &mut plan.whole_files,
                )?;
            }
        }

        let mut agents_md_sections = Vec::new();
        for (name, kinds) in &instructions {
            let body = read_text(&root.join("instructions").join(format!("{name}.md")))?;
            if kinds.contains(&AgentKind::Claude) {
                plan.whole_files.insert(
                    format!(".claude/rules/{name}.md"),
                    body.clone().into_bytes(),
                );
            }
            if kinds.contains(&AgentKind::Cursor) {
                plan.whole_files.insert(
                    format!(".cursor/rules/{name}.mdc"),
                    format!("---\ndescription: {name} (berth shared setup)\nalwaysApply: true\n---\n\n{body}")
                        .into_bytes(),
                );
            }
            if kinds.iter().any(|k| reads_agents_md(*k)) {
                agents_md_sections.push(body.trim_end().to_string());
            }
        }
        if !agents_md_sections.is_empty() {
            plan.whole_files.insert(
                "AGENTS.md".into(),
                format!(
                    "<!-- Written by berth from the shared setup. Edit it there: a change here stops berth updating this file. -->\n\n{}\n",
                    agents_md_sections.join("\n\n")
                )
                .into_bytes(),
            );
        }

        let mut claude_mcp = Map::new();
        let mut cursor_mcp = Map::new();
        let mut codex_mcp = toml::Table::new();
        for (name, kinds) in &mcp {
            let server = &self.mcp[name];
            if kinds.contains(&AgentKind::Claude) {
                claude_mcp.insert(name.clone(), server.json(RemoteShape::Typed));
            }
            if kinds.contains(&AgentKind::Cursor) {
                cursor_mcp.insert(name.clone(), server.json(RemoteShape::BareUrl));
            }
            if kinds.contains(&AgentKind::Codex) {
                codex_mcp.insert(name.clone(), server.toml());
            }
        }
        if !claude_mcp.is_empty() {
            plan.whole_files
                .insert(".mcp.json".into(), mcp_json(claude_mcp)?);
        }
        if !cursor_mcp.is_empty() {
            plan.whole_files
                .insert(".cursor/mcp.json".into(), mcp_json(cursor_mcp)?);
        }
        if !codex_mcp.is_empty() {
            let mut config = toml::Table::new();
            config.insert("mcp_servers".into(), toml::Value::Table(codex_mcp));
            plan.whole_files.insert(
                ".codex/config.toml".into(),
                toml::to_string(&config)?.into_bytes(),
            );
        }

        for (name, kinds) in &hooks {
            if !kinds.contains(&AgentKind::Claude) {
                continue;
            }
            let path = root.join("hooks").join(format!("{name}.json"));
            let events: Value = serde_json::from_str(&read_text(&path)?)
                .with_context(|| format!("{} is not valid JSON", path.display()))?;
            let Value::Object(events) = events else {
                bail!("{} is not an object of hook events", path.display());
            };
            for (event, groups) in events {
                let Value::Array(groups) = groups else {
                    bail!(
                        "{}: `{event}` is not an array of hook groups",
                        path.display()
                    );
                };
                for group in groups {
                    let Value::Object(mut group) = group else {
                        bail!("{}: a `{event}` group is not an object", path.display());
                    };
                    group.insert(SHARED_HOOK_TAG.into(), json!(true));
                    plan.claude_hook_groups
                        .push((event.clone(), Value::Object(group)));
                }
            }
        }
        Ok(plan)
    }
}

enum RemoteShape {
    Typed,
    BareUrl,
}

impl McpServer {
    fn json(&self, remote: RemoteShape) -> Value {
        if let Some(url) = &self.url {
            return match remote {
                RemoteShape::Typed => json!({ "type": "http", "url": url }),
                RemoteShape::BareUrl => json!({ "url": url }),
            };
        }
        let mut entry = json!({ "command": self.command, "args": self.args });
        if !self.env.is_empty() {
            entry["env"] = json!(self.env);
        }
        entry
    }

    fn toml(&self) -> toml::Value {
        let mut table = toml::Table::new();
        if let Some(url) = &self.url {
            table.insert("url".into(), url.clone().into());
            return toml::Value::Table(table);
        }
        table.insert(
            "command".into(),
            self.command.clone().unwrap_or_default().into(),
        );
        table.insert(
            "args".into(),
            toml::Value::Array(self.args.iter().cloned().map(Into::into).collect()),
        );
        if !self.env.is_empty() {
            let env = self.env.iter().map(|(k, v)| (k.clone(), v.clone().into()));
            table.insert("env".into(), toml::Value::Table(env.collect()));
        }
        toml::Value::Table(table)
    }
}

fn mcp_json(servers: Map<String, Value>) -> Result<Vec<u8>> {
    let mut text = serde_json::to_string_pretty(&json!({ "mcpServers": servers }))?;
    text.push('\n');
    Ok(text.into_bytes())
}

fn select(selected: &mut SelectedInOrder, names: &[String], kinds: &[AgentKind]) {
    for name in names {
        let at = match selected.iter().position(|(n, _)| n == name) {
            Some(at) => at,
            None => {
                selected.push((name.clone(), Vec::new()));
                selected.len() - 1
            }
        };
        for kind in kinds {
            if !selected[at].1.contains(kind) {
                selected[at].1.push(*kind);
            }
        }
    }
}

fn check_single_path_component(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', ':']) {
        bail!("`{name}` is not a valid item name");
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

fn copy_dir(src: &Path, rel: &str, files: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
    let entries = std::fs::read_dir(src).with_context(|| format!("read {}", src.display()))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        let path = entry.path();
        let target = format!("{rel}/{name}");
        if path.is_dir() {
            copy_dir(&path, &target, files)?;
        } else {
            let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            files.insert(target, bytes);
        }
    }
    Ok(())
}
