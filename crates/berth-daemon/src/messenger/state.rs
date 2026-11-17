use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use berth_core::AgentId;

const SENT_KEPT: usize = 2000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Awaiting {
    pub voice: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Queued {
    pub text: String,
    pub voice: bool,
}

#[derive(Default)]
pub(crate) struct Bridge {
    sent: Mutex<VecDeque<(i64, AgentId)>>,
    awaiting: Mutex<HashMap<AgentId, Awaiting>>,
    queued: Mutex<HashMap<AgentId, VecDeque<Queued>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Bridge {
    pub fn remember(&self, message: i64, agent: &AgentId) {
        let mut sent = lock(&self.sent);
        if sent.len() >= SENT_KEPT {
            sent.pop_front();
        }
        sent.push_back((message, agent.clone()));
    }

    pub fn agent_of(&self, message: i64) -> Option<AgentId> {
        lock(&self.sent)
            .iter()
            .rev()
            .find(|(id, _)| *id == message)
            .map(|(_, agent)| agent.clone())
    }

    pub fn expect_answer(&self, agent: &AgentId, voice: bool) {
        lock(&self.awaiting).insert(agent.clone(), Awaiting { voice });
    }

    pub fn awaiting(&self, agent: &AgentId) -> Option<Awaiting> {
        lock(&self.awaiting).get(agent).copied()
    }

    pub fn answered(&self, agent: &AgentId) {
        lock(&self.awaiting).remove(agent);
    }

    pub fn queue(&self, agent: &AgentId, prompt: Queued) -> usize {
        let mut queued = lock(&self.queued);
        let line = queued.entry(agent.clone()).or_default();
        line.push_back(prompt);
        line.len()
    }

    pub fn next_queued(&self, agent: &AgentId) -> Option<Queued> {
        let mut queued = lock(&self.queued);
        let line = queued.get_mut(agent)?;
        let next = line.pop_front();
        if line.is_empty() {
            queued.remove(agent);
        }
        next
    }
}
