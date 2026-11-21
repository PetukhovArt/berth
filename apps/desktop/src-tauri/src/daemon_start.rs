use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn start() -> io::Result<u32> {
    let mut command = Command::new(cli());
    command.arg("daemon").stdin(Stdio::null());
    for name in berth_core::env::HOST_AGENT_SESSION_VARS {
        command.env_remove(name);
    }
    crate::platform::detach(&mut command)
}

#[cfg(feature = "desktop")]
fn cli() -> PathBuf {
    crate::release_update::bundled_cli().unwrap_or_else(|| PathBuf::from("berth"))
}

#[cfg(not(feature = "desktop"))]
fn cli() -> PathBuf {
    PathBuf::from("berth")
}
