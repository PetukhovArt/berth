use crate::store::Store;
use berth_core::AgentId;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ITEMS_KEPT_PER_AGENT: u64 = 5000;
const OPEN_WRITE_GAP: Duration = Duration::from_millis(250);

struct Tail {
    seq: u64,
    json: String,
    written_at: Instant,
    dirty: bool,
}

#[derive(Default)]
pub struct Persist {
    tails: Mutex<HashMap<AgentId, Tail>>,
}

impl Persist {
    pub fn put(&self, store: &Store, agent: &AgentId, seq: u64, json: String, open: bool) {
        let mut tails = self.tails.lock().unwrap();
        if tails
            .get(agent)
            .is_some_and(|tail| tail.seq != seq && tail.dirty)
        {
            if let Some(superseded) = tails.remove(agent) {
                write(store, agent, superseded.seq, &superseded.json);
            }
        }
        if !open {
            if tails.get(agent).is_some_and(|tail| tail.seq == seq) {
                tails.remove(agent);
            }
            write(store, agent, seq, &json);
            return;
        }
        let now = Instant::now();
        match tails.get_mut(agent).filter(|tail| tail.seq == seq) {
            Some(tail) => {
                tail.json = json;
                if now.duration_since(tail.written_at) >= OPEN_WRITE_GAP {
                    write(store, agent, seq, &tail.json);
                    tail.written_at = now;
                    tail.dirty = false;
                } else {
                    tail.dirty = true;
                }
            }
            None => {
                write(store, agent, seq, &json);
                tails.insert(
                    agent.clone(),
                    Tail {
                        seq,
                        json,
                        written_at: now,
                        dirty: false,
                    },
                );
            }
        }
    }

    pub fn flush(&self, store: &Store, agent: &AgentId) {
        if let Some(tail) = self.tails.lock().unwrap().remove(agent) {
            if tail.dirty {
                write(store, agent, tail.seq, &tail.json);
            }
        }
    }
}

fn write(store: &Store, agent: &AgentId, seq: u64, json: &str) {
    if let Err(error) = store.put_acp_item(agent, seq, json, ITEMS_KEPT_PER_AGENT) {
        tracing::warn!(agent = %agent, error = %error, "persist ACP item failed");
    }
}
