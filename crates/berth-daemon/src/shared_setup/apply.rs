use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::plan::{Plan, SHARED_HOOK_TAG};

const MANIFEST_IN_GIT_DIR: &str = "berth-setup.json";
const CLAUDE_LOCAL_SETTINGS: &str = ".claude/settings.local.json";
const EXCLUDE_BLOCK_BEGIN: &str = "# berth shared setup (rewritten on every sync)";
const EXCLUDE_BLOCK_END: &str = "# end berth shared setup";

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    #[serde(default)]
    files: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct Report {
    pub written: Vec<String>,
    pub removed: Vec<String>,
    pub kept_as_found: Vec<String>,
}

pub(super) fn apply(checkout: &Path, plan: &Plan) -> Result<Report> {
    let (git_dir, common_dir) = git_dirs(checkout)?;
    let mut report = Report::default();
    if merge_claude_hook_groups(checkout, &plan.claude_hook_groups)? {
        report.written.push(CLAUDE_LOCAL_SETTINGS.into());
    }

    let own_manifest = git_dir.join(MANIFEST_IN_GIT_DIR);
    let previous = read_manifest(&own_manifest)
        .or_else(|| read_manifest(&common_dir.join(MANIFEST_IN_GIT_DIR)))
        .unwrap_or_default();
    let mut next = Manifest::default();

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

    if !next.files.is_empty() || own_manifest.exists() {
        std::fs::write(&own_manifest, serde_json::to_string_pretty(&next)?)
            .with_context(|| format!("write {}", own_manifest.display()))?;
    }
    write_exclude_block(&common_dir, plan.whole_files.keys())?;
    Ok(report)
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

fn merge_claude_hook_groups(checkout: &Path, groups: &[(String, Value)]) -> Result<bool> {
    let path = checkout.join(CLAUDE_LOCAL_SETTINGS);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound && groups.is_empty() => return Ok(false),
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    let mut root: Value = if text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&text).with_context(|| {
            format!(
                "{} is not valid JSON; berth left this checkout alone",
                path.display()
            )
        })?
    };
    let before = root.clone();
    let not_an_object = || {
        format!(
            "{} is not a JSON object; berth left this checkout alone",
            path.display()
        )
    };
    let settings = root.as_object_mut().with_context(not_an_object)?;
    if let Some(events) = settings.get_mut("hooks").and_then(Value::as_object_mut) {
        for event_groups in events.values_mut() {
            if let Some(list) = event_groups.as_array_mut() {
                list.retain(|g| g.get(SHARED_HOOK_TAG).is_none());
            }
        }
        events.retain(|_, list| list.as_array().is_none_or(|l| !l.is_empty()));
    }
    if !groups.is_empty() {
        let events = settings
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .with_context(not_an_object)?;
        for (event, group) in groups {
            events
                .entry(event.clone())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .with_context(not_an_object)?
                .push(group.clone());
        }
    }
    if root == before {
        return Ok(false);
    }
    let dir = path.parent().unwrap_or(checkout);
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let tmp = dir.join(".settings.local.json.berth-tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&root)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("rename into {}", path.display()))?;
    Ok(true)
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
    let patterns: Vec<String> = paths.map(|p| format!("/{}", escape_glob(p))).collect();
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
