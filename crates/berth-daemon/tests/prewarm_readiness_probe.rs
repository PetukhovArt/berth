#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use berth_core::{AgentId, SessionRef};
use berth_daemon::pty::{PtySession, SpawnSpec};

const PROMPT: &str = "reply with the single word ok";
const WINDOW: Duration = Duration::from_secs(40);

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

struct Probe {
    cwd: PathBuf,
    log_dir: PathBuf,
}

impl Probe {
    fn new(cwd: PathBuf, name: &str) -> Self {
        let log_dir = std::env::temp_dir()
            .join("berth-readiness-probe")
            .join(name);
        std::fs::create_dir_all(&log_dir).unwrap();
        let log = log_dir.join("hooks.log");
        let log = log.to_string_lossy().replace('\\', "/");
        let hook = |event: &str| {
            serde_json::json!([{ "hooks": [{ "type": "command", "command": format!(
                "{{ printf '%s {event} ' \"$(date +%s%3N)\"; tr -d '\\r\\n' | head -c 2000; echo; }} >> '{log}'"
            )}]}])
        };
        let settings = serde_json::json!({ "hooks": {
            "SessionStart": hook("SessionStart"),
            "UserPromptSubmit": hook("UserPromptSubmit"),
        }});
        std::fs::write(
            log_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        Self { cwd, log_dir }
    }

    fn trusted_repo() -> Self {
        Self::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(".."),
            "trusted",
        )
    }
}

fn hook_lines(dir: &Path) -> Vec<(u128, String, String)> {
    std::fs::read_to_string(dir.join("hooks.log"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(3, ' ');
            Some((
                it.next()?.parse().ok()?,
                it.next()?.to_string(),
                it.next().unwrap_or("").to_string(),
            ))
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum TypeWhen {
    Argv,
    AtSpawn,
    AtSessionStart,
    AtBracketedPaste,
    AfterSpareAge(Duration),
}

struct Run {
    spawn_ms: u128,
    first_output: Option<u128>,
    bracketed_paste: Option<u128>,
    placeholder: Option<u128>,
    session_start: Option<u128>,
    typed: Option<u128>,
    submits: Vec<(u128, String)>,
    trust_dialog: bool,
    output_bytes: usize,
    last_output: Option<u128>,
}

fn rel(t: Option<u128>, base: u128) -> String {
    t.map_or("-".into(), |t| format!("{}", t as i128 - base as i128))
}

impl std::fmt::Display for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = self.spawn_ms;
        let first_submit = self.submits.first().map(|(t, _)| *t);
        write!(
            f,
            "output {}..{} ({} B) | ?2004h {} | placeholder {} | SessionStart {} | typed {} | submit {} (typed->submit {}) | submits with the prompt {}/{} | trust dialog {}",
            rel(self.first_output, b),
            rel(self.last_output, b),
            self.output_bytes,
            rel(self.bracketed_paste, b),
            rel(self.placeholder, b),
            rel(self.session_start, b),
            rel(self.typed, b),
            rel(first_submit, b),
            rel(first_submit, self.typed.unwrap_or(b)),
            self.submits.iter().filter(|(_, p)| p.contains(PROMPT)).count(),
            self.submits.len(),
            self.trust_dialog,
        )
    }
}

async fn drive(probe: &Probe, when: TypeWhen) -> Run {
    let dir = probe.log_dir.as_path();
    let _ = std::fs::remove_file(dir.join("hooks.log"));
    let mut args = vec![
        "--settings".to_string(),
        dir.join("settings.json").to_string_lossy().into_owned(),
    ];
    if let TypeWhen::Argv = when {
        args.push(PROMPT.to_string());
    }
    let (program, args) = berth_daemon::launch::wrap_for_user_env("claude", &args);
    let spawn_ms = now_ms();
    let session: Arc<PtySession> = PtySession::spawn(
        SessionRef::Agent(AgentId("probe".into())),
        SpawnSpec {
            program,
            args,
            cwd: probe.cwd.clone(),
            env: vec![],
            scrub_env: &[],
            cols: 120,
            rows: 40,
        },
    )
    .unwrap();
    let mut run = Run {
        spawn_ms,
        first_output: None,
        bracketed_paste: None,
        placeholder: None,
        session_start: None,
        typed: None,
        submits: Vec::new(),
        trust_dialog: false,
        output_bytes: 0,
        last_output: None,
    };
    let started = Instant::now();
    while started.elapsed() < WINDOW {
        let (_, out) = session.snapshot(None);
        let text = String::from_utf8_lossy(&out);
        let t = now_ms();
        if run.first_output.is_none() && !out.is_empty() {
            run.first_output = Some(t);
        }
        if out.len() != run.output_bytes {
            run.output_bytes = out.len();
            run.last_output = Some(t);
        }
        if run.bracketed_paste.is_none() && text.contains("\x1b[?2004h") {
            run.bracketed_paste = Some(t);
        }
        if run.placeholder.is_none() && text.contains("Try") {
            run.placeholder = Some(t);
        }
        if text.contains("trust") && !run.trust_dialog {
            run.trust_dialog = true;
        }
        let hooks = hook_lines(dir);
        if run.session_start.is_none() {
            run.session_start = hooks
                .iter()
                .find(|(_, e, _)| e == "SessionStart")
                .map(|(t, _, _)| *t);
        }
        if hooks.iter().any(|(_, e, _)| e == "UserPromptSubmit") {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            run.submits = hook_lines(dir)
                .into_iter()
                .filter(|(_, e, _)| e == "UserPromptSubmit")
                .map(|(t, _, body)| (t, body))
                .collect();
            break;
        }
        let due = match when {
            TypeWhen::Argv => false,
            TypeWhen::AtSpawn => true,
            TypeWhen::AtSessionStart => run.session_start.is_some(),
            TypeWhen::AtBracketedPaste => run.bracketed_paste.is_some(),
            TypeWhen::AfterSpareAge(d) => started.elapsed() >= d,
        };
        if due && run.typed.is_none() {
            run.typed = Some(now_ms());
            session
                .write_input(&berth_daemon::triggers::paste(PROMPT))
                .unwrap();
            tokio::time::sleep(berth_daemon::triggers::PASTE_GAP).await;
            session.write_input(b"\r").unwrap();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let _ = std::fs::write(dir.join("last-output.txt"), session.snapshot(None).1);
    session.kill();
    tokio::time::sleep(Duration::from_millis(500)).await;
    run
}

async fn runs(probe: &Probe, moments: &[TypeWhen]) {
    let mut failures = Vec::new();
    for &when in moments {
        let run = drive(probe, when).await;
        eprintln!("[probe] {when:?}: {run}");
        let landed = run
            .submits
            .iter()
            .filter(|(_, b)| b.contains(PROMPT))
            .count();
        if landed != 1 {
            failures.push(format!("{when:?}: landed {landed}x"));
        }
    }
    eprintln!("[probe] failures: {failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn probe_claude_cold_argv_vs_typed_into_a_booted_spare() {
    let spare_age = TypeWhen::AfterSpareAge(Duration::from_secs(12));
    runs(
        &Probe::trusted_repo(),
        &[
            TypeWhen::Argv,
            spare_age,
            TypeWhen::Argv,
            spare_age,
            TypeWhen::Argv,
            spare_age,
        ],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn probe_claude_typed_at_each_boot_signal() {
    runs(
        &Probe::trusted_repo(),
        &[
            TypeWhen::AtSessionStart,
            TypeWhen::AtSessionStart,
            TypeWhen::AtSessionStart,
            TypeWhen::AtBracketedPaste,
            TypeWhen::AtBracketedPaste,
            TypeWhen::AtSpawn,
            TypeWhen::AtSpawn,
        ],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn probe_claude_untrusted_folder_hooks_wait_for_trust() {
    let name = format!("untrusted-{}", now_ms());
    let cwd = std::env::temp_dir()
        .join("berth-readiness-probe")
        .join(&name)
        .join("cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    let run = drive(
        &Probe::new(cwd, &name),
        TypeWhen::AfterSpareAge(Duration::from_secs(15)),
    )
    .await;
    eprintln!("[probe] untrusted: {run}");
}
