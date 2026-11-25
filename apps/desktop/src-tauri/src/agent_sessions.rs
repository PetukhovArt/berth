use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use berth_transcripts::transcript::{
    head_of, id_of, owned, read_subagent_transcript_file, read_tail_facts, read_transcript_file,
    title_of, valid_id, Head, Last, ReadFailed, Tail,
};
pub use berth_transcripts::transcript::{Harness, TranscriptMessage};
use serde::Serialize;
use serde_json::Value;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::connectors::ai::{claude_dir, codex_home, grok_home};
use crate::sink::BoxSink;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const DEFAULT_DAYS: u32 = 14;
const MAX_DAYS: u32 = 365;
const DAY_MS: u64 = 86_400_000;
const SESSION_LIMIT: usize = 500;
const FRESH_MS: u64 = 10_000;
const IDLE_MS: u64 = 4 * 3_600_000;
const START_SLACK_MS: u64 = 5_000;
const LIVE_ID_SLACK_MS: u64 = 120_000;
const HOST_DEPTH: usize = 8;
const GROK_TRANSCRIPT: &str = "chat_history.jsonl";
const CLAUDE_BATCH: [&str; 13] = [
    "-p",
    "--print",
    "mcp",
    "doctor",
    "update",
    "install",
    "setup-token",
    "config",
    "plugin",
    "-v",
    "--version",
    "-h",
    "--help",
];
const CODEX_BATCH: [&str; 16] = [
    "exec",
    "e",
    "mcp",
    "mcp-server",
    "app-server",
    "login",
    "logout",
    "completion",
    "apply",
    "a",
    "proto",
    "debug",
    "-V",
    "--version",
    "-h",
    "--help",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Finished,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AgentSession {
    pub(crate) harness: Harness,
    id: String,
    pub(crate) cwd: String,
    title: String,
    branch: Option<String>,
    pub(crate) updated_at: u64,
    status: Status,
    running: bool,
    host: Option<String>,
    in_berth: bool,
    berth_pids: Vec<u32>,
    messages: u32,
}

#[derive(Debug, Serialize)]
pub struct AgentSessionsChanged {
    sessions: Vec<AgentSession>,
}

#[derive(Default)]
pub struct AgentSessionWatch(Mutex<Option<JoinHandle<()>>>);

impl Drop for AgentSessionWatch {
    fn drop(&mut self) {
        let slot = self.0.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = slot.take() {
            task.abort();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Signature {
    modified: u64,
    len: u64,
}

struct Entry {
    harness: Harness,
    id: String,
    cwd_key: Option<String>,
    signature: Signature,
    head: Head,
    tail: Tail,
}

struct Found {
    harness: Harness,
    path: PathBuf,
    signature: Signature,
}

struct Proc {
    harness: Harness,
    cwd: Option<String>,
    resume: Option<String>,
    started: u64,
    host: Option<String>,
    berth: bool,
    berth_pids: Vec<u32>,
}

struct Claim {
    host: Option<String>,
    berth: bool,
    berth_pids: Vec<u32>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

fn system() -> &'static Mutex<System> {
    static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();
    SYSTEM.get_or_init(|| Mutex::new(System::new()))
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn now_ms() -> u64 {
    millis(SystemTime::now())
}

fn signature_of(meta: &fs::Metadata) -> Signature {
    Signature {
        modified: meta.modified().map(millis).unwrap_or(0),
        len: meta.len(),
    }
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    path.trim_end_matches('/').to_lowercase()
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn claude_root() -> Option<PathBuf> {
    claude_dir().map(|dir| dir.join("projects"))
}

fn claude_live_id(pid: Pid, started: u64) -> Option<String> {
    let file = claude_dir()?.join("sessions").join(format!("{pid}.json"));
    let value: Value = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    let at = value["startedAt"].as_u64()?;
    if at.abs_diff(started) > LIVE_ID_SLACK_MS {
        return None;
    }
    owned(&value["sessionId"]).filter(|id| valid_id(id))
}

fn codex_root() -> Option<PathBuf> {
    codex_home().map(|dir| dir.join("sessions"))
}

fn grok_root() -> Option<PathBuf> {
    grok_home().map(|dir| dir.join("sessions"))
}

fn grok_files(root: &Path, since: u64, out: &mut Vec<Found>) {
    for (_, folder) in subdirs(root) {
        for (_, session) in subdirs(&folder) {
            let path = session.join(GROK_TRANSCRIPT);
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            let signature = signature_of(&meta);
            if meta.is_file() && signature.modified >= since {
                out.push(Found {
                    harness: Harness::Grok,
                    path,
                    signature,
                });
            }
        }
    }
}

fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .collect()
}

fn jsonl_files(dir: &Path, harness: Harness, since: u64, out: &mut Vec<Found>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("jsonl") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let signature = signature_of(&meta);
        if signature.modified >= since {
            out.push(Found {
                harness,
                path,
                signature,
            });
        }
    }
}

fn codex_days(root: &Path, since_day: i64) -> Vec<PathBuf> {
    let number = |name: &str| name.parse::<i64>().ok();
    let mut days = Vec::new();
    for (year_name, year_dir) in subdirs(root) {
        let Some(year) = number(&year_name) else {
            continue;
        };
        if days_from_civil(year + 1, 1, 1) <= since_day {
            continue;
        }
        for (month_name, month_dir) in subdirs(&year_dir) {
            let Some(month) = number(&month_name) else {
                continue;
            };
            let next = if month >= 12 {
                days_from_civil(year + 1, 1, 1)
            } else {
                days_from_civil(year, month + 1, 1)
            };
            if next <= since_day {
                continue;
            }
            for (day_name, day_dir) in subdirs(&month_dir) {
                let Some(day) = number(&day_name) else {
                    continue;
                };
                if days_from_civil(year, month, day) >= since_day {
                    days.push(day_dir);
                }
            }
        }
    }
    days
}

fn scan(since: u64) -> Vec<Found> {
    let mut found = Vec::new();
    if let Some(root) = claude_root() {
        for (_, dir) in subdirs(&root) {
            jsonl_files(&dir, Harness::Claude, since, &mut found);
        }
    }
    if let Some(root) = codex_root() {
        let since_day = i64::try_from(since / DAY_MS).unwrap_or(0) - 1;
        for dir in codex_days(&root, since_day) {
            jsonl_files(&dir, Harness::Codex, since, &mut found);
        }
    }
    if let Some(root) = grok_root() {
        grok_files(&root, since, &mut found);
    }
    found
}

fn refresh_entry(cache: &mut HashMap<PathBuf, Entry>, found: &Found) {
    if let Some(entry) = cache.get_mut(&found.path) {
        if entry.signature != found.signature {
            entry.signature = found.signature;
            entry.tail = read_tail_facts(&found.path, found.harness);
            if entry.head.prompt.is_none() || entry.cwd_key.is_none() {
                entry.head = head_of(found.harness, &found.path);
                entry.cwd_key = entry.head.cwd.as_deref().map(normalize);
            }
        }
        return;
    }
    let head = head_of(found.harness, &found.path);
    let tail = read_tail_facts(&found.path, found.harness);
    cache.insert(
        found.path.clone(),
        Entry {
            harness: found.harness,
            id: id_of(found.harness, &found.path, &head),
            cwd_key: head.cwd.as_deref().map(normalize),
            signature: found.signature,
            head,
            tail,
        },
    );
}

fn stem_of(name: &OsStr) -> String {
    let name = name.to_string_lossy().to_ascii_lowercase();
    name.strip_suffix(".exe").map(str::to_owned).unwrap_or(name)
}

const LIVE: [(Harness, &str); 2] = [
    (Harness::Claude, "@anthropic-ai/claude-code"),
    (Harness::Codex, "@openai/codex"),
];

fn arg_harness(arg: &str) -> Option<Harness> {
    let arg = arg.replace('\\', "/").to_ascii_lowercase();
    let file = arg.rsplit('/').next().unwrap_or(&arg);
    let file = file
        .strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".js"))
        .or_else(|| file.strip_suffix(".cmd"))
        .unwrap_or(file);
    LIVE.into_iter()
        .find(|(harness, package)| arg.contains(package) || file == harness.program())
        .map(|(harness, _)| harness)
}

fn harness_of(process: &Process) -> Option<Harness> {
    arg_harness(&stem_of(process.name())).or_else(|| {
        process
            .cmd()
            .iter()
            .skip(1)
            .take(2)
            .find_map(|arg| arg_harness(&arg.to_string_lossy()))
    })
}

fn harness_args(args: &[String]) -> &[String] {
    let at = args
        .iter()
        .position(|arg| arg_harness(arg).is_some())
        .map_or(1, |at| at + 1);
    args.get(at..).unwrap_or_default()
}

fn interactive(harness: Harness, args: &[String]) -> bool {
    match harness {
        Harness::Claude => !args
            .iter()
            .any(|arg| CLAUDE_BATCH.contains(&arg.as_str()) || arg.starts_with("--print=")),
        Harness::Codex => args
            .iter()
            .find(|arg| !arg.starts_with('-') || CODEX_BATCH.contains(&arg.as_str()))
            .is_none_or(|first| !CODEX_BATCH.contains(&first.as_str())),
        Harness::Grok => false,
    }
}

fn resume_id(harness: Harness, args: &[String]) -> Option<String> {
    let value_after = |flags: &[&str]| {
        args.iter().enumerate().find_map(|(at, arg)| {
            if let Some((flag, value)) = arg.split_once('=') {
                if flags.contains(&flag) {
                    return Some(value.to_owned());
                }
            }
            if flags.contains(&arg.as_str()) {
                return args
                    .get(at + 1)
                    .filter(|next| !next.starts_with('-'))
                    .cloned();
            }
            None
        })
    };
    let id = match harness {
        Harness::Claude => value_after(&["--resume", "-r", "--session-id"]),
        Harness::Codex => value_after(&["resume"]),
        Harness::Grok => None,
    };
    id.filter(|id| valid_id(id))
}

const HOSTS: [(&[&str], &str); 24] = [
    (&["windowsterminal"], "Windows Terminal"),
    (&["code", "code - insiders"], "VS Code"),
    (&["cursor"], "Cursor"),
    (&["windsurf"], "Windsurf"),
    (&["zed"], "Zed"),
    (&["webstorm"], "WebStorm"),
    (&["idea"], "IntelliJ IDEA"),
    (&["pycharm"], "PyCharm"),
    (&["goland"], "GoLand"),
    (&["rider"], "Rider"),
    (&["clion"], "CLion"),
    (&["rustrover"], "RustRover"),
    (&["phpstorm"], "PhpStorm"),
    (&["alacritty"], "Alacritty"),
    (&["wezterm-gui", "wezterm"], "WezTerm"),
    (&["kitty"], "kitty"),
    (&["gnome-terminal-server"], "GNOME Terminal"),
    (&["konsole"], "Konsole"),
    (&["iterm2"], "iTerm2"),
    (&["terminal"], "Terminal"),
    (&["warp"], "Warp"),
    (&["tmux", "tmux: server"], "tmux"),
    (&["berth-desktop"], "Berth"),
    (&["veyra-desktop"], "Veyra"),
];

fn host_label(name: &str) -> Option<&'static str> {
    let name = name.trim_end_matches("64");
    HOSTS
        .iter()
        .find(|(names, _)| names.contains(&name))
        .map(|(_, label)| *label)
}

fn shell_label(name: &str) -> Option<&'static str> {
    Some(match name {
        "pwsh" => "PowerShell",
        "powershell" => "Windows PowerShell",
        "cmd" => "Command Prompt",
        "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        _ => return None,
    })
}

fn host_of(system: &System, process: &Process) -> (Option<String>, bool, Vec<u32>) {
    let mut fallback = None;
    let mut chain = vec![process.pid().as_u32()];
    let mut current = process.parent();
    for _ in 0..HOST_DEPTH {
        let Some(parent) = current.and_then(|pid| system.process(pid)) else {
            break;
        };
        let name = stem_of(parent.name());
        if name.starts_with("berth") || name.starts_with("pacer") {
            return (Some("berth".to_owned()), true, chain);
        }
        if let Some(label) = host_label(&name) {
            return (Some(label.to_owned()), false, Vec::new());
        }
        chain.push(parent.pid().as_u32());
        fallback = fallback.or_else(|| shell_label(&name));
        current = parent.parent();
    }
    (fallback.map(str::to_owned), false, Vec::new())
}

fn processes() -> Vec<Proc> {
    let mut system = lock(system());
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let candidates: Vec<Pid> = system
        .processes()
        .iter()
        .filter(|(_, process)| {
            matches!(
                stem_of(process.name()).as_str(),
                "claude" | "codex" | "node" | "bun"
            )
        })
        .map(|(pid, _)| *pid)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&candidates),
        false,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_cwd(UpdateKind::OnlyIfNotSet),
    );
    let harnesses: HashMap<Pid, Harness> = candidates
        .iter()
        .filter_map(|pid| Some((*pid, harness_of(system.process(*pid)?)?)))
        .collect();
    harnesses
        .iter()
        .filter_map(|(pid, harness)| {
            let process = system.process(*pid)?;
            if process.parent().and_then(|parent| harnesses.get(&parent)) == Some(harness) {
                return None;
            }
            let args: Vec<String> = process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let args = harness_args(&args);
            if !interactive(*harness, args) {
                return None;
            }
            let (host, berth, berth_pids) = host_of(&system, process);
            let started = process.start_time().saturating_mul(1000);
            let live = match harness {
                Harness::Claude => claude_live_id(*pid, started),
                Harness::Codex | Harness::Grok => None,
            };
            Some(Proc {
                harness: *harness,
                cwd: process
                    .cwd()
                    .map(|cwd| normalize(&cwd.to_string_lossy()))
                    .filter(|cwd| !cwd.is_empty()),
                resume: live.or_else(|| resume_id(*harness, args)),
                started,
                host,
                berth,
                berth_pids,
            })
        })
        .collect()
}

fn claim(candidates: &[&Entry], procs: &[Proc]) -> HashMap<usize, Claim> {
    let mut claims = HashMap::new();
    let mut pending = Vec::new();
    for proc in procs {
        let exact = proc.resume.as_ref().and_then(|id| {
            candidates
                .iter()
                .position(|entry| entry.harness == proc.harness && &entry.id == id)
        });
        match exact {
            Some(at) => {
                claims.entry(at).or_insert(Claim {
                    host: proc.host.clone(),
                    berth: proc.berth,
                    berth_pids: proc.berth_pids.clone(),
                });
            }
            None if proc.resume.is_none() => pending.push(proc),
            None => {}
        }
    }
    pending.sort_by_key(|proc| proc.cwd.is_none());
    for proc in pending {
        let pick = candidates.iter().enumerate().position(|(at, entry)| {
            !claims.contains_key(&at)
                && entry.harness == proc.harness
                && entry.signature.modified + START_SLACK_MS >= proc.started
                && proc
                    .cwd
                    .as_ref()
                    .is_none_or(|cwd| entry.cwd_key.as_ref() == Some(cwd))
        });
        if let Some(at) = pick {
            claims.insert(
                at,
                Claim {
                    host: proc.host.clone(),
                    berth: proc.berth,
                    berth_pids: proc.berth_pids.clone(),
                },
            );
        }
    }
    claims
}

fn status_of(entry: &Entry, running: bool, now: u64) -> Status {
    if !running {
        return Status::Closed;
    }
    let age = now.saturating_sub(entry.signature.modified);
    if age < FRESH_MS {
        return Status::Running;
    }
    if age > IDLE_MS {
        return Status::Finished;
    }
    match entry.tail.last {
        Last::Running => Status::Running,
        Last::Finished | Last::Unknown => Status::Finished,
    }
}

fn session_of(entry: &Entry, claim: Option<&Claim>, now: u64) -> AgentSession {
    let running = claim.is_some();
    AgentSession {
        harness: entry.harness,
        id: entry.id.clone(),
        cwd: entry.head.cwd.clone().unwrap_or_default(),
        title: title_of(&entry.head, &entry.tail, &entry.id),
        branch: entry
            .tail
            .branch
            .clone()
            .or_else(|| entry.head.branch.clone()),
        updated_at: entry.signature.modified,
        status: status_of(entry, running, now),
        running,
        host: claim.and_then(|claim| claim.host.clone()),
        in_berth: claim.is_some_and(|claim| claim.berth),
        berth_pids: claim
            .map(|claim| claim.berth_pids.clone())
            .unwrap_or_default(),
        messages: entry.tail.messages.unwrap_or(entry.tail.counted),
    }
}

pub(crate) fn collect(since_days: Option<u32>) -> Vec<AgentSession> {
    let now = now_ms();
    let days = u64::from(since_days.unwrap_or(DEFAULT_DAYS).clamp(1, MAX_DAYS));
    let found = scan(now.saturating_sub(days * DAY_MS));
    let procs = processes();
    let mut cache = lock(cache());
    let seen: HashSet<&Path> = found.iter().map(|item| item.path.as_path()).collect();
    for item in &found {
        refresh_entry(&mut cache, item);
    }
    cache.retain(|path, _| seen.contains(path.as_path()) || path.exists());
    let mut candidates: Vec<&Entry> = found
        .iter()
        .filter_map(|item| cache.get(&item.path))
        .filter(|entry| !entry.head.automated && entry.cwd_key.is_some())
        .filter(|entry| entry.harness != Harness::Grok || entry.head.prompt.is_some())
        .collect();
    candidates.sort_by_key(|entry| std::cmp::Reverse(entry.signature.modified));
    let mut ids = HashSet::new();
    candidates.retain(|entry| ids.insert((entry.harness, entry.id.as_str())));
    candidates.truncate(SESSION_LIMIT);
    let claims = claim(&candidates, &procs);
    candidates
        .iter()
        .enumerate()
        .map(|(at, entry)| session_of(entry, claims.get(&at), now))
        .collect()
}

pub(crate) fn transcript_files() -> Vec<(Harness, PathBuf)> {
    scan(0)
        .into_iter()
        .map(|found| (found.harness, found.path))
        .collect()
}

fn locate(harness: Harness, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err(crate::host_text!("badSessionId", id = id));
    }
    let cached = lock(cache())
        .iter()
        .find(|(_, entry)| entry.harness == harness && entry.id == id)
        .map(|(path, _)| path.clone());
    if let Some(path) = cached {
        return Ok(path);
    }
    let missing = || crate::host_text!("conversationNotFound", id = id);
    match harness {
        Harness::Claude => {
            let root = claude_root().ok_or_else(missing)?;
            subdirs(&root)
                .into_iter()
                .map(|(_, dir)| dir.join(format!("{id}.jsonl")))
                .find(|path| path.is_file())
                .ok_or_else(missing)
        }
        Harness::Codex => {
            let root = codex_root().ok_or_else(missing)?;
            let suffix = format!("{id}.jsonl");
            codex_days(&root, i64::MIN)
                .into_iter()
                .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name()
                        .and_then(OsStr::to_str)
                        .is_some_and(|name| name.ends_with(&suffix))
                })
                .ok_or_else(missing)
        }
        Harness::Grok => {
            let root = grok_root().ok_or_else(missing)?;
            subdirs(&root)
                .into_iter()
                .map(|(_, dir)| dir.join(id).join(GROK_TRANSCRIPT))
                .find(|path| path.is_file())
                .ok_or_else(missing)
        }
    }
}

fn read_failed(failed: ReadFailed) -> String {
    crate::host_text!("readFailedPlain", path = failed.path.display())
}

fn read_transcript(
    harness: Harness,
    id: &str,
    tail: Option<u32>,
    reach: Option<&str>,
) -> Result<Vec<TranscriptMessage>, String> {
    read_transcript_file(harness, &locate(harness, id)?, tail, reach).map_err(read_failed)
}

fn read_subagent_transcript(
    harness: Harness,
    id: &str,
    call_id: &str,
    tail: Option<u32>,
) -> Result<Option<Vec<TranscriptMessage>>, String> {
    read_subagent_transcript_file(harness, &locate(harness, id)?, call_id, tail)
        .map_err(read_failed)
}

pub async fn agent_sessions_list(since_days: Option<u32>) -> Result<Vec<AgentSession>, String> {
    spawn_blocking(move || collect(since_days))
        .await
        .map_err(|e| e.to_string())
}

pub async fn agent_session_transcript(
    harness: Harness,
    id: String,
    tail: Option<u32>,
    reach: Option<String>,
) -> Result<Vec<TranscriptMessage>, String> {
    spawn_blocking(move || read_transcript(harness, &id, tail, reach.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn agent_subagent_transcript(
    harness: Harness,
    id: String,
    call_id: String,
    tail: Option<u32>,
) -> Result<Option<Vec<TranscriptMessage>>, String> {
    spawn_blocking(move || read_subagent_transcript(harness, &id, &call_id, tail))
        .await
        .map_err(|e| e.to_string())?
}

pub fn agent_session_watch(
    watch: &AgentSessionWatch,
    since_days: Option<u32>,
    on_event: BoxSink<AgentSessionsChanged>,
) {
    if since_days == Some(0) {
        if let Some(previous) = lock(&watch.0).take() {
            previous.abort();
        }
        return;
    }
    let task = tokio::spawn(async move {
        let mut previous: Option<Vec<AgentSession>> = None;
        loop {
            if let Ok(sessions) = spawn_blocking(move || collect(since_days)).await {
                if previous.as_ref() != Some(&sessions) {
                    let event = AgentSessionsChanged {
                        sessions: sessions.clone(),
                    };
                    if !on_event.send(event) {
                        return;
                    }
                    previous = Some(sessions);
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
    if let Some(previous) = lock(&watch.0).replace(task) {
        previous.abort();
    }
}
