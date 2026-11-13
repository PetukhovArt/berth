//! The Windows smoke grid: end-to-end cover for what the Windows port
//! *replaced*, run against the real `berth` binary and a real daemon.
//!
//! The pre-existing grids (`e2e_pty.rs`, `e2e_tui.rs`) are `#![cfg(unix)]` —
//! they assert AF_UNIX sockets, `#!/bin/sh` STUB AGENTs, `chmod` bits and
//! `$SHELL -l -i -c` wrapping, none of which this platform has. They stay the
//! protocol's regression net on a Unix host. This file is the complement: the
//! mechanisms that only exist here, so that the parts of the port with no
//! coverage upstream have some.
//!
//! What is covered: the DAEMON SOCKET over loopback TCP, its bearer token in
//! both verdicts, the ENDPOINT FILE, the PIDFILE LOCK refusing a second
//! daemon, and the daemon outliving the client that spawned it.
//!
//! PTY SESSIONS themselves are covered where they live: the ConPTY child's
//! launch is gated on berth answering the host's `ESC[6n` (see
//! `berth_core::dsr`), and `berth-daemon`'s own PTY tests run the full
//! child lifecycle on Windows.

#![cfg(windows)]

use berth_core::codec::{read_frame, write_frame};
use berth_core::{env, ClientRequest, Entity, EntityId, ServerEvent, PROTOCOL_VERSION};
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;

/// How long a daemon reply may take to arrive.
const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
/// Sleep between polls of the filesystem or a socket.
const POLL_STEP: Duration = Duration::from_millis(50);

struct TestEnv {
    tmp: tempfile::TempDir,
    runtime_dir: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_dir = tmp.path().join("rt");
        Self { tmp, runtime_dir }
    }

    /// The ENDPOINT FILE the Windows transport writes: `<port>\n<token>\n`.
    fn endpoint_file(&self) -> PathBuf {
        self.runtime_dir.join("daemon.endpoint")
    }

    fn pidfile(&self) -> PathBuf {
        self.runtime_dir.join("daemon.pid")
    }

    /// The `berth` binary under test, pointed at this env's runtime and data
    /// dirs so it can never touch the developer's own daemon.
    fn cli(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_berth"));
        cmd.env(env::RUNTIME_DIR, &self.runtime_dir)
            .env(env::DATA_DIR, self.tmp.path().join("data"));
        cmd
    }

    fn spawn_daemon(&self) -> DaemonProc {
        let mut cmd = self.cli();
        cmd.args(["daemon", "--foreground"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        DaemonProc(cmd.spawn().unwrap())
    }

    /// Block until the daemon has published its ENDPOINT FILE, and read it.
    fn await_endpoint(&self) -> (u16, String) {
        let deadline = std::time::Instant::now() + EVENT_TIMEOUT;
        loop {
            if let Some(endpoint) = std::fs::read_to_string(self.endpoint_file())
                .ok()
                .and_then(|text| parse_endpoint(&text))
            {
                return endpoint;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the daemon never wrote {}",
                self.endpoint_file().display()
            );
            std::thread::sleep(POLL_STEP);
        }
    }
}

fn parse_endpoint(text: &str) -> Option<(u16, String)> {
    let mut lines = text.lines();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    let token = lines.next()?.trim().to_string();
    (port != 0 && !token.is_empty()).then_some((port, token))
}

/// A daemon spawned for one test, killed when the test's scope ends.
///
/// Without this a test that panics before its closing `Shutdown` leaks a
/// `berth daemon --foreground` that nothing reaps: it is DETACHED_PROCESS,
/// so it outlives the whole `cargo test` run and the next run's daemon then
/// fails to take the PIDFILE LOCK — an error pointing nowhere near the cause.
struct DaemonProc(std::process::Child);

impl Drop for DaemonProc {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        // No SIGTERM here. `Child::kill` is `TerminateProcess`, which skips
        // the daemon's clean shutdown — acceptable only because this is the
        // panic path; the happy path shuts down over the DAEMON SOCKET.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Connect and clear the transport's authorization gate, exactly as
/// `berth_core::transport::connect` does.
async fn connect_with_token(port: u16, token: &str) -> TcpStream {
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => break s,
            Err(e) if tokio::time::Instant::now() < deadline => {
                let _ = e;
                tokio::time::sleep(POLL_STEP).await;
            }
            Err(e) => panic!("daemon never listened on {port}: {e}"),
        }
    };
    write_frame(&mut stream, &token.to_string()).await.unwrap();
    stream
}

/// Send `Hello` and read what comes back.
///
/// `None` covers both ways a hung-up connection can present: a clean EOF at a
/// frame boundary (`Ok(None)`) and an abortive close (`ConnectionReset`),
/// which is what Windows reports when the peer drops a socket with data in
/// flight. Neither is a protocol reply, which is the distinction the callers
/// care about.
async fn hello(stream: &mut TcpStream) -> Option<ServerEvent> {
    if write_frame(
        stream,
        &ClientRequest::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await
    .is_err()
    {
        return None;
    }
    tokio::time::timeout(EVENT_TIMEOUT, read_frame::<ServerEvent, _>(stream))
        .await
        .expect("timed out waiting for the handshake reply")
        .unwrap_or(None)
}

/// The port and the token are the whole of the Windows DAEMON SOCKET: the
/// daemon publishes both in the ENDPOINT FILE, and a client that presents the
/// token gets a handshake.
#[tokio::test]
async fn the_daemon_publishes_an_endpoint_and_serves_a_client_that_presents_its_token() {
    let envd = TestEnv::new();
    let mut daemon = envd.spawn_daemon();
    let (port, token) = envd.await_endpoint();

    assert_eq!(token.len(), 64, "a 32-byte token, hex: {token:?}");
    assert!(
        token.bytes().all(|b| b.is_ascii_hexdigit()),
        "token is hex: {token:?}"
    );

    let mut stream = connect_with_token(port, &token).await;
    match hello(&mut stream).await {
        Some(ServerEvent::HelloOk {
            protocol_version, ..
        }) => assert_eq!(protocol_version, PROTOCOL_VERSION),
        other => panic!("bad handshake reply: {other:?}"),
    }

    write_frame(&mut stream, &ClientRequest::Shutdown)
        .await
        .unwrap();
    let _ = daemon.0.wait();
}

/// The other verdict, and the one that matters: loopback TCP is reachable by
/// anything on the machine, so a connection that cannot produce the token
/// must never reach the protocol. It is refused *before* `Hello`, so a wrong
/// token and a VERSION SKEW stay distinguishable.
#[tokio::test]
async fn a_client_without_the_token_never_reaches_the_protocol() {
    let envd = TestEnv::new();
    let mut daemon = envd.spawn_daemon();
    let (port, token) = envd.await_endpoint();

    let mut stream = connect_with_token(port, "not-the-token").await;
    assert!(
        hello(&mut stream).await.is_none(),
        "a bad token must be hung up on, not answered"
    );

    // And the daemon is unharmed: the right token still works afterwards.
    let mut good = connect_with_token(port, &token).await;
    assert!(
        matches!(hello(&mut good).await, Some(ServerEvent::HelloOk { .. })),
        "one refused client must not take the daemon down with it"
    );

    write_frame(&mut good, &ClientRequest::Shutdown)
        .await
        .unwrap();
    let _ = daemon.0.wait();
}

/// The PIDFILE LOCK is what keeps two daemons off one RUNTIME DIR. On
/// Windows it is `LockFileEx`, and this is the test that it actually refuses
/// — a lock that silently succeeded twice would give two daemons fighting
/// over one ENDPOINT FILE and one database.
#[tokio::test]
async fn a_second_daemon_is_refused_while_the_first_holds_the_pidfile_lock() {
    let envd = TestEnv::new();
    let mut first = envd.spawn_daemon();
    let (port, token) = envd.await_endpoint();

    let second = envd
        .cli()
        .args(["daemon", "--foreground"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("the second daemon runs");
    assert!(
        !second.success(),
        "a second daemon must refuse to start, not share the runtime dir"
    );

    // The pidfile names the daemon that holds the lock — the first one.
    let recorded: u32 = std::fs::read_to_string(envd.pidfile())
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(recorded, first.0.id(), "the pidfile names the lock holder");

    // And the first is still serving, not merely still running.
    let mut stream = connect_with_token(port, &token).await;
    assert!(matches!(
        hello(&mut stream).await,
        Some(ServerEvent::HelloOk { .. })
    ));

    write_frame(&mut stream, &ClientRequest::Shutdown)
        .await
        .unwrap();
    let _ = first.0.wait();
}

/// DAEMON SETSID, in its Windows spelling: a daemon auto-spawned by a client
/// gets DETACHED_PROCESS, so it must outlive the client that started it. If
/// it did not, every one-shot CLI would take the user's sessions down on exit.
#[tokio::test]
async fn the_auto_spawned_daemon_outlives_the_client_that_started_it() {
    let envd = TestEnv::new();

    // `berth add` on a real directory: a one-shot client that spawns the
    // daemon, talks to it, and exits. It has to be a directory that exists —
    // `add` canonicalizes before it connects, so a missing path fails
    // locally and never spawns a daemon at all. Whether the daemon *accepts*
    // the project is beside the point; only that it is still there after the
    // client is gone.
    let project = envd.tmp.path().join("some-dir");
    std::fs::create_dir_all(&project).unwrap();
    let _ = envd
        .cli()
        .args(["add", "--"])
        .arg(&project)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("the client runs");

    let (port, token) = envd.await_endpoint();
    let mut stream = connect_with_token(port, &token).await;
    assert!(
        matches!(hello(&mut stream).await, Some(ServerEvent::HelloOk { .. })),
        "the daemon died with its client — DETACHED_PROCESS is not taking effect"
    );

    write_frame(&mut stream, &ClientRequest::Shutdown)
        .await
        .unwrap();

    // The ENDPOINT FILE goes with it, so the next client does not chase a
    // port nothing is listening on.
    let deadline = std::time::Instant::now() + EVENT_TIMEOUT;
    while envd.endpoint_file().exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the endpoint file outlived the daemon"
        );
        std::thread::sleep(POLL_STEP);
    }
}

async fn read_until(
    stream: &mut TcpStream,
    mut pred: impl FnMut(&ServerEvent) -> bool,
) -> Vec<ServerEvent> {
    let mut seen = Vec::new();
    tokio::time::timeout(EVENT_TIMEOUT, async {
        while let Some(event) = read_frame::<ServerEvent, _>(stream).await.unwrap() {
            let done = pred(&event);
            seen.push(event);
            if done {
                return;
            }
        }
        panic!("the daemon hung up; saw {seen:#?}");
    })
    .await
    .expect("timed out waiting for the daemon");
    seen
}

#[tokio::test]
async fn a_bare_shutdown_from_a_skewed_client_keeps_the_sessions_for_the_next_daemon() {
    let envd = TestEnv::new();
    let repo = envd.tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());

    let mut daemon = envd.spawn_daemon();
    let (port, token) = envd.await_endpoint();
    let mut c = connect_with_token(port, &token).await;
    assert!(matches!(
        hello(&mut c).await,
        Some(ServerEvent::HelloOk { .. })
    ));
    write_frame(&mut c, &ClientRequest::Subscribe)
        .await
        .unwrap();
    read_until(&mut c, |e| matches!(e, ServerEvent::Snapshot { .. })).await;
    write_frame(
        &mut c,
        &ClientRequest::AddProject {
            req_id: 1,
            path: repo.clone(),
            name: None,
            create_missing: false,
        },
    )
    .await
    .unwrap();
    let worktree = read_until(
        &mut c,
        |e| matches!(e, ServerEvent::EntityUpserted { entity: Entity::Worktree(w) } if w.is_main),
    )
    .await
    .into_iter()
    .find_map(|e| match e {
        ServerEvent::EntityUpserted {
            entity: Entity::Worktree(w),
        } if w.is_main => Some(w),
        _ => None,
    })
    .unwrap();
    write_frame(
        &mut c,
        &ClientRequest::CreateTerminal {
            req_id: 2,
            worktree: worktree.id.clone(),
            name: None,
        },
    )
    .await
    .unwrap();
    let events = read_until(&mut c, |e| {
        matches!(
            e,
            ServerEvent::Ack { req_id: 2, .. }
                | ServerEvent::Error {
                    req_id: Some(2),
                    ..
                }
        )
    })
    .await;
    let Some(ServerEvent::Ack {
        created: Some(EntityId::Terminal(terminal)),
        ..
    }) = events.last()
    else {
        panic!("CreateTerminal failed: {events:#?}");
    };
    let terminal = terminal.clone();

    let mut skewed = connect_with_token(port, &token).await;
    write_frame(&mut skewed, &ClientRequest::Shutdown)
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;
    let exited = loop {
        if let Some(status) = daemon.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "a bare Shutdown did not stop the daemon"
        );
        tokio::time::sleep(POLL_STEP).await;
    };
    assert!(
        exited.success(),
        "the daemon did not stop cleanly: {exited}"
    );
    let recorded =
        std::fs::read_to_string(envd.tmp.path().join("data").join("live-sessions.json")).unwrap();
    assert!(
        recorded.contains(terminal.as_str()),
        "the live terminal was not recorded: {recorded}"
    );

    let daemon = envd.spawn_daemon();
    let (port, token) = envd.await_endpoint();
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;
    loop {
        let mut c = connect_with_token(port, &token).await;
        assert!(matches!(
            hello(&mut c).await,
            Some(ServerEvent::HelloOk { .. })
        ));
        write_frame(&mut c, &ClientRequest::Subscribe)
            .await
            .unwrap();
        let snapshot = read_until(&mut c, |e| matches!(e, ServerEvent::Snapshot { .. })).await;
        let alive = snapshot.iter().any(|e| match e {
            ServerEvent::Snapshot { terminals, .. } => {
                terminals.iter().any(|t| t.id == terminal && t.alive)
            }
            _ => false,
        });
        if alive {
            write_frame(&mut c, &ClientRequest::Shutdown).await.unwrap();
            let mut daemon = daemon;
            let _ = tokio::task::spawn_blocking(move || daemon.0.wait()).await;
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the next daemon did not bring the terminal back"
        );
        tokio::time::sleep(POLL_STEP * 4).await;
    }
}
