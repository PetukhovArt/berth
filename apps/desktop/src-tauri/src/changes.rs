use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::task::spawn_blocking;

use crate::connectors::types::{count_patch, DiffFile, FileStatus};

const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PATCH_LINES: usize = 2000;
const MAX_PATCH_BYTES: usize = 512 * 1024;
const BINARY_PROBE: usize = 8000;
const NO_NEWLINE: &str = "\\ No newline at end of file";

struct Git {
    root: PathBuf,
}

impl Git {
    async fn run(&self, args: &[&str]) -> Result<(bool, Vec<u8>), String> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let run = async {
            crate::platform::spawn_tied(&mut command)?
                .wait_with_output()
                .await
        };
        let output = tokio::time::timeout(GIT_TIMEOUT, run)
            .await
            .map_err(|_| {
                format!(
                    "git {} не ответил за {} с",
                    args.join(" "),
                    GIT_TIMEOUT.as_secs()
                )
            })?
            .map_err(|error| format!("не удалось запустить git: {error}"))?;
        if output.status.success() {
            Ok((true, output.stdout))
        } else {
            Ok((false, output.stderr))
        }
    }

    async fn bytes(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        match self.run(args).await? {
            (true, out) => Ok(out),
            (false, err) => Err(format!(
                "git {}: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&err).trim()
            )),
        }
    }

    async fn line(&self, args: &[&str]) -> Option<String> {
        match self.run(args).await {
            Ok((true, out)) => {
                let text = String::from_utf8_lossy(&out).trim().to_owned();
                (!text.is_empty()).then_some(text)
            }
            _ => None,
        }
    }

    async fn base(&self) -> Option<String> {
        let head = self
            .line(&["rev-parse", "--verify", "--quiet", "HEAD"])
            .await?;
        let Some(default) = self.default_branch().await else {
            return Some(head);
        };
        let branch = self
            .line(&["symbolic-ref", "--quiet", "--short", "HEAD"])
            .await;
        let default_name = default.strip_prefix("origin/").unwrap_or(&default);
        if branch.as_deref() == Some(default_name) {
            return Some(head);
        }
        Some(
            self.line(&["merge-base", &default, "HEAD"])
                .await
                .unwrap_or(head),
        )
    }

    async fn default_branch(&self) -> Option<String> {
        if let Some(remote) = self
            .line(&[
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ])
            .await
        {
            return Some(remote);
        }
        for candidate in ["main", "master", "origin/main", "origin/master"] {
            let spec = format!("{candidate}^{{commit}}");
            if self
                .line(&["rev-parse", "--verify", "--quiet", &spec])
                .await
                .is_some()
            {
                return Some(candidate.to_owned());
            }
        }
        None
    }

    async fn tracked(&self, base: &str) -> Result<Vec<DiffFile>, String> {
        let flags = ["-M", "--no-color", "--no-ext-diff"];
        let raw_args = [&["diff", base, "--raw", "-z"][..], &flags].concat();
        let numstat_args = [&["diff", base, "--numstat", "-z"][..], &flags].concat();
        let patch_args = [&["diff", base][..], &flags].concat();
        let (raw, numstat, patch) = tokio::join!(
            self.bytes(&raw_args),
            self.bytes(&numstat_args),
            self.bytes(&patch_args)
        );
        let entries = parse_raw(&raw?);
        let counts = parse_numstat(&numstat?);
        let patch = String::from_utf8_lossy(&patch?).into_owned();
        let blocks = split_blocks(&patch);
        let aligned = blocks.len() == entries.len() && counts.len() == entries.len();
        let mut files = Vec::with_capacity(entries.len());
        for (index, entry) in entries.into_iter().enumerate() {
            let block = if aligned {
                blocks[index].to_owned()
            } else {
                let mut args = vec!["diff", base];
                args.extend(flags);
                args.extend(["--", entry.old_path.as_str(), entry.new_path.as_str()]);
                String::from_utf8_lossy(&self.bytes(&args).await.unwrap_or_default()).into_owned()
            };
            let count = aligned.then(|| counts[index]);
            files.push(entry.into_file(&block, count));
        }
        Ok(files)
    }

    async fn untracked(&self) -> Result<Vec<String>, String> {
        let out = self
            .bytes(&["ls-files", "--others", "--exclude-standard", "-z"])
            .await?;
        Ok(out
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect())
    }
}

struct Entry {
    old_path: String,
    new_path: String,
    status: FileStatus,
}

impl Entry {
    fn into_file(self, block: &str, count: Option<Option<(u32, u32)>>) -> DiffFile {
        let hunks = block.find("\n@@").map_or("", |at| &block[at + 1..]);
        let binary = matches!(count, Some(None)) || (hunks.is_empty() && is_binary_block(block));
        let (additions, deletions) = match count {
            Some(Some(found)) => found,
            _ => count_patch(hunks),
        };
        let too_large = !binary && oversized(hunks);
        DiffFile {
            old_path: self.old_path,
            new_path: self.new_path,
            status: self.status,
            patch: if binary || too_large {
                String::new()
            } else {
                hunks.to_owned()
            },
            additions,
            deletions,
            binary,
            too_large,
        }
    }
}

fn is_binary_block(block: &str) -> bool {
    block
        .lines()
        .any(|line| line.starts_with("Binary files ") || line == "GIT binary patch")
}

fn oversized(patch: &str) -> bool {
    patch.len() > MAX_PATCH_BYTES || patch.lines().count() > MAX_PATCH_LINES
}

fn status_of(code: &str) -> FileStatus {
    match code.chars().next() {
        Some('A') | Some('C') => FileStatus::Added,
        Some('D') => FileStatus::Deleted,
        Some('R') => FileStatus::Renamed,
        _ => FileStatus::Modified,
    }
}

fn parse_raw(bytes: &[u8]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut fields = bytes
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned());
    while let Some(meta) = fields.next() {
        if !meta.starts_with(':') {
            continue;
        }
        let code = meta.rsplit(' ').next().unwrap_or_default().to_owned();
        let Some(first) = fields.next() else {
            break;
        };
        let paired = matches!(code.chars().next(), Some('R') | Some('C'));
        let second = if paired { fields.next() } else { None };
        let status = status_of(&code);
        let (old_path, new_path) = match second {
            Some(new_path) if status == FileStatus::Renamed => (first, new_path),
            Some(new_path) => (new_path.clone(), new_path),
            None => (first.clone(), first),
        };
        entries.push(Entry {
            old_path,
            new_path,
            status,
        });
    }
    entries
}

fn parse_numstat(bytes: &[u8]) -> Vec<Option<(u32, u32)>> {
    let mut counts = Vec::new();
    let mut fields = bytes.split(|b| *b == 0);
    while let Some(field) = fields.next() {
        let field = String::from_utf8_lossy(field);
        let mut parts = field.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if path.is_empty() {
            fields.next();
            fields.next();
        }
        counts.push(match (added.parse::<u32>(), deleted.parse::<u32>()) {
            (Ok(a), Ok(d)) => Some((a, d)),
            _ => None,
        });
    }
    counts
}

fn split_blocks(patch: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = Vec::new();
    if patch.starts_with("diff --git ") {
        starts.push(0);
    }
    starts.extend(patch.match_indices("\ndiff --git ").map(|(at, _)| at + 1));
    starts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = starts.get(i + 1).copied().unwrap_or(patch.len());
            &patch[start..end]
        })
        .collect()
}

fn untracked_file(root: &Path, path: String) -> DiffFile {
    let full = root.join(&path);
    let blank = |binary: bool, too_large: bool| DiffFile {
        old_path: path.clone(),
        new_path: path.clone(),
        status: FileStatus::Added,
        patch: String::new(),
        additions: 0,
        deletions: 0,
        binary,
        too_large,
    };
    let meta = match fs::symlink_metadata(&full) {
        Ok(meta) => meta,
        Err(_) => return blank(false, false),
    };
    let content = if meta.file_type().is_symlink() {
        match fs::read_link(&full) {
            Ok(target) => target.to_string_lossy().replace('\\', "/").into_bytes(),
            Err(_) => return blank(false, false),
        }
    } else {
        if meta.len() as usize > MAX_PATCH_BYTES {
            let binary = probe_binary(&full);
            return blank(binary, !binary);
        }
        match fs::read(&full) {
            Ok(bytes) => bytes,
            Err(_) => return blank(false, false),
        }
    };
    if content[..content.len().min(BINARY_PROBE)].contains(&0) {
        return blank(true, false);
    }
    let text = String::from_utf8_lossy(&content);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if lines.len() > MAX_PATCH_LINES {
        return DiffFile {
            additions: lines.len() as u32,
            ..blank(false, true)
        };
    }
    let mut patch = String::new();
    if !lines.is_empty() {
        patch.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
        for line in &lines {
            patch.push('+');
            patch.push_str(line);
        }
        if !text.ends_with('\n') {
            patch.push('\n');
            patch.push_str(NO_NEWLINE);
            patch.push('\n');
        }
    }
    DiffFile {
        additions: lines.len() as u32,
        patch,
        ..blank(false, false)
    }
}

fn probe_binary(path: &Path) -> bool {
    let mut head = vec![0u8; BINARY_PROBE];
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let read = file.read(&mut head).unwrap_or(0);
    head[..read].contains(&0)
}

pub async fn worktree_changes(path: String) -> Result<Vec<DiffFile>, String> {
    let probe = Git {
        root: PathBuf::from(&path),
    };
    let root = probe
        .line(&["rev-parse", "--show-toplevel"])
        .await
        .ok_or_else(|| format!("не git-репозиторий: {path}"))?;
    let git = Git {
        root: PathBuf::from(root),
    };
    let mut files = match git.base().await {
        Some(base) => git.tracked(&base).await?,
        None => Vec::new(),
    };
    let untracked = git.untracked().await?;
    let root = git.root.clone();
    let extra = spawn_blocking(move || {
        untracked
            .into_iter()
            .map(|path| untracked_file(&root, path))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|error| error.to_string())?;
    files.extend(extra);
    files.sort_by(|a, b| a.new_path.cmp(&b.new_path));
    Ok(files)
}
