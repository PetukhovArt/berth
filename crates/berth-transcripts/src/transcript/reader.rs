use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::message::{
    claude_messages, codex_messages, grok_messages, record_of, Harness, Role, TranscriptMessage,
};
use super::turn_files::line_model;
use super::{subagents, tool_exit};

const TRANSCRIPT_BYTES: u64 = 256 * 1024;
const TRANSCRIPT_MAX_BYTES: u64 = 16 * 1024 * 1024;
const TRANSCRIPT_DEFAULT: usize = 200;
const TRANSCRIPT_MAX: usize = 1000;
const REACH_MAX_BYTES: u64 = 64 * 1024 * 1024;
const REACH_CONTEXT: usize = 20;

#[derive(Debug)]
pub struct ReadFailed {
    pub path: PathBuf,
}

pub(super) fn records(buffer: &[u8], base: u64) -> impl Iterator<Item = (u64, Value)> + '_ {
    let mut next = base;
    buffer.split(|byte| *byte == b'\n').filter_map(move |line| {
        let at = next;
        next += line.len() as u64 + 1;
        if line.is_empty() {
            return None;
        }
        serde_json::from_slice::<Value>(line)
            .ok()
            .map(|value| (at, value))
    })
}

pub(super) struct Window {
    pub(super) buffer: Vec<u8>,
    pub(super) base: u64,
    from_start: bool,
}

pub(super) fn read_tail(path: &Path, budget: u64) -> Option<Window> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(budget);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buffer = Vec::new();
    file.take(budget).read_to_end(&mut buffer).ok()?;
    let mut base = start;
    if start > 0 {
        let cut = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |at| at + 1);
        buffer.drain(..cut);
        base += cut as u64;
    }
    Some(Window {
        buffer,
        base,
        from_start: start == 0,
    })
}

fn reach_budget(path: &Path, reach: Option<&str>) -> u64 {
    let len = fs::metadata(path).map_or(0, |meta| meta.len());
    reach
        .and_then(record_of)
        .filter(|record| *record < len)
        .map_or(TRANSCRIPT_BYTES, |record| {
            (len - record + TRANSCRIPT_BYTES).clamp(TRANSCRIPT_BYTES, REACH_MAX_BYTES)
        })
}

fn read_messages(
    harness: Harness,
    path: &Path,
    tail: Option<u32>,
    reach: Option<&str>,
    in_subagent: bool,
) -> Result<Vec<TranscriptMessage>, ReadFailed> {
    let wanted = tail
        .map_or(TRANSCRIPT_DEFAULT, |tail| tail as usize)
        .clamp(1, TRANSCRIPT_MAX);
    let mut budget = reach_budget(path, reach);
    loop {
        let window = read_tail(path, budget).ok_or_else(|| ReadFailed {
            path: path.to_path_buf(),
        })?;
        let mut messages: Vec<TranscriptMessage> = Vec::new();
        let mut model: Option<String> = None;
        let mut calls: HashMap<String, usize> = HashMap::new();
        let mut agents: HashMap<String, usize> = HashMap::new();
        for (record, line) in records(&window.buffer, window.base) {
            if let Some(next) = line_model(harness, &line) {
                model = Some(next);
            }
            let from = messages.len();
            match harness {
                Harness::Claude => claude_messages(&line, record, in_subagent, &mut messages),
                Harness::Codex => codex_messages(&line, record, &mut messages),
                Harness::Grok => grok_messages(&line, record, &mut messages),
            }
            for message in &mut messages[from..] {
                if message.role == Role::Assistant {
                    message.model.clone_from(&model);
                }
            }
            let tools = (from..messages.len()).filter(|&index| messages[index].tool.is_some());
            for (index, id) in tools.zip(tool_exit::call_ids(harness, &line, in_subagent)) {
                if !id.is_empty() {
                    calls.insert(id, index);
                }
            }
            for (id, code) in tool_exit::results(harness, &line) {
                if let Some(&index) = calls.get(&id) {
                    messages[index].exit = Some(code);
                }
            }
            for settled in subagents::settled(harness, &line) {
                let by_call = settled.call_id.as_ref().and_then(|id| calls.get(id));
                let by_agent = settled.agent_id.as_ref().and_then(|id| agents.get(id));
                let Some(&index) = by_call.or(by_agent) else {
                    continue;
                };
                if let Some(id) = settled.agent_id {
                    agents.insert(id, index);
                }
                if let Some(subagent) = messages[index].subagent.as_mut() {
                    subagent.settle(settled.report);
                }
            }
        }
        if messages.len() >= wanted || window.from_start || budget >= TRANSCRIPT_MAX_BYTES {
            let mut skip = messages.len().saturating_sub(wanted);
            if let Some(found) =
                reach.and_then(|key| messages.iter().position(|message| message.key == key))
            {
                skip = skip.min(found.saturating_sub(REACH_CONTEXT));
            }
            messages.drain(..skip);
            return Ok(messages);
        }
        budget *= 4;
    }
}

pub fn read_transcript_file(
    harness: Harness,
    path: &Path,
    tail: Option<u32>,
    reach: Option<&str>,
) -> Result<Vec<TranscriptMessage>, ReadFailed> {
    read_messages(harness, path, tail, reach, false)
}

pub fn read_subagent_transcript_file(
    harness: Harness,
    parent: &Path,
    call_id: &str,
    tail: Option<u32>,
) -> Result<Option<Vec<TranscriptMessage>>, ReadFailed> {
    let Some(path) = subagents::transcript_of(parent, call_id) else {
        return Ok(None);
    };
    read_messages(harness, &path, tail, None, true).map(Some)
}
