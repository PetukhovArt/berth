use crate::rights::{authorize, Member, Need, Permission, Principal};
use crate::server::Shared;
use crate::store::Event;
use crate::tls::{Endpoint, Io};
use anyhow::{bail, Context, Result};
use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, http::HeaderValue};
use tokio_tungstenite::WebSocketStream;

const PING_EVERY: Duration = Duration::from_secs(30);
const SILENT_HUB: Duration = Duration::from_secs(90);
const RIGHTS_CHANGED: u16 = 4001;

#[derive(Debug, Clone)]
pub(crate) struct Ended {
    pub(crate) member: String,
    pub(crate) why: EndedWhy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndedWhy {
    Revoked,
    RightsChanged,
}

impl EndedWhy {
    fn close_frame(self) -> CloseFrame {
        let (code, reason) = match self {
            Self::Revoked => (close_code::POLICY, "revoked"),
            Self::RightsChanged => (RIGHTS_CHANGED, "rights changed"),
        };
        CloseFrame {
            code,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Next {
    Event(Event),
    Closed,
    RightsChanged,
}

#[derive(Deserialize)]
pub(crate) struct StreamQuery {
    after: Option<i64>,
}

pub(crate) async fn stream(
    State(s): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<StreamQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let signed = authorize(&s.store.lock().unwrap(), &headers, Need::Signed);
    let reader = match signed.and_then(Principal::into_member) {
        Ok(m) => m,
        Err(e) => return <(StatusCode, String)>::from(e).into_response(),
    };
    let appended = s.appended.subscribe();
    let ended = s.ended.subscribe();
    ws.on_upgrade(move |socket| feed(s, (appended, ended), reader, q.after, socket))
}

async fn send(socket: &mut WebSocket, event: &Event) -> Result<()> {
    let text = serde_json::to_string(event)?;
    socket.send(Message::Text(text.into())).await?;
    Ok(())
}

async fn feed(
    s: Shared,
    (mut appended, mut ended): (Receiver<Event>, Receiver<Ended>),
    reader: Member,
    after: Option<i64>,
    mut socket: WebSocket,
) {
    let reads = reader.holds(Permission::ChannelsRead);
    let backlog = match after {
        Some(after) if reads => s.store.lock().unwrap().seen_after(&reader.id, after),
        _ => Ok(Vec::new()),
    };
    let Ok(backlog) = backlog else { return };
    let mut last = after.unwrap_or(0);
    for event in backlog {
        last = event.seq;
        if send(&mut socket, &event).await.is_err() {
            return;
        }
    }
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    loop {
        tokio::select! {
            got = appended.recv() => {
                let event = match got {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) | Err(RecvError::Closed) => return,
                };
                if !reads || event.seq <= last {
                    continue;
                }
                let sees = s.store.lock().unwrap().sees(&reader.id, &event.channel);
                if !sees.unwrap_or(false) {
                    continue;
                }
                last = event.seq;
                if send(&mut socket, &event).await.is_err() {
                    return;
                }
            }
            gone = ended.recv() => match gone {
                Ok(e) if e.member != reader.id => {}
                Ok(e) => {
                    let _ = socket.send(Message::Close(Some(e.why.close_frame()))).await;
                    return;
                }
                Err(_) => return,
            },
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            },
            _ = ping.tick() => {
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    return;
                }
            }
        }
    }
}

pub struct Subscription {
    socket: WebSocketStream<Box<dyn Io>>,
}

impl Subscription {
    pub(crate) async fn open(endpoint: &Endpoint, token: &str, after: Option<i64>) -> Result<Self> {
        let io = endpoint.connect().await?;
        let scheme = if endpoint.tls { "wss" } else { "ws" };
        let query = after.map(|a| format!("?after={a}")).unwrap_or_default();
        let mut request =
            format!("{scheme}://{}/stream{query}", endpoint.authority).into_client_request()?;
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
        let (socket, _) = tokio_tungstenite::client_async(request, io)
            .await
            .context("the hub refused the event stream")?;
        Ok(Self { socket })
    }

    pub async fn next(&mut self) -> Result<Next> {
        loop {
            let Ok(incoming) = tokio::time::timeout(SILENT_HUB, self.socket.next()).await else {
                bail!("the hub went silent");
            };
            match incoming {
                Some(Ok(tungstenite::Message::Close(Some(frame))))
                    if u16::from(frame.code) == RIGHTS_CHANGED =>
                {
                    return Ok(Next::RightsChanged)
                }
                None | Some(Ok(tungstenite::Message::Close(_))) => return Ok(Next::Closed),
                Some(Ok(tungstenite::Message::Text(text))) => {
                    return Ok(Next::Event(serde_json::from_str(&text)?))
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
            }
        }
    }
}
