use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const PRESENCE_TTL: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Present {
    pub member: String,
    pub name: String,
    pub agents: Vec<String>,
}

pub struct Presence {
    ttl: Duration,
    seen: HashMap<(String, String), (Vec<String>, Instant)>,
}

impl Presence {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            seen: HashMap::new(),
        }
    }

    pub fn report(&mut self, member: &str, channels: HashMap<String, Vec<String>>) {
        self.seen.retain(|(_, m), _| m != member);
        let now = Instant::now();
        for (channel, agents) in channels {
            if !agents.is_empty() {
                self.seen
                    .insert((channel, member.to_string()), (agents, now));
            }
        }
    }

    pub fn on(&mut self, channel: &str) -> Vec<(String, Vec<String>)> {
        let ttl = self.ttl;
        self.seen.retain(|_, (_, at)| at.elapsed() < ttl);
        let mut found: Vec<_> = self
            .seen
            .iter()
            .filter(|((c, _), _)| c == channel)
            .map(|((_, m), (agents, _))| (m.clone(), agents.clone()))
            .collect();
        found.sort();
        found
    }
}
