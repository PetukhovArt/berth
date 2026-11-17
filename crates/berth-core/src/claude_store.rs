use std::fs;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 12;

/// The name Claude Code gives the directory under `~/.claude/projects`
/// holding the transcripts of sessions run in `path`: every character that
/// is not an ASCII letter or digit becomes `-`, so `D:\repo\wt` becomes
/// `D--repo-wt`.
pub fn slug(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The cwd to show and resume a transcript in on this machine.
///
/// A transcript records the cwd of the machine it was written on; one
/// synced in from another machine names a path that does not exist here.
/// Its directory name is still the slug of a checkout on this machine
/// (that is where Claude Code on this machine looks for it), so a missing
/// cwd is replaced by the directory that slug names, when one does.
pub fn local_cwd(recorded: &str, transcript: &Path) -> Option<PathBuf> {
    if recorded.is_empty() || Path::new(recorded).exists() {
        return None;
    }
    let dir_slug = transcript
        .parent()?
        .file_name()?
        .to_string_lossy()
        .into_owned();
    path_of_slug(&dir_slug)
}

/// An existing directory whose [`slug`] is `slug`, found by walking the
/// filesystem one matching name at a time. The slug is lossy, so the first
/// match wins.
pub fn path_of_slug(slug: &str) -> Option<PathBuf> {
    roots(slug).into_iter().find_map(|root| {
        let rest = strip_prefix_ignore_case(slug, &self::slug(&root))?;
        descend(&root, rest, 0)
    })
}

fn roots(slug: &str) -> Vec<PathBuf> {
    if cfg!(windows) {
        slug.chars()
            .next()
            .filter(char::is_ascii_alphabetic)
            .map(|drive| vec![PathBuf::from(format!("{drive}:\\"))])
            .unwrap_or_default()
    } else {
        vec![PathBuf::from("/")]
    }
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

fn descend(dir: &Path, rest: &str, depth: usize) -> Option<PathBuf> {
    if rest.is_empty() {
        return Some(dir.to_path_buf());
    }
    if depth >= MAX_DEPTH {
        return None;
    }
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let part = slug(Path::new(&entry.file_name()));
        let Some(next) = strip_prefix_ignore_case(rest, &part) else {
            continue;
        };
        let next = match next.strip_prefix('-') {
            Some(after) => after,
            None if next.is_empty() => next,
            None => continue,
        };
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = descend(&path, next, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_synced_transcript_resolves_to_the_checkout_its_directory_names() {
        let base = tempfile::tempdir().unwrap();
        let long_base = fs::canonicalize(base.path()).unwrap();
        let long_base = long_base
            .to_str()
            .and_then(|path| path.strip_prefix(r"\\?\"))
            .map_or(long_base.clone(), PathBuf::from);
        let checkout = long_base.join("web-projects").join("web-client");
        let worktree = checkout.join(".claude").join("worktrees").join("x-1");
        fs::create_dir_all(&worktree).unwrap();

        let store = tempfile::tempdir().unwrap();
        let transcript = store.path().join(slug(&checkout)).join("s.jsonl");
        let elsewhere = "/home/someone-else/WebstormProjects/web-client";
        assert_eq!(local_cwd(elsewhere, &transcript), Some(checkout.clone()));
        assert_eq!(path_of_slug(&slug(&worktree)), Some(worktree));
        assert_eq!(local_cwd(&checkout.to_string_lossy(), &transcript), None);
    }
}
