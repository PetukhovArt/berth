use std::io;
use std::process::{Command, Stdio};

pub fn start() -> io::Result<u32> {
    let mut command = Command::new("berth");
    command.arg("daemon").stdin(Stdio::null());
    for name in berth_core::env::HOST_AGENT_SESSION_VARS {
        command.env_remove(name);
    }
    crate::platform::detach(&mut command)
}
