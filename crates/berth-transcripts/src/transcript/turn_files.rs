use serde::Serialize;
use serde_json::Value;

use super::message::Harness;

const PATCH_START: &str = "*** Begin Patch";
const PATCH_FILE_MARKERS: [&str; 3] = ["*** Update File: ", "*** Add File: ", "*** Delete File: "];
const PATCH_MOVE: &str = "*** Move to: ";
const SYNTHETIC_MODEL: &str = "<synthetic>";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileEdit {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
}

fn lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

fn count(old: &str, new: &str) -> (u32, u32) {
    let old = lines(old);
    let new = lines(new);
    let head = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let added = new.len() - head - tail;
    let removed = old.len() - head - tail;
    (
        u32::try_from(added).unwrap_or(u32::MAX),
        u32::try_from(removed).unwrap_or(u32::MAX),
    )
}

fn parsed(input: &Value) -> Value {
    match input {
        Value::String(raw) => serde_json::from_str(raw).unwrap_or_else(|_| input.clone()),
        other => other.clone(),
    }
}

fn path_of(input: &Value) -> Option<String> {
    ["file_path", "notebook_path", "path"]
        .iter()
        .find_map(|key| input[key].as_str())
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn patch_edits(patch: &str) -> Vec<FileEdit> {
    let mut out: Vec<FileEdit> = Vec::new();
    for line in patch.lines() {
        if let Some(path) = PATCH_FILE_MARKERS
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
        {
            out.push(FileEdit {
                path: path.trim().to_owned(),
                additions: 0,
                deletions: 0,
            });
        } else if let Some(path) = line.strip_prefix(PATCH_MOVE) {
            if let Some(last) = out.last_mut() {
                last.path = path.trim().to_owned();
            }
        } else if let Some(last) = out.last_mut() {
            if line.starts_with('+') {
                last.additions += 1;
            } else if line.starts_with('-') {
                last.deletions += 1;
            }
        }
    }
    out
}

fn patch_in(input: &Value) -> Option<String> {
    match input {
        Value::String(text) if text.contains(PATCH_START) => Some(text.clone()),
        Value::Array(items) => items.iter().find_map(patch_in),
        Value::Object(map) => map.values().find_map(patch_in),
        _ => None,
    }
}

pub fn file_edits(name: Option<&str>, input: &Value) -> Vec<FileEdit> {
    let input = parsed(input);
    if let Some(patch) = patch_in(&input) {
        return patch_edits(&patch);
    }
    let Some(path) = path_of(&input) else {
        return Vec::new();
    };
    let (additions, deletions) = match name.unwrap_or_default() {
        "Edit" => count(str_of(&input, "old_string"), str_of(&input, "new_string")),
        "MultiEdit" => input["edits"]
            .as_array()
            .map(|edits| {
                edits.iter().fold((0, 0), |(added, removed), edit| {
                    let (a, r) = count(str_of(edit, "old_string"), str_of(edit, "new_string"));
                    (added + a, removed + r)
                })
            })
            .unwrap_or_default(),
        "Write" => count("", str_of(&input, "content")),
        "NotebookEdit" => count("", str_of(&input, "new_source")),
        _ => return Vec::new(),
    };
    vec![FileEdit {
        path,
        additions,
        deletions,
    }]
}

pub fn line_model(harness: Harness, line: &Value) -> Option<String> {
    let model = match harness {
        Harness::Claude if line["type"].as_str() == Some("assistant") => &line["message"]["model"],
        Harness::Codex if line["type"].as_str() == Some("turn_context") => {
            &line["payload"]["model"]
        }
        _ => return None,
    };
    model
        .as_str()
        .map(str::trim)
        .filter(|model| !model.is_empty() && *model != SYNTHETIC_MODEL)
        .map(str::to_owned)
}
