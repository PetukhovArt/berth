#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use berth_core::{AgentId, SessionRef};
use berth_daemon::pty::{PtySession, SpawnSpec, DEFAULT_COLS, DEFAULT_ROWS};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const OWNER_MODE: &str = "BERTH_TEST_CONPTY_OWNER";
const OWNER_TEST: &str = "owner_of_a_session";
const READY: &str = "conpty-owner-ready";

#[tokio::test]
#[ignore = "the owner half of console_hosts_die_with_the_process_that_created_them"]
async fn owner_of_a_session() {
    if std::env::var_os(OWNER_MODE).is_none() {
        return;
    }
    let _session = PtySession::spawn(
        SessionRef::Agent(AgentId::generate()),
        SpawnSpec {
            program: "ping.exe".into(),
            args: vec!["-n".into(), "600".into(), "127.0.0.1".into()],
            cwd: std::env::temp_dir(),
            env: vec![],
            scrub_env: &[],
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        },
    )
    .unwrap();
    println!("{READY}");
    tokio::time::sleep(Duration::from_secs(600)).await;
}

fn console_hosts_of(system: &mut System, parent: Pid) -> Vec<Pid> {
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .iter()
        .filter(|(_, p)| {
            p.parent() == Some(parent)
                && ["OpenConsole.exe", "conhost.exe"]
                    .iter()
                    .any(|name| p.name().eq_ignore_ascii_case(name))
        })
        .map(|(pid, _)| *pid)
        .collect()
}

#[test]
fn console_hosts_die_with_the_process_that_created_them() {
    let mut owner = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", OWNER_TEST, "--nocapture"])
        .env(OWNER_MODE, "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(owner.stdout.take().unwrap()).lines();
    if !lines.any(|line| line.is_ok_and(|line| line.contains(READY))) {
        let _ = owner.kill();
        panic!("the owner never opened its session");
    }

    let mut system = System::new();
    let hosts = console_hosts_of(&mut system, Pid::from_u32(owner.id()));
    assert!(!hosts.is_empty(), "the owner's session has no console host");

    owner.kill().unwrap();
    owner.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&hosts),
            true,
            ProcessRefreshKind::nothing(),
        );
        let alive: Vec<_> = hosts
            .iter()
            .filter(|pid| system.process(**pid).is_some())
            .collect();
        if alive.is_empty() {
            break;
        }
        if Instant::now() > deadline {
            for pid in &alive {
                if let Some(host) = system.process(**pid) {
                    host.kill();
                }
            }
            panic!("console hosts {alive:?} outlived the process that created them");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
