use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use berth_core::ServerEvent;
use tokio::sync::{mpsc, oneshot, Notify};

const ANSWER_WITHIN: Duration = Duration::from_secs(10 * 60);

#[derive(Debug)]
pub(crate) enum Failure {
    NoApp,
    Failed(String),
}

type Reply = oneshot::Sender<Result<String, Failure>>;

#[derive(Default)]
struct State {
    next: u64,
    offers: Vec<(u64, mpsc::Sender<ServerEvent>)>,
    pending: HashMap<u64, (u64, Reply)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);
static OFFERED: LazyLock<Notify> = LazyLock::new(Notify::new);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Offer(u64);

impl Drop for Offer {
    fn drop(&mut self) {
        let mut state = state();
        state.offers.retain(|(id, _)| *id != self.0);
        let orphaned: Vec<u64> = state
            .pending
            .iter()
            .filter(|(_, (offer, _))| *offer == self.0)
            .map(|(request, _)| *request)
            .collect();
        for request in orphaned {
            if let Some((_, reply)) = state.pending.remove(&request) {
                let _ = reply.send(Err(Failure::NoApp));
            }
        }
    }
}

pub(crate) fn offer(out: mpsc::Sender<ServerEvent>) -> Offer {
    let id = {
        let mut state = state();
        state.next += 1;
        let id = state.next;
        state.offers.push((id, out));
        id
    };
    OFFERED.notify_waiters();
    Offer(id)
}

pub(crate) fn answer(request: u64, result: Result<String, String>) {
    if let Some((_, reply)) = state().pending.remove(&request) {
        let _ = reply.send(result.map_err(Failure::Failed));
    }
}

pub(crate) async fn offered(within: Duration) {
    let _ = tokio::time::timeout(within, OFFERED.notified()).await;
}

pub(crate) async fn transcribe(pcm: &Path) -> Result<String, Failure> {
    let pcm_path = pcm.to_string_lossy().into_owned();
    loop {
        let (offer, out, request, rx) = {
            let mut state = state();
            let Some((offer, out)) = state.offers.last().cloned() else {
                return Err(Failure::NoApp);
            };
            state.next += 1;
            let request = state.next;
            let (tx, rx) = oneshot::channel();
            state.pending.insert(request, (offer, tx));
            (offer, out, request, rx)
        };
        let event = ServerEvent::TranscribeVoice {
            request,
            pcm_path: pcm_path.clone(),
        };
        if out.send(event).await.is_err() {
            let mut state = state();
            state.pending.remove(&request);
            state.offers.retain(|(id, _)| *id != offer);
            continue;
        }
        return match tokio::time::timeout(ANSWER_WITHIN, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Failure::NoApp),
            Err(_) => {
                state().pending.remove(&request);
                Err(Failure::Failed(
                    "the app did not finish the transcript in 10 minutes".to_owned(),
                ))
            }
        };
    }
}
