use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::hooks::CODEX_GUARD_FILE;
use super::plan::Plan;
use super::settings::{
    is_bare, merge_keys, merge_tagged_hooks, read_json, write_json, TrackedKey, WantedKey,
};

const MANIFEST_IN_GIT_DIR: &str = "berth-setup.json";
const CLAUDE_LOCAL_SETTINGS: &str = ".claude/settings.local.json";
const CLAUDE_SETTINGS: &str = ".claude/settings.json";
const CURSOR_HOOKS: &str = ".cursor/hooks.json";
const CLAUDE_MCP_FILE: &str = ".mcp.json";
const MCP_APPROVALS: &str = "enabledMcpjsonServers";
const EXCLUDE_BLOCK_BEGIN: &str = "# berth shared setup (rewritten on every sync)";
const EXCLUDE_BLOCK_END: &str = "# end berth shared setup";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keys: Vec<TrackedKey>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    created: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub struct Report {
    pub written: Vec<String>,
    pub removed: Vec<String>,
    pub kept_as_found: Vec<String>,
    pub skipped: Vec<String>,
}

pub(super) fn apply(checkout: &Path, plan: &Plan) -> Result<Report> {
    let (git_dir, common_dir) = git_dirs(checkout)?;
    let mut report = Report {
        skipped: plan.skipped.clone(),
        ..Report::default()
    };

    let own_manifest = git_dir.join(MANIFEST_IN_GIT_DIR);
    let previous = read_manifest(&own_manifest)
        .or_else(|| read_manifest(&common_dir.join(MANIFEST_IN_GIT_DIR)))
        .unwrap_or_default();
    let mut next = Manifest::default();

    write_whole_files(checkout, plan, &previous, &mut next, &mut report)?;

    let approvals: Vec<WantedKey> = if report.kept_as_found.iter().any(|f| f == CLAUDE_MCP_FILE) {
        Vec::new()
    } else {
        plan.claude_mcp_servers
            .iter()
            .map(|name| WantedKey::member(MCP_APPROVALS, name))
            .collect()
    };
    let plugin_keys: Vec<WantedKey> = plan
        .claude_settings_keys
        .iter()
        .map(|(field, key, value)| WantedKey::key(field, key, value.clone()))
        .collect();

    let files = [
        SettingsFile {
            rel: CLAUDE_LOCAL_SETTINGS,
            hook_entries: &plan.claude_hook_groups,
            versioned: false,
            keys: &approvals,
        },
        SettingsFile {
            rel: CURSOR_HOOKS,
            hook_entries: &plan.cursor_hook_entries,
            versioned: true,
            keys: &[],
        },
        SettingsFile {
            rel: CLAUDE_SETTINGS,
            hook_entries: &[],
            versioned: false,
            keys: &plugin_keys,
        },
    ];
    for file in &files {
        merge_settings_file(checkout, file, &previous, &mut next, &mut report)?;
    }

    write_codex_guard(&common_dir, &plan.codex_hook_items)?;

    if !next.files.is_empty()
        || !next.keys.is_empty()
        || !next.created.is_empty()
        || own_manifest.exists()
    {
        std::fs::write(&own_manifest, serde_json::to_string_pretty(&next)?)
            .with_context(|| format!("write {}", own_manifest.display()))?;
    }
    write_exclude_block(
        &common_dir,
        plan.whole_files.keys().chain(next.created.iter()),
    )?;
    Ok(report)
}

fn write_whole_files(
    checkout: &Path,
    plan: &Plan,
    previous: &Manifest,
    next: &mut Manifest,
    report: &mut Report,
) -> Result<()> {
    for (rel, content) in &plan.whole_files {
        let path = checkout.join(rel);
        let wanted = fingerprint(content);
        match std::fs::read(&path) {
            Ok(current) => {
                let found = fingerprint(&current);
                if found == wanted {
                    next.files.insert(rel.clone(), wanted);
                    continue;
                }
                if previous.files.get(rel) != Some(&found) {
                    report.kept_as_found.push(rel.clone());
                    continue;
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(_) => {
                report.kept_as_found.push(rel.clone());
                continue;
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
        next.files.insert(rel.clone(), wanted);
        report.written.push(rel.clone());
    }

    for (rel, written_fingerprint) in &previous.files {
        if plan.whole_files.contains_key(rel) {
            continue;
        }
        let path = checkout.join(rel);
        let unchanged_since_written =
            std::fs::read(&path).is_ok_and(|current| fingerprint(&current) == *written_fingerprint);
        if unchanged_since_written {
            std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
            remove_emptied_dirs(path.parent(), checkout);
            report.removed.push(rel.clone());
        }
    }
    Ok(())
}

struct SettingsFile<'a> {
    rel: &'static str,
    hook_entries: &'a [(String, Value)],
    versioned: bool,
    keys: &'a [WantedKey],
}

fn merge_settings_file(
    checkout: &Path,
    file: &SettingsFile,
    previous: &Manifest,
    next: &mut Manifest,
    report: &mut Report,
) -> Result<()> {
    let rel = file.rel;
    let path = checkout.join(rel);
    let (mut root, existed) = match read_json(&path)? {
        Some(root) => (root, true),
        None if file.hook_entries.is_empty() && file.keys.is_empty() => return Ok(()),
        None => (json!({}), false),
    };
    let before = root.clone();
    merge_tagged_hooks(&mut root, &path, file.hook_entries, file.versioned)?;
    let tracked_before: Vec<&TrackedKey> = previous.keys.iter().filter(|k| k.file == rel).collect();
    let outcome = merge_keys(&mut root, &path, rel, file.keys, &tracked_before)?;
    next.keys.extend(outcome.tracked);
    report
        .kept_as_found
        .extend(outcome.kept.into_iter().map(|k| format!("{rel} {k}")));

    let created_by_berth = !existed || previous.created.contains(rel);
    if created_by_berth && is_bare(&root) {
        if existed {
            std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
            remove_emptied_dirs(path.parent(), checkout);
            report.removed.push(rel.to_string());
        }
        return Ok(());
    }
    if created_by_berth {
        next.created.insert(rel.to_string());
    }
    if root != before {
        write_json(&path, &root)?;
        report.written.push(rel.to_string());
    }
    Ok(())
}

fn write_codex_guard(common_dir: &Path, items: &[String]) -> Result<()> {
    let path = common_dir.join(CODEX_GUARD_FILE);
    if items.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != ErrorKind::NotFound => {
                Err(e).with_context(|| format!("remove {}", path.display()))
            }
            _ => Ok(()),
        };
    }
    let text = format!("{}\n", items.join("\n"));
    if std::fs::read_to_string(&path).is_ok_and(|current| current == text) {
        return Ok(());
    }
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

pub(super) fn merge_codex_home(codex_home: &Path, groups: &[(String, Value)]) -> Result<bool> {
    if !codex_home.is_dir() {
        return Ok(false);
    }
    let path = codex_home.join("hooks.json");
    let (mut root, existed) = match read_json(&path)? {
        Some(root) => (root, true),
        None if groups.is_empty() => return Ok(false),
        None => (json!({}), false),
    };
    let before = root.clone();
    merge_tagged_hooks(&mut root, &path, groups, false)?;
    if root == before || (!existed && is_bare(&root)) {
        return Ok(false);
    }
    write_json(&path, &root)?;
    Ok(true)
}

fn git_dirs(checkout: &Path) -> Result<(PathBuf, PathBuf)> {
    let dot_git = checkout.join(".git");
    let meta = std::fs::metadata(&dot_git)
        .with_context(|| format!("{} is not a git checkout", checkout.display()))?;
    if meta.is_dir() {
        return Ok((dot_git.clone(), dot_git));
    }
    let text =
        std::fs::read_to_string(&dot_git).with_context(|| format!("read {}", dot_git.display()))?;
    let linked = text
        .trim()
        .strip_prefix("gitdir:")
        .with_context(|| format!("{} does not name a git dir", dot_git.display()))?
        .trim();
    let git_dir = checkout.join(linked);
    let common_dir = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(common) => git_dir.join(common.trim()),
        Err(_) => git_dir.clone(),
    };
    Ok((git_dir, common_dir))
}

fn read_manifest(path: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(manifest) => Some(manifest),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "ignoring an unreadable shared setup manifest");
            None
        }
    }
}

pub(super) fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn remove_emptied_dirs(mut dir: Option<&Path>, checkout: &Path) {
    while let Some(current) = dir {
        if current == checkout || !current.starts_with(checkout) {
            return;
        }
        if std::fs::remove_dir(current).is_err() {
            return;
        }
        dir = current.parent();
    }
}

fn write_exclude_block<'a>(
    common_dir: &Path,
    paths: impl Iterator<Item = &'a String>,
) -> Result<()> {
    let exclude = common_dir.join("info").join("exclude");
    let current = std::fs::read_to_string(&exclude).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut inside_block = false;
    for line in current.lines() {
        match line {
            EXCLUDE_BLOCK_BEGIN => inside_block = true,
            EXCLUDE_BLOCK_END => inside_block = false,
            _ if !inside_block => lines.push(line.to_string()),
            _ => {}
        }
    }
    let patterns: BTreeSet<String> = paths.map(|p| format!("/{}", escape_glob(p))).collect();
    if !patterns.is_empty() {
        lines.push(EXCLUDE_BLOCK_BEGIN.into());
        lines.extend(patterns);
        lines.push(EXCLUDE_BLOCK_END.into());
    }
    let mut next = lines.join("\n");
    if !next.is_empty() {
        next.push('\n');
    }
    if next == current {
        return Ok(());
    }
    if let Some(parent) = exclude.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&exclude, next).with_context(|| format!("write {}", exclude.display()))
}

fn escape_glob(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '\\' | '*' | '?' | '[') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
