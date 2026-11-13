use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::connectors::secrets::{self, Lookup};

const LEGACY_TOKEN_ID: &str = "remote-pairing-token";
const DEVICES_ID: &str = "remote-devices";
const DEFAULT_NAME: &str = "Default";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Full,
    ReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: u64,
    pub name: String,
    pub scope: Scope,
    pub token: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: u64,
    pub name: String,
    pub scope: Scope,
    pub created_at: u64,
}

#[derive(Debug, Serialize)]
pub struct Pairing {
    pub url: String,
    pub qr_svg: Option<String>,
}

impl Device {
    pub fn info(&self) -> DeviceInfo {
        DeviceInfo {
            id: self.id,
            name: self.name.clone(),
            scope: self.scope,
            created_at: self.created_at,
        }
    }
}

pub fn fresh(devices: &[Device], name: &str, scope: Scope, token: String, now: u64) -> Device {
    Device {
        id: devices.iter().map(|device| device.id).max().unwrap_or(0) + 1,
        name: name.trim().to_owned(),
        scope,
        token,
        created_at: now,
    }
}

pub fn default_device(token: String, now: u64) -> Device {
    fresh(&[], DEFAULT_NAME, Scope::Full, token, now)
}

pub fn load(now: u64, mint: impl Fn() -> Result<String, String>) -> Result<Vec<Device>, String> {
    match secrets::lookup(DEVICES_ID) {
        Lookup::Found(text) => {
            let devices: Vec<Device> =
                serde_json::from_str(&text).map_err(|error| error.to_string())?;
            if !devices.is_empty() {
                return Ok(devices);
            }
        }
        Lookup::Missing => {}
        Lookup::Failed(message) => return Err(message),
    }
    let token = match secrets::lookup(LEGACY_TOKEN_ID) {
        Lookup::Found(token) => token,
        Lookup::Missing => mint()?,
        Lookup::Failed(message) => return Err(message),
    };
    let devices = vec![default_device(token, now)];
    save(&devices)?;
    Ok(devices)
}

pub fn save(devices: &[Device]) -> Result<(), String> {
    let text = serde_json::to_string(devices).map_err(|error| error.to_string())?;
    secrets::save(DEVICES_ID, &text)
}

const READ_ONLY_COMMANDS: &[&str] = &[
    "berth_watch",
    "berth_attach",
    "berth_resize",
    "berth_detach",
    "berth_mark_seen",
    "settings_watch",
    "connectors_watch",
    "agent_session_watch",
    "fs_watch",
    "fs_unwatch",
    "connectors_list",
    "connector_asset",
    "forge_inbox",
    "forge_for_worktree",
    "forge_change_request",
    "forge_diff",
    "forge_discussions",
    "forge_pipeline",
    "forge_file",
    "forge_source",
    "project_repo_url",
    "jira_search",
    "jira_issue",
    "confluence_search",
    "confluence_page",
    "platform_info",
    "settings_read",
    "berth_idle_timeout_read",
    "berth_restore_sessions_read",
    "berth_flag_read",
    "agent_sessions_list",
    "agent_session_transcript",
    "review_queue",
    "review_adapter",
    "fs_read_text",
    "fs_read_doc",
    "fs_read_binary",
    "fs_list_markdown",
    "fs_list_dirs",
    "history_projects",
    "worktree_changes",
    "worktree_git_state",
];

const READ_ONLY_REQUESTS: &[&str] = &[
    "Hello",
    "Subscribe",
    "Attach",
    "Detach",
    "Resize",
    "ListBranches",
    "ListOrphanedSessions",
    "MarkPrSeen",
    "MarkAgentSeen",
    "GetMetrics",
    "GetUsageLimits",
    "GetTaskNote",
    "GetProjectSetup",
    "ListTriggers",
];

fn request_kind(request: &Value) -> Option<&str> {
    match request {
        Value::String(kind) => Some(kind),
        Value::Object(map) if map.len() == 1 => map.keys().next().map(String::as_str),
        _ => None,
    }
}

pub fn permits(scope: Scope, cmd: &str, args: &Value) -> bool {
    match scope {
        Scope::Full => true,
        Scope::ReadOnly if cmd == "berth_call" => args
            .get("request")
            .and_then(request_kind)
            .is_some_and(|kind| READ_ONLY_REQUESTS.contains(&kind)),
        Scope::ReadOnly => READ_ONLY_COMMANDS.contains(&cmd),
    }
}
