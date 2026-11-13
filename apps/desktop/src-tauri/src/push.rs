use std::sync::Mutex;
use std::time::Duration;

use axum::http::Uri;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use web_push_native::jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair};
use web_push_native::p256::PublicKey;
use web_push_native::{Auth, WebPushBuilder};

use crate::connectors::secrets::{self, Lookup};

const SECRET_ID: &str = "web-push";
const CONTACT: &str = "https://github.com/PetukhovArt/berth";
const SUBSCRIPTION_LIMIT: usize = 20;
const PAYLOAD_LIMIT: usize = 3000;
const TTL: Duration = Duration::from_secs(5 * 60);
const SEND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Keys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    pub endpoint: String,
    pub keys: Keys,
}

#[derive(Serialize, Deserialize)]
struct Store {
    vapid: String,
    subscriptions: Vec<Subscription>,
}

pub struct Push {
    store: Mutex<Option<Store>>,
    http: reqwest::Client,
}

fn fresh() -> Store {
    Store {
        vapid: URL_SAFE_NO_PAD.encode(ES256KeyPair::generate().to_bytes()),
        subscriptions: Vec::new(),
    }
}

fn load() -> Result<Store, String> {
    match secrets::lookup(SECRET_ID) {
        Lookup::Found(text) => serde_json::from_str(&text)
            .map_err(|error| format!("web push keys are unreadable: {error}")),
        Lookup::Missing => {
            let store = fresh();
            persist(&store)?;
            Ok(store)
        }
        Lookup::Failed(message) => Err(message),
    }
}

fn persist(store: &Store) -> Result<(), String> {
    let text = serde_json::to_string(store).map_err(|error| error.to_string())?;
    secrets::save(SECRET_ID, &text)
}

fn key_pair(store: &Store) -> Result<ES256KeyPair, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(&store.vapid)
        .map_err(|error| error.to_string())?;
    ES256KeyPair::from_bytes(&bytes).map_err(|error| error.to_string())
}

fn valid(subscription: &Subscription) -> Result<(), String> {
    let uri: Uri = subscription
        .endpoint
        .parse()
        .map_err(|_| "invalid push endpoint".to_owned())?;
    if uri.scheme_str() != Some("https") {
        return Err("push endpoint must be https".to_owned());
    }
    let public = URL_SAFE_NO_PAD
        .decode(&subscription.keys.p256dh)
        .map_err(|_| "invalid p256dh".to_owned())?;
    PublicKey::from_sec1_bytes(&public).map_err(|_| "invalid p256dh".to_owned())?;
    let auth = URL_SAFE_NO_PAD
        .decode(&subscription.keys.auth)
        .map_err(|_| "invalid auth".to_owned())?;
    if auth.len() != 16 {
        return Err("invalid auth".to_owned());
    }
    Ok(())
}

fn request(
    pair: &ES256KeyPair,
    subscription: &Subscription,
    body: &[u8],
) -> Result<reqwest::Request, String> {
    let uri: Uri = subscription
        .endpoint
        .parse()
        .map_err(|_| "invalid push endpoint".to_owned())?;
    let public = URL_SAFE_NO_PAD
        .decode(&subscription.keys.p256dh)
        .map_err(|error| error.to_string())?;
    let auth = URL_SAFE_NO_PAD
        .decode(&subscription.keys.auth)
        .map_err(|error| error.to_string())?;
    let builder = WebPushBuilder::new(
        uri,
        PublicKey::from_sec1_bytes(&public).map_err(|error| error.to_string())?,
        Auth::clone_from_slice(&auth),
    )
    .with_valid_duration(TTL)
    .with_vapid(pair, CONTACT);
    let built = builder.build(body).map_err(|error| error.to_string())?;
    reqwest::Request::try_from(built).map_err(|error| error.to_string())
}

impl Push {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            store: Mutex::new(None),
            http,
        }
    }

    fn with<T>(&self, run: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(load()?);
        }
        let store = guard
            .as_mut()
            .ok_or_else(|| "web push unavailable".to_owned())?;
        run(store)
    }

    pub fn public_key(&self) -> Result<String, String> {
        self.with(|store| {
            let pair = key_pair(store)?;
            Ok(URL_SAFE_NO_PAD.encode(pair.public_key().public_key().to_bytes_uncompressed()))
        })
    }

    pub fn subscribe(&self, subscription: Subscription) -> Result<(), String> {
        valid(&subscription)?;
        self.with(|store| {
            store
                .subscriptions
                .retain(|known| known.endpoint != subscription.endpoint);
            store.subscriptions.push(subscription);
            let excess = store.subscriptions.len().saturating_sub(SUBSCRIPTION_LIMIT);
            store.subscriptions.drain(..excess);
            persist(store)
        })
    }

    pub fn unsubscribe(&self, endpoint: &str) -> Result<(), String> {
        self.with(|store| {
            store
                .subscriptions
                .retain(|known| known.endpoint != endpoint);
            persist(store)
        })
    }

    pub async fn notify(&self, payload: Value) -> Result<(), String> {
        let body = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
        if body.len() > PAYLOAD_LIMIT {
            return Err("push payload too large".to_owned());
        }
        let (pair, subscriptions) =
            self.with(|store| Ok((key_pair(store)?, store.subscriptions.clone())))?;
        let mut gone = Vec::new();
        for subscription in subscriptions {
            let Ok(request) = request(&pair, &subscription, &body) else {
                gone.push(subscription.endpoint);
                continue;
            };
            let sent = tokio::time::timeout(SEND_TIMEOUT, self.http.execute(request)).await;
            if let Ok(Ok(response)) = sent {
                let status = response.status().as_u16();
                if status == 404 || status == 410 {
                    gone.push(subscription.endpoint);
                }
            }
        }
        for endpoint in gone {
            self.unsubscribe(&endpoint)?;
        }
        Ok(())
    }
}
