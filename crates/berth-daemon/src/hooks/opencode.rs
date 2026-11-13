use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const PLUGIN_FILE: &str = "berth.js";
const PLUGIN_SOURCE: &str = include_str!("opencode-plugin.mjs.txt");

pub fn plugin_dir() -> PathBuf {
    berth_core::env::non_empty("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            berth_core::env::home_dir()
                .unwrap_or_default()
                .join(".config")
        })
        .join("opencode")
        .join("plugin")
}

pub fn install_plugin(dir: &Path) -> Result<()> {
    let path = dir.join(PLUGIN_FILE);
    if std::fs::read_to_string(&path).is_ok_and(|text| text == PLUGIN_SOURCE) {
        return Ok(());
    }
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let tmp = dir.join(format!(".{PLUGIN_FILE}.berth-tmp"));
    std::fs::write(&tmp, PLUGIN_SOURCE).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("rename into {}", path.display()))?;
    Ok(())
}
