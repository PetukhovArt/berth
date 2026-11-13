use std::io;
use std::process::Stdio;
use std::time::Duration;

use berth_core::env;
use berth_core::transport::{Forward, Stream};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(180);
const FORWARD_READY: Duration = Duration::from_secs(15);
const FORWARD_POLL: Duration = Duration::from_millis(100);
const SSH_NO_WINDOW: u32 = 0x0800_0000;
const REMOTE_START: &str = "PATH=\"$HOME/.local/bin:$PATH\"; command -v berth >/dev/null 2>&1 || curl -fsSL https://raw.githubusercontent.com/PetukhovArt/berth/main/install.sh | sh >&2; berth socket";

struct Tunnel {
    _ssh: Child,
}

static TUNNEL: Mutex<Option<Tunnel>> = Mutex::const_new(None);

pub fn target() -> Option<String> {
    env::non_empty(env::REMOTE)
}

pub async fn connect(target: &str) -> io::Result<Stream> {
    let mut slot = TUNNEL.lock().await;
    *slot = None;
    let socket = remote_socket(target).await?;
    let scratch = std::env::temp_dir().join(format!("berth-ssh-{}", std::process::id()));
    let forward = Forward::new(&scratch)?;
    let mut ssh = ssh();
    ssh.args([
        "-N",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "ServerAliveInterval=15",
    ])
    .arg("-L")
    .arg(format!("{}:{socket}", forward.ssh_spec()))
    .arg(target);
    ssh.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = ssh.spawn().map_err(spawn_error)?;
    let deadline = tokio::time::Instant::now() + FORWARD_READY;
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "ssh forward to {target} ended: {status}"
            )));
        }
        match forward.connect().await {
            Ok(stream) => {
                *slot = Some(Tunnel { _ssh: child });
                return Ok(stream);
            }
            Err(error) if tokio::time::Instant::now() >= deadline => {
                return Err(io::Error::other(format!(
                    "ssh forward to {target} did not come up: {error}"
                )));
            }
            Err(_) => tokio::time::sleep(FORWARD_POLL).await,
        }
    }
}

async fn remote_socket(target: &str) -> io::Result<String> {
    let output = ssh()
        .arg(target)
        .arg(REMOTE_START)
        .output()
        .await
        .map_err(spawn_error)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let socket = stdout.lines().map(str::trim).rfind(|line| !line.is_empty());
    match socket {
        Some(socket) if output.status.success() => Ok(socket.to_string()),
        _ => Err(io::Error::other(format!(
            "ssh {target}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
    }
}

fn ssh() -> Command {
    let mut command = Command::new("ssh");
    command
        .args(["-o", "BatchMode=yes"])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(SSH_NO_WINDOW);
    command
}

fn spawn_error(error: io::Error) -> io::Error {
    io::Error::other(format!("could not run ssh: {error}"))
}
