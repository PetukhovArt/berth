use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use berth_core::{AgentKind, Project};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::hooks::{grok_file, HookItem};
use super::mcp::{mcp_json, mcp_toml, opencode_json, pretty, McpServer, RemoteShape};
use crate::agent::setup::{HookFormat, McpFormat, RuleFormat, Setup};
use crate::agent::{self, ALL};

pub const SETUP_FILE: &str = "berth-setup.toml";

const GROK_HOOKS_FILE: &str = ".grok/hooks/berth-shared.json";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupFile {
    #[serde(default, rename = "set")]
    sets: Vec<Set>,
    #[serde(default)]
    mcp: BTreeMap<String, McpServer>,
    #[serde(default)]
    plugins: BTreeMap<String, Plugin>,
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
    #[serde(default)]
    plugins: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plugin {
    marketplace: String,
    plugin: Option<String>,
    source: Option<Value>,
}

#[derive(Debug, Default)]
pub(super) struct Plan {
    pub whole_files: BTreeMap<String, Vec<u8>>,
    pub claude_hook_groups: Vec<(String, Value)>,
    pub cursor_hook_entries: Vec<(String, Value)>,
    pub codex_hook_items: Vec<String>,
    pub claude_mcp_servers: Vec<String>,
    pub claude_settings_keys: Vec<(String, String, Value)>,
    pub skipped: Vec<String>,
}

fn setup_of(kind: AgentKind) -> &'static Setup {
    agent::of(kind).setup
}

enum McpFile {
    Json(RemoteShape, Map<String, Value>),
    OpenCode(Map<String, Value>),
    Toml(toml::Table),
}

impl McpFile {
    fn new(format: &McpFormat) -> Self {
        match format {
            McpFormat::Json { typed_remote } => Self::Json(
                if *typed_remote {
                    RemoteShape::Typed
                } else {
                    RemoteShape::BareUrl
                },
                Map::new(),
            ),
            McpFormat::OpenCode => Self::OpenCode(Map::new()),
            McpFormat::Toml => Self::Toml(toml::Table::new()),
        }
    }

    fn insert(&mut self, name: &str, server: &McpServer) {
        let name = name.to_string();
        match self {
            Self::Json(shape, map) => {
                map.insert(name, server.json(*shape));
            }
            Self::OpenCode(map) => {
                map.insert(name, server.opencode());
            }
            Self::Toml(table) => {
                table.insert(name, server.toml());
            }
        }
    }

    fn render(self) -> Result<Vec<u8>> {
        match self {
            Self::Json(_, map) => mcp_json(map),
            Self::OpenCode(map) => opencode_json(map),
            Self::Toml(table) => mcp_toml(table),
        }
    }
}

type SelectedInOrder = Vec<(String, Vec<AgentKind>)>;

#[derive(Default)]
struct Selection {
    skills: SelectedInOrder,
    instructions: SelectedInOrder,
    mcp: SelectedInOrder,
    hooks: SelectedInOrder,
    plugins: SelectedInOrder,
}

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
            if let Some(name) = set.plugins.iter().find(|n| !file.plugins.contains_key(*n)) {
                bail!("a set names the plugin `{name}`, but there is no [plugins.{name}]");
            }
        }
        for (name, server) in &file.mcp {
            if server.command.is_some() == server.url.is_some() {
                bail!("[mcp.{name}] needs exactly one of `command` and `url`");
            }
        }
        let mut sources: BTreeMap<&str, &Value> = BTreeMap::new();
        for (name, plugin) in &file.plugins {
            if plugin.marketplace.trim().is_empty() {
                bail!("[plugins.{name}] needs a `marketplace`");
            }
            let Some(source) = &plugin.source else {
                continue;
            };
            if !source.is_object() {
                bail!("[plugins.{name}] `source` must be a table, like {{ source = \"github\", repo = \"owner/repo\" }}");
            }
            match sources.insert(&plugin.marketplace, source) {
                Some(other) if other != source => bail!(
                    "the marketplace `{}` has two different sources",
                    plugin.marketplace
                ),
                _ => {}
            }
        }
        Ok(file)
    }

    fn select(&self, project: &Project) -> Selection {
        let folder = Project::folder_name(&project.repo_path);
        let mut selection = Selection::default();
        for set in &self.sets {
            let in_project = set
                .projects
                .as_ref()
                .is_none_or(|names| names.iter().any(|n| *n == project.name || *n == folder));
            if !in_project {
                continue;
            }
            let kinds = set.kinds.as_deref().unwrap_or(&ALL);
            select(&mut selection.skills, &set.skills, kinds);
            select(&mut selection.instructions, &set.instructions, kinds);
            select(&mut selection.mcp, &set.mcp, kinds);
            select(&mut selection.hooks, &set.hooks, kinds);
            select(&mut selection.plugins, &set.plugins, kinds);
        }
        selection
    }

    pub(super) fn plan(&self, root: &Path, project: &Project) -> Result<Plan> {
        let selection = self.select(project);
        let mut plan = Plan::default();
        plan_skills(root, &selection.skills, &mut plan)?;
        plan_instructions(root, &selection.instructions, &mut plan)?;
        self.plan_mcp(&selection.mcp, &mut plan)?;
        plan_hooks(root, &selection.hooks, &mut plan)?;
        self.plan_plugins(&selection.plugins, &mut plan);
        let mut seen = std::collections::BTreeSet::new();
        plan.skipped.retain(|line| seen.insert(line.clone()));
        Ok(plan)
    }

    fn plan_mcp(&self, mcp: &SelectedInOrder, plan: &mut Plan) -> Result<()> {
        let mut files: BTreeMap<&str, McpFile> = BTreeMap::new();
        for (name, kinds) in mcp {
            let server = &self.mcp[name];
            for target in kinds.iter().filter_map(|k| setup_of(*k).mcp.as_ref()) {
                files
                    .entry(target.file)
                    .or_insert_with(|| McpFile::new(&target.format))
                    .insert(name, server);
                if target.approved_in_settings {
                    plan.claude_mcp_servers.push(name.clone());
                }
            }
        }
        for (file, servers) in files {
            plan.whole_files.insert(file.into(), servers.render()?);
        }
        Ok(())
    }

    fn plan_plugins(&self, plugins: &SelectedInOrder, plan: &mut Plan) {
        for (name, kinds) in plugins {
            if !kinds.iter().any(|k| setup_of(*k).plugins) {
                continue;
            }
            let plugin = &self.plugins[name];
            let id = format!(
                "{}@{}",
                plugin.plugin.as_deref().unwrap_or(name),
                plugin.marketplace
            );
            plan.claude_settings_keys
                .push(("enabledPlugins".into(), id, json!(true)));
            if let Some(source) = &plugin.source {
                let entry = (
                    "extraKnownMarketplaces".to_string(),
                    plugin.marketplace.clone(),
                    json!({ "source": source }),
                );
                if !plan.claude_settings_keys.contains(&entry) {
                    plan.claude_settings_keys.push(entry);
                }
            }
        }
    }

    pub(super) fn codex_hook_groups_of_every_project(
        &self,
        root: &Path,
    ) -> Result<Vec<(String, Value)>> {
        let mut names: Vec<&String> = Vec::new();
        for set in &self.sets {
            let kinds = set.kinds.as_deref().unwrap_or(&ALL);
            if hook_setup(kinds, HookFormat::Codex).is_none() {
                continue;
            }
            for name in &set.hooks {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        let mut groups = Vec::new();
        for name in names {
            groups.extend(HookItem::read(root, name)?.codex(&mut Vec::new()));
        }
        Ok(groups)
    }

    pub(super) fn acp_servers(&self, project: &Project, http: bool) -> Vec<Value> {
        self.select(project)
            .mcp
            .iter()
            .filter(|(_, kinds)| kinds.iter().any(|k| setup_of(*k).mcp_over_acp))
            .filter_map(|(name, _)| self.mcp[name].acp(name, http))
            .collect()
    }
}

fn plan_skills(root: &Path, skills: &SelectedInOrder, plan: &mut Plan) -> Result<()> {
    for (name, kinds) in skills {
        let src = root.join("skills").join(name);
        if !src.is_dir() {
            bail!("skill `{name}`: {} is not a directory", src.display());
        }
        let mut dirs: Vec<&str> = kinds.iter().map(|k| setup_of(*k).skills).collect();
        dirs.sort();
        dirs.dedup();
        for dir in dirs {
            copy_dir(&src, &format!("{dir}/{name}"), &mut plan.whole_files)?;
        }
    }
    Ok(())
}

fn plan_instructions(root: &Path, instructions: &SelectedInOrder, plan: &mut Plan) -> Result<()> {
    let mut agents_md_sections = Vec::new();
    for (name, kinds) in instructions {
        let body = read_text(&root.join("instructions").join(format!("{name}.md")))?;
        for rule in kinds.iter().filter_map(|k| setup_of(*k).rule.as_ref()) {
            let text = match rule.format {
                RuleFormat::Plain => body.clone(),
                RuleFormat::AlwaysApply => format!(
                    "---\ndescription: {name} (berth shared setup)\nalwaysApply: true\n---\n\n{body}"
                ),
            };
            let path = format!("{}/{name}.{}", rule.dir, rule.ext);
            plan.whole_files.insert(path, text.into_bytes());
        }
        if kinds.iter().any(|k| setup_of(*k).reads_agents_md) {
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
    Ok(())
}

fn plan_hooks(root: &Path, hooks: &SelectedInOrder, plan: &mut Plan) -> Result<()> {
    let mut grok_groups = Vec::new();
    for (name, kinds) in hooks {
        let item = HookItem::read(root, name)?;
        for format in HookFormat::ALL {
            let Some(setup) = hook_setup(kinds, format) else {
                continue;
            };
            let imported = setup.imports_hooks_of.iter().any(|k| kinds.contains(k));
            match format {
                HookFormat::Claude => plan.claude_hook_groups.extend(item.claude()),
                HookFormat::Cursor => {
                    let entries = item.cursor(&mut plan.skipped);
                    if !imported {
                        plan.cursor_hook_entries.extend(entries);
                    }
                }
                HookFormat::Codex => {
                    if !item.codex(&mut plan.skipped).is_empty() {
                        plan.codex_hook_items.push(name.clone());
                    }
                }
                HookFormat::Grok => {
                    let groups = item.grok(&mut plan.skipped);
                    if !imported {
                        grok_groups.extend(groups);
                    }
                }
            }
        }
    }
    if !grok_groups.is_empty() {
        plan.whole_files
            .insert(GROK_HOOKS_FILE.into(), pretty(&grok_file(grok_groups))?);
    }
    Ok(())
}

fn hook_setup(kinds: &[AgentKind], format: HookFormat) -> Option<&'static Setup> {
    let mut setups = kinds.iter().map(|k| setup_of(*k));
    setups.find(|s| s.hooks == Some(format))
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
