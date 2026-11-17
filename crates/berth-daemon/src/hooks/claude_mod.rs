use anyhow::{Context, Result};
use berth_core::platform::NoWindow;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const FILES: &[(&str, &str)] = &[
    (
        ".claude-plugin/plugin.json",
        r#"{ "name": "berth", "version": "1.0.0", "description": "Reports this session's status to berth." }
"#,
    ),
    ("hooks/hooks.json", "{ \"modules\": [\"./register.ts\"] }\n"),
    (
        "hooks/register.ts",
        include_str!("claude-mod-register.ts.txt"),
    ),
];

pub fn plugin_dir() -> PathBuf {
    berth_core::paths::data_dir().join("claude-mod")
}

pub fn install_mod(dir: &Path) -> Result<()> {
    for (rel, source) in FILES {
        let path = dir.join(rel);
        if std::fs::read_to_string(&path).is_ok_and(|text| text == *source) {
            continue;
        }
        let parent = path.parent().unwrap_or(dir);
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        let tmp = path.with_extension("berth-tmp");
        std::fs::write(&tmp, source).with_context(|| format!("write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("rename into {}", path.display()))?;
    }
    Ok(())
}

const FIRST_VERSION_WITH_MODS: (u32, u32, u32) = (2, 1, 287);

const MANAGED_SETTINGS: &[&str] = &[
    "C:\\Program Files\\ClaudeCode\\managed-settings.json",
    "C:\\ProgramData\\ClaudeCode\\managed-settings.json",
    "/Library/Application Support/ClaudeCode/managed-settings.json",
    "/etc/claude-code/managed-settings.json",
];

pub fn claude_loads_mods() -> bool {
    static CACHE: Mutex<Option<(PathBuf, Option<SystemTime>, bool)>> = Mutex::new(None);
    let Some(program) = berth_core::platform::resolve_program("claude") else {
        return false;
    };
    let modified = std::fs::metadata(&program).and_then(|m| m.modified()).ok();
    let mut cache = CACHE.lock().unwrap();
    if let Some((path, at, loads)) = cache.as_ref() {
        if *path == program && *at == modified {
            return *loads;
        }
    }
    let loads = version_of(&program).is_some_and(|v| v >= FIRST_VERSION_WITH_MODS)
        && !mods_restricted_by_policy();
    *cache = Some((program, modified, loads));
    loads
}

fn version_of(program: &Path) -> Option<(u32, u32, u32)> {
    let output = std::process::Command::new(program)
        .arg("--version")
        .no_window()
        .output()
        .ok()?;
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let word = text.split_whitespace().next()?;
    let mut parts = word.split('.').map(|p| p.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

fn mods_restricted_by_policy() -> bool {
    MANAGED_SETTINGS.iter().any(|path| {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|v| v["allowManagedModsOnly"] == serde_json::Value::Bool(true))
    })
}

pub fn with_plugin_dir(dir: &Path, args: Vec<String>) -> Vec<String> {
    let mut out = vec!["--plugin-dir".to_string(), dir.display().to_string()];
    out.extend(args);
    out
}
