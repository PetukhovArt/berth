use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub struct ExcludeBlock {
    begin: &'static str,
    end: &'static str,
}

pub const SHARED_SETUP_BLOCK: ExcludeBlock = ExcludeBlock {
    begin: "# berth shared setup (rewritten on every sync)",
    end: "# end berth shared setup",
};

pub const CREATED_HOOK_FILES_BLOCK: ExcludeBlock = ExcludeBlock {
    begin: "# berth hook files (created by berth)",
    end: "# end berth hook files",
};

pub fn git_dirs(checkout: &Path) -> Result<(PathBuf, PathBuf)> {
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

pub fn replace_block<'a>(
    common_dir: &Path,
    block: &ExcludeBlock,
    paths: impl Iterator<Item = &'a String>,
) -> Result<()> {
    let patterns: BTreeSet<String> = paths.map(|p| anchored_pattern(p)).collect();
    rewrite(common_dir, block, |_| patterns)
}

pub fn add_to_block(checkout: &Path, block: &ExcludeBlock, path: &str) -> Result<()> {
    let (_, common_dir) = git_dirs(checkout)?;
    let added = anchored_pattern(path);
    rewrite(&common_dir, block, |mut held| {
        held.insert(added);
        held
    })
}

fn rewrite(
    common_dir: &Path,
    block: &ExcludeBlock,
    next_patterns: impl FnOnce(BTreeSet<String>) -> BTreeSet<String>,
) -> Result<()> {
    let exclude = common_dir.join("info").join("exclude");
    let current = std::fs::read_to_string(&exclude).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut held: BTreeSet<String> = BTreeSet::new();
    let mut inside_block = false;
    for line in current.lines() {
        if line == block.begin {
            inside_block = true;
        } else if line == block.end {
            inside_block = false;
        } else if inside_block {
            held.insert(line.to_string());
        } else {
            lines.push(line.to_string());
        }
    }
    let patterns = next_patterns(held);
    if !patterns.is_empty() {
        lines.push(block.begin.into());
        lines.extend(patterns);
        lines.push(block.end.into());
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

fn anchored_pattern(path: &str) -> String {
    format!("/{}", escape_glob(path))
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
