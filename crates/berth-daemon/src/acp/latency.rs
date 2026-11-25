use berth_core::AgentId;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

const BOUNDS_MS: [u128; 8] = [5, 10, 20, 50, 100, 200, 500, 1000];
const LOG_EVERY: u64 = 100;
const P95: u64 = 95;

#[derive(Default)]
struct Histogram {
    buckets: [u64; BOUNDS_MS.len() + 1],
    total: u64,
}

impl Histogram {
    fn add(&mut self, wait: Duration) {
        let at = BOUNDS_MS
            .iter()
            .position(|bound| wait.as_millis() <= *bound)
            .unwrap_or(BOUNDS_MS.len());
        self.buckets[at] += 1;
        self.total += 1;
    }

    fn p95_ms(&self) -> Option<u128> {
        let target = (self.total * P95).div_ceil(100);
        let mut seen = 0;
        for (at, count) in self.buckets.iter().enumerate() {
            seen += count;
            if seen >= target {
                return BOUNDS_MS.get(at).copied();
            }
        }
        None
    }
}

#[derive(Default)]
pub struct Latency {
    agents: Mutex<HashMap<AgentId, Histogram>>,
}

impl Latency {
    pub fn record(&self, agent: &AgentId, wait: Duration) {
        let mut agents = self.agents.lock().unwrap();
        let histogram = agents.entry(agent.clone()).or_default();
        histogram.add(wait);
        if histogram.total.is_multiple_of(LOG_EVERY) {
            tracing::debug!(
                agent = %agent,
                samples = histogram.total,
                p95_ms = ?histogram.p95_ms(),
                "ACP emitted minus read"
            );
        }
    }
}
