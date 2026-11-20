use std::sync::Arc;
use std::time::Duration;

use berth_core::ServerEvent;
use berth_hub::client::HubClient;
use berth_hub::Next;
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

enum StreamEnd {
    Shutdown,
    Dropped,
    RightsChanged,
}

impl Daemon {
    pub(crate) async fn run_hub_stream(self: Arc<Self>, push: Arc<HubPush>) {
        let mut last_seen: Option<i64> = None;
        loop {
            let end = match hub() {
                Ok(hub) => self.follow_stream(&hub, &push, &mut last_seen).await,
                Err(_) => StreamEnd::Dropped,
            };
            match end {
                StreamEnd::Shutdown => return,
                StreamEnd::RightsChanged => continue,
                StreamEnd::Dropped => {}
            }
            tokio::select! {
                _ = self.shutdown.cancelled() => return,
                _ = tokio::time::sleep(RETRY) => {}
            }
        }
    }

    async fn follow_stream(
        &self,
        hub: &HubClient,
        push: &HubPush,
        last_seen: &mut Option<i64>,
    ) -> StreamEnd {
        let mut subscription = match hub.subscribe(*last_seen).await {
            Ok(subscription) => subscription,
            Err(e) => {
                tracing::debug!(error = %e, "hub event stream unavailable");
                return StreamEnd::Dropped;
            }
        };
        push.set_live(true);
        self.broadcast(ServerEvent::HubChanged);
        let end = loop {
            let next = tokio::select! {
                _ = self.shutdown.cancelled() => break StreamEnd::Shutdown,
                next = subscription.next() => next,
            };
            let event = match next {
                Ok(Next::Event(event)) => event,
                Ok(Next::Closed) => break StreamEnd::Dropped,
                Ok(Next::RightsChanged) => break StreamEnd::RightsChanged,
                Err(e) => {
                    tracing::debug!(error = %e, "hub event stream dropped");
                    break StreamEnd::Dropped;
                }
            };
            *last_seen = Some(last_seen.unwrap_or(0).max(event.seq));
            self.broadcast(ServerEvent::ChannelAppended {
                channel: event.channel,
                seq: event.seq,
            });
            push.appended.notify_one();
        };
        push.set_live(false);
        end
    }
}
