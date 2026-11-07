//! The OS-free half of [`super::login_shell_path`]: the probe a login shell
//! runs, reading its answer out of whatever the rc files print around it,
//! and merging that with the `PATH` this process inherited.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const START: &str = "__BERTH_LOGIN_PATH_START__";
const END: &str = "__BERTH_LOGIN_PATH_END__";

/// Long enough for an rc file that loads nvm and a prompt framework, short
/// enough that a hung one only delays the first window by this much.
#[cfg_attr(not(unix), allow(dead_code))]
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// The `-c` command line: the markers let rc-file noise (banners, `echo`,
/// a prompt drawn to stdout) sit on either side of the answer.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn probe_script() -> String {
    format!("printf '{START}%s{END}' \"$PATH\"")
}

/// The text between the first start marker and the end marker after it.
pub fn extract(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    let (_, rest) = text.split_once(START)?;
    let (path, _) = rest.split_once(END)?;
    (!path.trim().is_empty()).then(|| path.to_owned())
}

/// `login` first, in its order, then whatever `inherited` adds; each
/// directory once, empty entries dropped.
pub fn merge(login: &OsStr, inherited: Option<&OsStr>) -> Option<OsString> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let entries =
        std::env::split_paths(login).chain(inherited.into_iter().flat_map(std::env::split_paths));
    for dir in entries {
        if !dir.as_os_str().is_empty() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).ok()
}

/// Run `command` with no stdin and stderr discarded, and return the marked
/// answer once it appears on stdout. `None` when the command cannot start,
/// exits without one, or `timeout` passes first; the command is killed
/// either way, so a hanging rc file cannot outlive the probe.
pub fn capture(mut command: Command, timeout: Duration) -> Option<String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut seen = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    seen.extend_from_slice(&chunk[..n]);
                    if let Some(found) = extract(&seen) {
                        let _ = tx.send(Some(found));
                        return;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = tx.send(extract(&seen));
    });
    let found = rx.recv_timeout(timeout).ok().flatten();
    let _ = child.kill();
    let _ = child.wait();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn marked(path: &str) -> String {
        format!("{START}{path}{END}")
    }

    #[test]
    fn the_answer_is_read_out_of_rc_file_noise() {
        let output = format!(
            "Last login: Mon\n\x1b[32mwelcome\x1b[0m\n{}\nnvm: using node 22\n",
            marked("/opt/homebrew/bin:/usr/bin")
        );
        assert_eq!(
            extract(output.as_bytes()).as_deref(),
            Some("/opt/homebrew/bin:/usr/bin")
        );
    }

    #[test]
    fn no_answer_without_both_markers_or_with_an_empty_one() {
        assert_eq!(extract(b"just a banner\n"), None);
        assert_eq!(extract(format!("{START}/usr/bin").as_bytes()), None);
        assert_eq!(extract(format!("/usr/bin{END}").as_bytes()), None);
        assert_eq!(extract(marked("").as_bytes()), None);
    }

    #[test]
    fn merge_keeps_the_login_order_and_drops_duplicates() {
        let join = |dirs: &[&str]| std::env::join_paths(dirs).unwrap();
        let login = join(&["/opt/homebrew/bin", "/usr/bin", "/opt/homebrew/bin", ""]);
        let inherited = join(&["/usr/bin", "/bin", "/usr/sbin"]);
        assert_eq!(
            merge(&login, Some(&inherited)),
            Some(join(&[
                "/opt/homebrew/bin",
                "/usr/bin",
                "/bin",
                "/usr/sbin"
            ]))
        );
        assert_eq!(merge(&join(&["/a", "/a"]), None), Some(join(&["/a"])));
    }

    fn shell(script: &str) -> Command {
        if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
            command
        } else {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        }
    }

    #[test]
    fn a_hanging_command_is_cut_off_at_the_timeout() {
        let script = if cfg!(windows) {
            "Start-Sleep -Seconds 30"
        } else {
            "sleep 30"
        };
        let started = Instant::now();
        assert_eq!(capture(shell(script), Duration::from_millis(1500)), None);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn the_answer_is_taken_without_waiting_for_the_command_to_exit() {
        let script = if cfg!(windows) {
            format!(
                "[Console]::Out.Write('noise{}'); [Console]::Out.Flush(); Start-Sleep -Seconds 30",
                marked("/usr/bin")
            )
        } else {
            format!("echo noise; printf '%s' '{}'; sleep 30", marked("/usr/bin"))
        };
        let started = Instant::now();
        assert_eq!(
            capture(shell(&script), Duration::from_secs(20)).as_deref(),
            Some("/usr/bin")
        );
        assert!(started.elapsed() < Duration::from_secs(15));
    }

    #[test]
    fn a_command_that_cannot_start_answers_nothing() {
        let command = Command::new("berth-no-such-shell-for-the-probe");
        assert_eq!(capture(command, Duration::from_secs(1)), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_probe_script_prints_the_shells_path_between_the_markers() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", &format!("echo noise; {}", probe_script())])
            .env("PATH", "/usr/bin:/bin:/opt/berth probe/bin");
        assert_eq!(
            capture(command, PROBE_TIMEOUT).as_deref(),
            Some("/usr/bin:/bin:/opt/berth probe/bin")
        );
    }
}
