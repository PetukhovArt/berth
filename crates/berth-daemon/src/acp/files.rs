use crate::git::git;
use anyhow::Result;
use std::path::Path;

const MAX_LIMIT: usize = 200;
const SCATTERED: usize = 1000;

pub async fn list(root: &Path, query: &str, limit: usize) -> Result<Vec<String>> {
    let listed = git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )
    .await?;
    let query = query.trim().to_lowercase();
    let mut ranked: Vec<(usize, &str)> = listed
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter_map(|path| Some((score(path, &query)?, path)))
        .collect();
    ranked.sort_unstable();
    Ok(ranked
        .into_iter()
        .take(limit.min(MAX_LIMIT))
        .map(|(_, path)| path.to_string())
        .collect())
}

fn score(path: &str, query: &str) -> Option<usize> {
    let lower = path.to_lowercase();
    if query.is_empty() {
        return Some(path.len());
    }
    let name_start = lower.rfind('/').map_or(0, |at| at + 1);
    if let Some(at) = lower[name_start..].find(query) {
        return Some(at + path.len());
    }
    let mut chars = lower.char_indices();
    let mut first = None;
    let mut last = 0;
    for wanted in query.chars() {
        let (at, _) = chars.find(|(_, c)| *c == wanted)?;
        first.get_or_insert(at);
        last = at;
    }
    Some(SCATTERED + last - first.unwrap_or(last) + path.len())
}
