use berth_core::acp::{BUILT_IN_PROVIDERS, NATIVE_OFF_FOR_NEW_INSTALLS};
use berth_core::AgentKind;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::settings::{berth_config_object, berth_config_write};

const PROVIDERS_KEY: &str = "providers";
const NATIVE: [(AgentKind, &str); 5] = [
    (AgentKind::Claude, "Claude"),
    (AgentKind::Codex, "Codex"),
    (AgentKind::Grok, "Grok"),
    (AgentKind::Cursor, "Cursor"),
    (AgentKind::OpenCode, "OpenCode"),
];

const LOG_BYTES_SHOWN: u64 = 64 * 1024;

#[derive(Debug, Serialize)]
pub struct ProviderRow {
    id: String,
    name: String,
    acp: bool,
    enabled: bool,
    command: Option<String>,
    program: Option<String>,
    args: Option<Vec<String>>,
    built_in: bool,
    found: Option<bool>,
}

fn providers_object() -> Result<Map<String, Value>, String> {
    let config = berth_config_object(&berth_core::paths::config_path())?;
    Ok(match config.get(PROVIDERS_KEY) {
        Some(Value::Object(providers)) => providers.clone(),
        _ => Map::new(),
    })
}

fn enabled_in(entry: Option<&Value>) -> Option<bool> {
    entry?.get("enabled")?.as_bool()
}

fn command_line(command: &str, args: &[String]) -> String {
    std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn strings(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|arg| arg.as_str().map(str::to_string))
        .collect()
}

fn acp_row(id: &str, entry: Option<&Value>) -> Option<ProviderRow> {
    let built_in = BUILT_IN_PROVIDERS.iter().find(|b| b.id == id);
    let text = |key: &str| entry.and_then(|e| e.get(key)).and_then(Value::as_str);
    let command = text("command")
        .map(str::to_string)
        .or_else(|| built_in.map(|b| b.command.to_string()))?;
    let args = strings(entry.and_then(|e| e.get("args"))).unwrap_or_else(|| {
        built_in
            .map(|b| b.args.iter().map(|a| a.to_string()).collect())
            .unwrap_or_default()
    });
    let name = text("name")
        .map(str::to_string)
        .or_else(|| built_in.map(|b| b.name.to_string()))
        .unwrap_or_else(|| id.to_string());
    Some(ProviderRow {
        id: id.to_string(),
        name,
        acp: true,
        enabled: enabled_in(entry).unwrap_or(built_in.is_none()),
        found: Some(berth_core::platform::resolve_program(&command).is_some()),
        command: Some(command_line(&command, &args)),
        program: Some(command),
        args: Some(args),
        built_in: built_in.is_some(),
    })
}

pub async fn providers_read() -> Result<Vec<ProviderRow>, String> {
    let providers = providers_object()?;
    let native = NATIVE.iter().map(|(kind, name)| ProviderRow {
        id: kind.as_str().to_string(),
        name: name.to_string(),
        acp: false,
        enabled: enabled_in(providers.get(kind.as_str()))
            .unwrap_or(!NATIVE_OFF_FOR_NEW_INSTALLS.contains(kind)),
        command: None,
        program: None,
        args: None,
        built_in: true,
        found: None,
    });
    let acp_entries = match providers.get("acp") {
        Some(Value::Object(entries)) => entries.clone(),
        _ => Map::new(),
    };
    let custom = acp_entries
        .keys()
        .filter(|id| !BUILT_IN_PROVIDERS.iter().any(|b| b.id == id.as_str()))
        .cloned();
    let acp = BUILT_IN_PROVIDERS
        .iter()
        .map(|b| b.id.to_string())
        .chain(custom)
        .filter_map(|id| acp_row(&id, acp_entries.get(&id)));
    Ok(native.chain(acp).collect())
}

pub async fn providers_write(id: String, acp: bool, enabled: bool) -> Result<(), String> {
    let known_native = NATIVE.iter().any(|(kind, _)| kind.as_str() == id);
    if !acp && !known_native {
        return Err(format!("unknown provider `{id}`"));
    }
    let mut providers = providers_object()?;
    let scope = if acp {
        match providers
            .entry("acp")
            .or_insert_with(|| Value::Object(Map::new()))
        {
            Value::Object(entries) => entries,
            _ => return Err("providers.acp is not an object".into()),
        }
    } else {
        &mut providers
    };
    let entry = scope.entry(id).or_insert_with(|| Value::Object(Map::new()));
    match entry {
        Value::Object(fields) => {
            fields.insert("enabled".into(), Value::Bool(enabled));
        }
        _ => *entry = serde_json::json!({ "enabled": enabled }),
    }
    berth_config_write(PROVIDERS_KEY, Value::Object(providers))
}

fn acp_entries(providers: &mut Map<String, Value>) -> Result<&mut Map<String, Value>, String> {
    match providers
        .entry("acp")
        .or_insert_with(|| Value::Object(Map::new()))
    {
        Value::Object(entries) => Ok(entries),
        _ => Err("providers.acp is not an object".into()),
    }
}

fn valid_acp_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub async fn providers_save_acp(
    id: String,
    name: String,
    command: String,
    args: Vec<String>,
) -> Result<(), String> {
    if !valid_acp_id(&id) {
        return Err(format!(
            "`{id}` is not a valid id: use lowercase letters, digits, - and _"
        ));
    }
    let command = command.trim().to_string();
    if command.is_empty() {
        return Err("the command is empty".into());
    }
    let built_in = BUILT_IN_PROVIDERS.iter().find(|b| b.id == id);
    let mut providers = providers_object()?;
    let entries = acp_entries(&mut providers)?;
    let entry = entries
        .entry(id)
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    let Value::Object(fields) = entry else {
        unreachable!()
    };
    let args: Vec<String> = args
        .into_iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    match built_in {
        Some(b) => {
            fields.remove("name");
            if command == b.command {
                fields.remove("command");
            } else {
                fields.insert("command".into(), Value::String(command));
            }
            if args.iter().map(String::as_str).eq(b.args.iter().copied()) {
                fields.remove("args");
            } else {
                fields.insert("args".into(), serde_json::json!(args));
            }
        }
        None => {
            let name = name.trim();
            if name.is_empty() {
                fields.remove("name");
            } else {
                fields.insert("name".into(), Value::String(name.to_string()));
            }
            fields.insert("command".into(), Value::String(command));
            fields.insert("args".into(), serde_json::json!(args));
        }
    }
    berth_config_write(PROVIDERS_KEY, Value::Object(providers))
}

pub async fn providers_remove_acp(id: String) -> Result<(), String> {
    let built_in = BUILT_IN_PROVIDERS.iter().any(|b| b.id == id);
    let mut providers = providers_object()?;
    let entries = acp_entries(&mut providers)?;
    if built_in {
        if let Some(Value::Object(fields)) = entries.get_mut(&id) {
            fields.retain(|key, _| key == "enabled");
        }
    } else {
        entries.remove(&id);
    }
    berth_config_write(PROVIDERS_KEY, Value::Object(providers))
}

pub async fn acp_agent_log(agent: String) -> Result<String, String> {
    if agent.is_empty() || !agent.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("invalid agent id".into());
    }
    let path = berth_core::paths::acp_log_path(&agent);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e.to_string()),
    };
    let start = bytes.len().saturating_sub(LOG_BYTES_SHOWN as usize);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}
