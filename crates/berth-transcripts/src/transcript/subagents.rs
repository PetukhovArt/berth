use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::message::Harness;
use super::text::{clip, owned, tag_value};

const LAUNCH_TOOLS: [&str; 2] = ["Agent", "Task"];
const DESCRIPTION_CHARS: usize = 120;
const WAITING_MARKER: &str = "waiting on its own background work";
const NOTIFICATION: &str = "task-notification";

#[derive(Debug, Serialize)]
pub struct Subagent {
    call_id: String,
    description: String,
    kind: Option<String>,
    model: Option<String>,
    background: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<Report>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    status: String,
    waiting: bool,
    at: Option<String>,
    #[serde(skip)]
    model: Option<String>,
}

pub struct Settled {
    pub call_id: Option<String>,
    pub agent_id: Option<String>,
    pub report: Report,
}

impl Subagent {
    pub fn settle(&mut self, report: Report) {
        if report.model.is_some() {
            self.model.clone_from(&report.model);
        }
        self.report = Some(report);
    }
}

fn flag(value: &Value) -> bool {
    value.as_bool() == Some(true) || value.as_str() == Some("true")
}

pub fn launch(harness: Harness, block: &Value) -> Option<Subagent> {
    if harness != Harness::Claude || !LAUNCH_TOOLS.contains(&block["name"].as_str()?) {
        return None;
    }
    let input = &block["input"];
    let description = owned(&input["description"])
        .or_else(|| owned(&input["prompt"]).map(|prompt| clip(&prompt, DESCRIPTION_CHARS)))?;
    Some(Subagent {
        call_id: owned(&block["id"])?,
        description,
        kind: owned(&input["subagent_type"]),
        model: owned(&input["model"]),
        background: flag(&input["run_in_background"]),
        report: None,
    })
}

fn notification(text: &str, line: &Value) -> Option<Settled> {
    let call_id = tag_value(text, "tool-use-id").map(str::to_owned);
    let agent_id = tag_value(text, "task-id").map(str::to_owned);
    if call_id.is_none() && agent_id.is_none() {
        return None;
    }
    Some(Settled {
        call_id,
        agent_id,
        report: Report {
            status: tag_value(text, "status")?.to_owned(),
            waiting: tag_value(text, "summary")
                .is_some_and(|summary| summary.contains(WAITING_MARKER)),
            at: owned(&line["timestamp"]),
            model: None,
        },
    })
}

fn delivered_notification(line: &Value) -> Option<Settled> {
    match line["type"].as_str()? {
        "user" => {
            let text = line["message"]["content"].as_str()?;
            (line["origin"]["kind"].as_str() == Some(NOTIFICATION)
                || text.starts_with("<task-notification>"))
            .then(|| notification(text, line))
            .flatten()
        }
        "attachment" if line["attachment"]["type"].as_str() == Some("queued_command") => {
            notification(line["attachment"]["prompt"].as_str()?, line)
        }
        _ => None,
    }
}

fn results(line: &Value) -> Vec<Settled> {
    let outcome = &line["toolUseResult"];
    line["message"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"].as_str() == Some("tool_result"))
        .filter_map(|block| {
            let failed = block["is_error"].as_bool() == Some(true);
            let status = owned(&outcome["status"])
                .unwrap_or_else(|| if failed { "failed" } else { "completed" }.to_owned());
            Some(Settled {
                call_id: Some(owned(&block["tool_use_id"])?),
                agent_id: owned(&outcome["agentId"]),
                report: Report {
                    status,
                    waiting: false,
                    at: owned(&line["timestamp"]),
                    model: owned(&outcome["resolvedModel"]),
                },
            })
        })
        .collect()
}

pub fn settled(harness: Harness, line: &Value) -> Vec<Settled> {
    if harness != Harness::Claude {
        return Vec::new();
    }
    if let Some(found) = delivered_notification(line) {
        return vec![found];
    }
    if line["type"].as_str() == Some("user") {
        return results(line);
    }
    Vec::new()
}

fn transcript_dir(parent: &Path) -> PathBuf {
    parent.with_extension("").join("subagents")
}

pub fn transcript_of(parent: &Path, call_id: &str) -> Option<PathBuf> {
    let dir = transcript_dir(parent);
    fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .find_map(|meta| {
            let text = fs::read_to_string(&meta).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            (value["toolUseId"].as_str() == Some(call_id))
                .then(|| meta.with_extension("").with_extension("jsonl"))
        })
        .filter(|path| path.is_file())
}
