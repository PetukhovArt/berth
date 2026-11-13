use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

const HTTPS_PORT: &str = "--https=443";
const TIMEOUT: Duration = Duration::from_secs(10);

fn serve_on_args(port: u16) -> Vec<String> {
    vec![
        "serve".to_owned(),
        "--bg".to_owned(),
        HTTPS_PORT.to_owned(),
        format!("http://127.0.0.1:{port}"),
    ]
}

fn serve_off_args() -> Vec<String> {
    vec!["serve".to_owned(), HTTPS_PORT.to_owned(), "off".to_owned()]
}

fn dns_name(status_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(status_json).ok()?;
    let name = value.get("Self")?.get("DNSName")?.as_str()?;
    let name = name.trim_end_matches('.');
    (!name.is_empty()).then(|| name.to_owned())
}

fn public_url(dns_name: &str) -> String {
    format!("https://{dns_name}/")
}

fn summary(stdout: &[u8], stderr: &[u8]) -> String {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(stderr),
        String::from_utf8_lossy(stdout)
    );
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn run(args: &[String]) -> Result<String, String> {
    let mut command = Command::new("tailscale");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let label = format!("tailscale {}", args.join(" "));
    let output = tokio::time::timeout(TIMEOUT, async { command.spawn()?.wait_with_output().await })
        .await
        .map_err(|_| format!("{label}: no answer in {} s", TIMEOUT.as_secs()))?
        .map_err(|error| format!("{label}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "{label}: {}",
            summary(&output.stdout, &output.stderr)
        ))
    }
}

pub async fn serve(port: u16) -> Result<String, String> {
    let status = run(&["status".to_owned(), "--json".to_owned()]).await?;
    let name = dns_name(&status).ok_or("tailscale status has no DNS name for this machine")?;
    run(&serve_on_args(port)).await?;
    Ok(public_url(&name))
}

pub fn unserve() {
    let mut command = std::process::Command::new("tailscale");
    command
        .args(serve_off_args())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let _ = command.status();
}
