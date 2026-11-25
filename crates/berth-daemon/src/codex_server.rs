use berth_core::platform::NoWindow;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, SystemTime};

const OWN_SERVER_FLAG: &str = "--no-daemon";

pub fn own_server_args() -> Vec<String> {
    if codex_knows_own_server_flag() {
        vec![OWN_SERVER_FLAG.to_string()]
    } else {
        Vec::new()
    }
}

fn codex_knows_own_server_flag() -> bool {
    static CACHE: Mutex<Option<(PathBuf, Option<SystemTime>, bool)>> = Mutex::new(None);
    let Some(program) = berth_core::platform::resolve_program("codex") else {
        return false;
    };
    let modified = std::fs::metadata(&program).and_then(|m| m.modified()).ok();
    let mut cache = CACHE.lock().unwrap();
    if let Some((path, at, knows)) = cache.as_ref() {
        if *path == program && *at == modified {
            return *knows;
        }
    }
    let Some(help) = help_text(&program) else {
        return false;
    };
    let knows = help.contains(OWN_SERVER_FLAG);
    *cache = Some((program, modified, knows));
    knows
}

const HELP_TIMEOUT: Duration = Duration::from_secs(5);

fn help_text(program: &Path) -> Option<String> {
    let mut command = std::process::Command::new(program);
    command.arg("--help").no_window();
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sent.send(command.output());
    });
    let output = received.recv_timeout(HELP_TIMEOUT).ok()?.ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}
