use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use tokio::task::spawn_blocking;

use crate::agent_sessions::Harness;
use crate::connectors::ai::{claude_dir, codex_home};

const DESCRIPTION_CHARS: usize = 200;
const COMMAND_DEPTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
}

pub async fn slash_commands(harness: Harness, cwd: String) -> Result<Vec<SlashCommand>, String> {
    spawn_blocking(move || match harness {
        Harness::Claude => claude_commands(Path::new(&cwd)),
        Harness::Codex => codex_commands(),
    })
    .await
    .map_err(|e| e.to_string())
}

fn claude_commands(cwd: &Path) -> Vec<SlashCommand> {
    let mut out = Vec::new();
    let project = cwd.join(".claude");
    let user = claude_dir();
    for root in std::iter::once(project.clone()).chain(user.clone()) {
        scan_skills(&root.join("skills"), None, &mut out);
        scan_commands(&root.join("commands"), None, &mut out);
    }
    if let Some(user) = user {
        for (plugin, dir) in enabled_plugins(&user, &project, cwd) {
            scan_skills(&dir.join("skills"), Some(&plugin), &mut out);
            scan_commands(&dir.join("commands"), Some(&plugin), &mut out);
        }
    }
    dedupe(out)
}

fn codex_commands() -> Vec<SlashCommand> {
    let mut out = Vec::new();
    if let Some(home) = codex_home() {
        for path in markdown_files(&home.join("prompts"), 1) {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(command) = command_file(&path, format!("prompts:{stem}")) {
                out.push(command);
            }
        }
    }
    dedupe(out)
}

fn dedupe(commands: Vec<SlashCommand>) -> Vec<SlashCommand> {
    let mut seen = HashSet::new();
    commands
        .into_iter()
        .filter(|command| seen.insert(command.name.to_lowercase()))
        .collect()
}

fn qualified(plugin: Option<&str>, name: &str) -> String {
    match plugin {
        Some(plugin) => format!("{plugin}:{name}"),
        None => name.to_string(),
    }
}

fn scan_skills(dir: &Path, plugin: Option<&str>, out: &mut Vec<SlashCommand>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut found: Vec<SlashCommand> = entries
        .flatten()
        .filter_map(|entry| {
            let text = fs::read_to_string(entry.path().join("SKILL.md")).ok()?;
            let meta = frontmatter(&text);
            if meta.get("user-invocable").is_some_and(|v| v == "false") {
                return None;
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            let name = meta.get("name").cloned().unwrap_or(dir_name);
            Some(SlashCommand {
                name: qualified(plugin, &name),
                description: describe(&meta, &text),
            })
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(found);
}

fn scan_commands(dir: &Path, plugin: Option<&str>, out: &mut Vec<SlashCommand>) {
    let mut found: Vec<SlashCommand> = markdown_files(dir, COMMAND_DEPTH)
        .into_iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?.to_string();
            command_file(&path, qualified(plugin, &stem))
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(found);
}

fn command_file(path: &Path, name: String) -> Option<SlashCommand> {
    let text = fs::read_to_string(path).ok()?;
    let meta = frontmatter(&text);
    Some(SlashCommand {
        name,
        description: describe(&meta, &text),
    })
}

fn markdown_files(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 1 {
                out.extend(markdown_files(&path, depth - 1));
            }
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    out
}

fn frontmatter(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return out;
    }
    let mut folded: Option<String> = None;
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(key) = &folded {
            if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
                let entry = out.entry(key.clone()).or_insert_with(String::new);
                if !line.trim().is_empty() {
                    if !entry.is_empty() {
                        entry.push(' ');
                    }
                    entry.push_str(line.trim());
                }
                continue;
            }
            folded = None;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.starts_with(' ') || key.starts_with('\t') {
            continue;
        }
        let key = key.trim().to_string();
        let value = value.trim();
        if matches!(value, ">" | "|" | ">-" | "|-" | ">+" | "|+") {
            folded = Some(key);
            continue;
        }
        out.insert(key, unquote(value).to_string());
    }
    out
}

fn unquote(value: &str) -> &str {
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn describe(meta: &BTreeMap<String, String>, text: &str) -> String {
    let description = meta
        .get("description")
        .filter(|d| !d.is_empty())
        .cloned()
        .unwrap_or_else(|| first_line(text));
    let line = description.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(DESCRIPTION_CHARS) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line,
    }
}

fn first_line(text: &str) -> String {
    let body = match text.strip_prefix("---") {
        Some(rest) => rest.split_once("\n---").map(|(_, body)| body).unwrap_or(""),
        None => text,
    };
    body.lines()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| !line.is_empty() && *line != "---")
        .unwrap_or_default()
        .to_string()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn enabled_plugins(user: &Path, project: &Path, cwd: &Path) -> Vec<(String, PathBuf)> {
    let mut enabled = BTreeMap::new();
    for file in [
        user.join("settings.json"),
        project.join("settings.json"),
        project.join("settings.local.json"),
    ] {
        let Some(Value::Object(map)) =
            read_json(&file).and_then(|j| j.get("enabledPlugins").cloned())
        else {
            continue;
        };
        for (key, on) in map {
            enabled.insert(key, on.as_bool() == Some(true));
        }
    }
    let installed = read_json(&user.join("plugins").join("installed_plugins.json"));
    let Some(Value::Object(plugins)) = installed.as_ref().and_then(|j| j.get("plugins")) else {
        return Vec::new();
    };
    let here = normalized(cwd);
    enabled
        .into_iter()
        .filter(|(_, on)| *on)
        .filter_map(|(key, _)| {
            let installs = plugins.get(&key)?.as_array()?;
            let pick = installs
                .iter()
                .find(|i| {
                    i.get("projectPath")
                        .and_then(Value::as_str)
                        .map(|p| normalized(Path::new(p)))
                        == Some(here.clone())
                })
                .or_else(|| {
                    installs
                        .iter()
                        .find(|i| i.get("scope").and_then(Value::as_str) == Some("user"))
                })
                .or_else(|| installs.first())?;
            let path = PathBuf::from(pick.get("installPath")?.as_str()?);
            let name = key.split('@').next().unwrap_or(&key).to_string();
            Some((name, path))
        })
        .collect()
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}
