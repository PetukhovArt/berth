use std::io::ErrorKind;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::hooks::SHARED_HOOK_TAG;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct TrackedKey {
    pub file: String,
    pub field: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

pub(super) struct WantedKey {
    field: String,
    key: String,
    value: Option<Value>,
}

impl WantedKey {
    pub fn key(field: &str, key: &str, value: Value) -> Self {
        Self {
            field: field.into(),
            key: key.into(),
            value: Some(value),
        }
    }

    pub fn member(field: &str, item: &str) -> Self {
        Self {
            field: field.into(),
            key: item.into(),
            value: None,
        }
    }
}

#[derive(Default)]
pub(super) struct KeysOutcome {
    pub tracked: Vec<TrackedKey>,
    pub kept: Vec<String>,
}

pub(super) fn read_json(path: &Path) -> Result<Option<Value>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Some(json!({})));
    }
    let root: Value = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} is not valid JSON; berth left this checkout alone",
            path.display()
        )
    })?;
    if !root.is_object() {
        bail!(
            "{} is not a JSON object; berth left this checkout alone",
            path.display()
        );
    }
    Ok(Some(root))
}

pub(super) fn write_json(path: &Path, root: &Value) -> Result<()> {
    let dir = path.parent().context("a settings file has a parent")?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.berth-tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(root)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("rename into {}", path.display()))
}

pub(super) fn is_bare(root: &Value) -> bool {
    root.as_object().is_some_and(|object| {
        object.iter().all(|(key, value)| {
            key == "version"
                || value.as_object().is_some_and(Map::is_empty)
                || value.as_array().is_some_and(Vec::is_empty)
        })
    })
}

fn not_an_object(path: &Path, what: &str) -> String {
    format!(
        "{what} in {} is not what berth expects; berth left this checkout alone",
        path.display()
    )
}

pub(super) fn merge_tagged_hooks(
    root: &mut Value,
    path: &Path,
    entries: &[(String, Value)],
    versioned: bool,
) -> Result<()> {
    let settings = root
        .as_object_mut()
        .with_context(|| not_an_object(path, "the root"))?;
    if let Some(events) = settings.get_mut("hooks").and_then(Value::as_object_mut) {
        for event_entries in events.values_mut() {
            if let Some(list) = event_entries.as_array_mut() {
                list.retain(|g| g.get(SHARED_HOOK_TAG).is_none());
            }
        }
        events.retain(|_, list| list.as_array().is_none_or(|l| !l.is_empty()));
    }
    if entries.is_empty() {
        return Ok(());
    }
    if versioned {
        settings.entry("version").or_insert(json!(1));
    }
    let events = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| not_an_object(path, "`hooks`"))?;
    for (event, entry) in entries {
        events
            .entry(event.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .with_context(|| not_an_object(path, &format!("`hooks.{event}`")))?
            .push(entry.clone());
    }
    Ok(())
}

pub(super) fn merge_keys(
    root: &mut Value,
    path: &Path,
    rel: &str,
    wanted: &[WantedKey],
    tracked_before: &[&TrackedKey],
) -> Result<KeysOutcome> {
    let settings = root
        .as_object_mut()
        .with_context(|| not_an_object(path, "the root"))?;
    let mut outcome = KeysOutcome::default();
    let was_tracked = |field: &str, key: &str| {
        tracked_before
            .iter()
            .find(|t| t.field == field && t.key == key)
    };

    for old in tracked_before {
        if wanted
            .iter()
            .any(|w| w.field == old.field && w.key == old.key)
        {
            continue;
        }
        let Some(container) = settings.get_mut(&old.field) else {
            continue;
        };
        match (&old.value, container) {
            (Some(value), Value::Object(object)) => {
                if object.get(&old.key) == Some(value) {
                    object.remove(&old.key);
                }
            }
            (None, Value::Array(list)) => list.retain(|item| item.as_str() != Some(&old.key)),
            _ => continue,
        }
        let emptied = settings.get(&old.field).is_some_and(|c| {
            c.as_object().is_some_and(Map::is_empty) || c.as_array().is_some_and(Vec::is_empty)
        });
        if emptied {
            settings.remove(&old.field);
        }
    }

    for want in wanted {
        let tracked = TrackedKey {
            file: rel.to_string(),
            field: want.field.clone(),
            key: want.key.clone(),
            value: want.value.clone(),
        };
        let before = was_tracked(&want.field, &want.key);
        match &want.value {
            Some(value) => {
                let object = settings
                    .entry(want.field.clone())
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .with_context(|| not_an_object(path, &format!("`{}`", want.field)))?;
                match object.get(&want.key) {
                    None => {
                        object.insert(want.key.clone(), value.clone());
                        outcome.tracked.push(tracked);
                    }
                    Some(current) if current == value => {
                        if before.is_some() {
                            outcome.tracked.push(tracked);
                        }
                    }
                    Some(current) if before.and_then(|b| b.value.as_ref()) == Some(current) => {
                        object.insert(want.key.clone(), value.clone());
                        outcome.tracked.push(tracked);
                    }
                    Some(_) => outcome.kept.push(format!("{}.{}", want.field, want.key)),
                }
            }
            None => {
                let list = settings
                    .entry(want.field.clone())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .with_context(|| not_an_object(path, &format!("`{}`", want.field)))?;
                if list.iter().any(|item| item.as_str() == Some(&want.key)) {
                    if before.is_some() {
                        outcome.tracked.push(tracked);
                    }
                } else {
                    list.push(json!(want.key));
                    outcome.tracked.push(tracked);
                }
            }
        }
    }
    Ok(outcome)
}
