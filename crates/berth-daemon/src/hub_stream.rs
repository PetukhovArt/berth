use std::sync::Arc;
use std::time::Duration;

use berth_core::ServerEvent;
use tokio::sync::{watch, Notify};

use crate::channels::hub;
use crate::registry::Daemon;

const RETRY: Duration = Duration::from_secs(5);

pub(crate) struct HubPush {
    appended: Notify,
    live: watch::Sender<bool>,
}

impl Default for HubPush {
    fn default() -> Self {
        Self {
            appended: Notify::new(),
            live: watch::channel(false).0,
        }
    }
}

impl HubPush {
    pub(crate) fn is_live(&self) -> bool {
        *self.live.borrow()
    }

    pub(crate) fn watch_live(&self) -> watch::Receiver<bool> {
        self.live.subscribe()
    }

    pub(crate) async fn appended(&self) {
        self.appended.notified().await
    }

    fn set_live(&self, live: bool) {
        self.live.send_replace(live);
        self.appended.notify_one();
    }
}

impl Daemon {
    pub(crate) async fn run_hub_stream(self: Arc<Self>, push: Arc<HubPush>) {
        let mut last_seen: Option<i64> = None;
        loop {
            if let Ok(hub) = hub() {
                match hub.subscribe(last_seen).await {
                    Ok(mut subscription) => {
                        push.set_live(true);
                        loop {
                            let next = tokio::select! {
                                _ = self.shutdown.cancelled() => return,
                                next = subscription.next() => next,
                            };
                            let event = match next {
                                Ok(Some(event)) => event,
                                Ok(None) => break,
                                Err(e) => {
                                    tracing::debug!(error = %e, "hub event stream dropped");
                                    break;
                                }
                            };
                            last_seen = Some(last_seen.unwrap_or(0).max(event.seq));
                            self.broadcast(ServerEvent::ChannelAppended {
                                channel: event.channel,
                                seq: event.seq,
                            });
                            push.appended.notify_one();
                        }
                        push.set_live(false);
                    }
                    Err(e) => tracing::debug!(error = %e, "hub event stream unavailable"),
                }
            }
            tokio::select! {
                _ = self.shutdown.cancelled() => return,
                _ = tokio::time::sleep(RETRY) => {}
            }
        }
    }
}
