use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::AppHandle;

use berth_core::platform::NoWindow;

use crate::platform;
use crate::settings;

const ROOT_VAR: &str = "BERTH_APP_ROOT";
const SOURCE_REPO_KEY: &str = "app.sourceRepo";
const BRANCH: &str = "main";
const BUILD_FILE: &str = "build.json";
const LOG_TAIL_LINES: usize = 14;
const LOG_TAIL_BYTES: u64 = 64 * 1024;
const EXIT_DELAY: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    build_id: String,
    commit: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    source_repo: String,
    #[serde(default)]
    built_at: String,
    #[serde(default)]
    protocol: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    commit: String,
    subject: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    root: String,
    running: Option<Build>,
    current: Option<Build>,
    previous: Option<Build>,
    source_repo: Option<String>,
    head: Option<Head>,
    head_error: Option<String>,
    daemon_protocol: Option<u32>,
    daemon_build: Option<Build>,
    job: Option<Value>,
}

fn root() -> Option<PathBuf> {
    if let Some(dir) = crate::legacy::env(ROOT_VAR) {
        return Some(PathBuf::from(dir));
    }
    platform::default_install_root()
}

fn plain(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

fn read_build(dir: &Path) -> Option<Build> {
    let text = fs::read_to_string(dir.join(BUILD_FILE)).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

fn running_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent().map(|dir| plain(platform::install_dir(dir)))
}

fn running_build() -> Option<Build> {
    static RUNNING: OnceLock<Option<Build>> = OnceLock::new();
    RUNNING
        .get_or_init(|| running_dir().and_then(|dir| read_build(&dir)))
        .clone()
}

fn link(name: &str) -> Option<PathBuf> {
    let path = root()?.join("app").join(name);
    fs::canonicalize(&path).ok().map(plain)
}

fn status_path() -> Option<PathBuf> {
    root().map(|root| root.join("build").join("status.json"))
}

fn source_repo() -> Option<String> {
    let value: Value = serde_json::from_str(&settings::settings_text()).unwrap_or(Value::Null);
    value
        .get(SOURCE_REPO_KEY)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|repo| !repo.is_empty())
        .map(str::to_owned)
        .or_else(|| running_build().map(|build| build.source_repo))
        .filter(|repo| !repo.is_empty())
}

fn hidden(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_window();
    command
}

fn output(mut command: Command) -> Result<String, String> {
    let out = command.output().map_err(|error| error.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

fn git(repo: &str, args: &[&str]) -> Result<String, String> {
    let mut command = hidden("git");
    command.arg("-C").arg(repo).args(args);
    output(command)
}

fn head(repo: &str) -> Result<Head, String> {
    let commit = git(
        repo,
        &[
            "rev-parse",
            "--verify",
            &format!("refs/heads/{BRANCH}^{{commit}}"),
        ],
    )?;
    let subject = git(repo, &["log", "-1", "--format=%s", &commit])?;
    Ok(Head { commit, subject })
}

fn alive(pid: u32) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    system.process(pid).is_some()
}

fn read_status() -> Option<Map<String, Value>> {
    let text = fs::read_to_string(status_path()?).ok()?;
    match serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

fn write_status(map: &Map<String, Value>) -> io::Result<()> {
    let path = status_path().ok_or_else(|| io::Error::other("no install root"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(map).unwrap_or_default())?;
    fs::rename(&temp, &path)
}

fn running_pid(map: &Map<String, Value>) -> Option<u32> {
    if map.get("state").and_then(Value::as_str) != Some("running") {
        return None;
    }
    map.get("pid")
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
}

fn tail(path: &Path) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(LOG_TAIL_BYTES)));
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect();
    lines[lines.len().saturating_sub(LOG_TAIL_LINES)..]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

fn job() -> Option<Value> {
    let mut map = read_status()?;
    if let Some(pid) = running_pid(&map) {
        if !alive(pid) {
            map.insert("state".into(), "failed".into());
            map.insert(
                "error".into(),
                "the build process exited unexpectedly".into(),
            );
            let _ = write_status(&map);
        }
    }
    let log = map.get("log").and_then(Value::as_str).map(PathBuf::from);
    if let Some(log) = log {
        let lines = tail(&log).into_iter().map(Value::String).collect();
        map.insert("tail".into(), Value::Array(lines));
    }
    Some(Value::Object(map))
}

fn script() -> Result<PathBuf, String> {
    [running_dir(), link("current")]
        .into_iter()
        .flatten()
        .map(|dir| dir.join(platform::INSTALLER_SCRIPT))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "this copy of Berth was not installed with scripts/{}",
                platform::INSTALLER_SCRIPT
            )
        })
}

fn require_self_update() -> Result<(), String> {
    if platform::SELF_UPDATE {
        Ok(())
    } else {
        Err(platform::UNSUPPORTED.into())
    }
}

pub async fn app_update_info() -> Result<Info, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let repo = source_repo();
        let (head, head_error) = match repo.as_deref().map(head) {
            Some(Ok(head)) => (Some(head), None),
            Some(Err(error)) => (None, Some(error)),
            None => (None, None),
        };
        Info {
            root: root()
                .map(|root| root.display().to_string())
                .unwrap_or_default(),
            running: running_build(),
            current: link("current").and_then(|dir| read_build(&dir)),
            previous: link("previous").and_then(|dir| read_build(&dir)),
            source_repo: repo,
            head,
            head_error,
            daemon_protocol: crate::berth::daemon_protocol(),
            daemon_build: root().and_then(|root| read_build(&root.join("daemon"))),
            job: job(),
        }
    })
    .await
    .map_err(|error| error.to_string())
}

pub async fn app_update_status() -> Option<Value> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .ok()
        .flatten()
}

fn is_commit(text: &str) -> bool {
    (4..=64).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub async fn app_update_changes(from: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let to = running_build().ok_or("this copy of Berth is not a working copy")?;
        if !is_commit(&from) || !is_commit(&to.commit) {
            return Err("not a commit id".into());
        }
        let repo = source_repo().ok_or("no source repository")?;
        let range = format!("{from}..{}", to.commit);
        let log = git(&repo, &["log", "--no-merges", "--format=%s", &range])?;
        Ok(log.lines().map(str::to_owned).collect())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn app_update_start() -> Result<(), String> {
    require_self_update()?;
    let script = script()?;
    let repo = source_repo().ok_or("no source repository: set app.sourceRepo in settings")?;
    let root = root().ok_or(platform::INSTALL_ROOT_MISSING)?;
    if let Some(pid) = read_status().as_ref().and_then(running_pid) {
        if alive(pid) {
            return Err("a build is already running".into());
        }
    }
    let mut command = platform::installer_command(&script)?;
    command.arg("-Repo").arg(&repo).arg("-Root").arg(&root);
    let pid = platform::detach(&mut command).map_err(|error| error.to_string())?;
    let mut map = Map::new();
    map.insert("state".into(), "running".into());
    map.insert("step".into(), "start".into());
    map.insert("pid".into(), pid.into());
    if let Ok(head) = head(&repo) {
        map.insert("commit".into(), head.commit.into());
        map.insert("subject".into(), head.subject.into());
    }
    map.insert("repo".into(), repo.into());
    if read_status().and_then(|map| running_pid(&map)) != Some(pid) {
        let _ = write_status(&map);
    }
    Ok(())
}

pub fn app_update_cancel() -> Result<(), String> {
    let mut map = read_status().ok_or("no build is running")?;
    let pid = running_pid(&map).ok_or("no build is running")?;
    map.insert("state".into(), "cancelled".into());
    write_status(&map).map_err(|error| error.to_string())?;
    let killed = berth_core::platform::kill_process_tree(pid);
    if alive(pid) {
        map.insert("state".into(), "running".into());
        let _ = write_status(&map);
        return Err(killed
            .err()
            .unwrap_or_else(|| "the build is still running".into()));
    }
    Ok(())
}

pub async fn app_rollback() -> Result<(), String> {
    run_script("-Rollback").await
}

pub async fn app_daemon_update() -> Result<(), String> {
    run_script("-UpdateDaemon").await
}

async fn run_script(flag: &'static str) -> Result<(), String> {
    require_self_update()?;
    let script = script()?;
    let root = root().ok_or(platform::INSTALL_ROOT_MISSING)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut command = platform::installer_command(&script)?;
        command.arg(flag).arg("-Root").arg(root);
        output(command).map(drop)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn app_restart(app: &AppHandle) -> Result<(), String> {
    require_self_update()?;
    let exe = root()
        .ok_or(platform::INSTALL_ROOT_MISSING)?
        .join("app")
        .join("current")
        .join(platform::APP_EXECUTABLE);
    if !exe.is_file() {
        return Err(format!("{} is missing", exe.display()));
    }
    let mut command = platform::relaunch_command(&exe, std::process::id())?;
    platform::detach(&mut command).map_err(|error| error.to_string())?;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(EXIT_DELAY).await;
        app.exit(0);
    });
    Ok(())
}
