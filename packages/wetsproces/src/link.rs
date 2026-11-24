//! The link with the servers of other cells of the case: how a record of
//! this server's own cell reaches the cell that records a receipt of it,
//! when that cell runs on a server of its own.
//!
//! One server can run every cell of a case, with an admin entering what an
//! external party did ([`crate::external`]). A link replaces that for one
//! party: the municipality's server and the GGD's each hold the chronicles
//! of the case, and each makes only the records of its own cell
//! ([`Service::own_cell`]). What the other cell did arrives as a message and
//! is taken over ([`Service::take_over`]), so each server's copy of the
//! other cell holds exactly what crossed the link.
//!
//! - **Outbound.** The chronicle is the outbox ([`Service::outbound`]): a
//!   record of the own cell that a linked cell records a receipt of is sent
//!   until that cell's server confirmed it. A worker ([`Link::spawn`]) sends
//!   what waits, in the order of the chronicle, and tries again later when a
//!   server does not answer. Nothing is written twice, so a crash between a
//!   commit and its sending loses nothing.
//! - **A message** is a compact JWS (ES256) of the sender's cell (`iss`),
//!   the receiver's (`aud`), the id of the record (`jti`), the moment of
//!   sending (`iat`), the fact, its fields and the day it was recorded on.
//! - **Inbound.** `POST /koppeling/v1/berichten` checks the signature under
//!   the key of the cell that claims to send, and takes the record over
//!   under a key made of that cell and the record's id: a message that
//!   arrives twice is recorded once.
//! - **Keys.** Each server signs with its own private key. The public key of
//!   a peer is pinned in the configuration; without one, in test mode only,
//!   it is read from the peer's `GET /koppeling/v1/sleutel` over TLS and
//!   read again when a signature stops verifying, as after a restart with a
//!   fresh key.
//!
//! The routes are shaped after the inway and outway of FSC: one address that
//! takes signed messages from known organizations, one worker that sends.
//!
//! `GET /koppeling/v1/partijen` exists without a link as well: it names the
//! parties the executor asks something of by its policy, which is what a
//! deployment reads to decide which party gets a server of its own.
//!
//! FSC itself (contracts, a directory, mutual TLS with PKIoverheid) is not
//! used; it could take the place of this transport without touching what is
//! recorded.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use regelrecht_fsc4b::jose::{JoseError, Jwk, Jws, SigningKey, VerifyingKey};
use serde_json::{Map, Value, json};
use tokio::sync::Notify;

use crate::api::{ApiError, AppState};
use crate::config::{KeySource, LinkSettings, Mode};
use crate::service::{Outbound, Service, ServiceError, TakeOver};

/// Where a server takes messages.
pub const MESSAGES_PATH: &str = "/koppeling/v1/berichten";

/// Where a server gives its public key.
pub const KEY_PATH: &str = "/koppeling/v1/sleutel";

/// Where a server says which parties of its case do work of their own, for
/// whoever deploys it.
pub const PARTIES_PATH: &str = "/koppeling/v1/partijen";

/// How far the moment of sending may lie from this server's clock.
const MAX_SKEW: Duration = Duration::from_secs(300);

/// How long the worker waits before it looks again, when nothing woke it.
const POLL: Duration = Duration::from_secs(3);

/// How long a key read from a peer counts before a signature that does not
/// verify has it read again: a caller without the key cannot make this
/// server ask the peer more often than that.
const KEY_REFRESH: Duration = Duration::from_secs(10);

/// How long one request to a peer may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Why a link does not open.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("the private key of the link: {0}")]
    Key(JoseError),
    #[error("reading the private key of the link from {path}: {reason}")]
    KeyFile { path: String, reason: String },
    #[error("the public key of the linked cell '{cell}': {reason}")]
    PeerKey { cell: String, reason: String },
    #[error("{0}")]
    Service(#[from] ServiceError),
}

/// The server of a linked cell.
#[derive(Debug)]
struct Peer {
    url: String,
    /// Its public key when the configuration pins one.
    pinned: Option<Jwk>,
}

/// The link of this server, open.
pub struct Link {
    own: String,
    key: SigningKey,
    peers: BTreeMap<String, Peer>,
    /// The public keys read from the peers themselves, each with the
    /// moment it was read.
    fetched: Mutex<BTreeMap<String, (Jwk, Instant)>>,
    http: reqwest::Client,
    /// Whether the test clock follows a sender whose clock is ahead.
    follow_clock: bool,
    wake: Notify,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("own", &self.own)
            .field("peers", &self.peers)
            .finish_non_exhaustive()
    }
}

/// The seconds since the Unix epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Link {
    /// The link `settings` describes, for the server of the cell `own`;
    /// none when it names no peer. Without a private key, which only test
    /// mode allows ([`crate::config`]), the server makes one that lasts
    /// until it stops.
    ///
    /// # Errors
    ///
    /// When the private key or a pinned public key cannot be read.
    pub fn open(
        settings: &LinkSettings,
        own: &str,
        mode: Mode,
        http: reqwest::Client,
    ) -> Result<Option<Self>, LinkError> {
        if settings.peers.is_empty() {
            return Ok(None);
        }
        let key = match &settings.key {
            Some(KeySource::Pem(pem)) => SigningKey::from_pkcs8_pem(pem),
            Some(KeySource::File(path)) => {
                let pem = std::fs::read_to_string(path).map_err(|e| LinkError::KeyFile {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                })?;
                SigningKey::from_pkcs8_pem(&pem)
            }
            None => SigningKey::generate(),
        }
        .map_err(LinkError::Key)?;
        let mut peers = BTreeMap::new();
        for peer in &settings.peers {
            let pinned = peer
                .key
                .as_deref()
                .map(|text| {
                    let jwk: Jwk = serde_json::from_str(text).map_err(|e| e.to_string())?;
                    VerifyingKey::from_jwk(&jwk).map_err(|e| e.to_string())?;
                    Ok(jwk)
                })
                .transpose()
                .map_err(|reason: String| LinkError::PeerKey {
                    cell: peer.cell.clone(),
                    reason,
                })?;
            peers.insert(
                peer.cell.clone(),
                Peer {
                    url: peer.url.clone(),
                    pinned,
                },
            );
        }
        Ok(Some(Self {
            own: own.to_string(),
            key,
            peers,
            fetched: Mutex::new(BTreeMap::new()),
            http,
            follow_clock: mode == Mode::Test,
            wake: Notify::new(),
        }))
    }

    /// The cells this server is linked with.
    pub fn cells(&self) -> impl Iterator<Item = &str> {
        self.peers.keys().map(String::as_str)
    }

    /// The routes of the link. No session guards them: a message proves
    /// its sender by its signature, and the key is public.
    pub fn routes() -> Router<AppState> {
        Router::new()
            .route(MESSAGES_PATH, post(receive))
            .route(KEY_PATH, get(public_key))
    }

    /// Have the worker look for records to send now, as after a step that
    /// may have made one.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Start the worker that sends what `service` has waiting, for as long
    /// as the server runs.
    pub fn spawn(self: &Arc<Self>, service: Arc<Service>) {
        let link = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                link.deliver(&service).await;
                tokio::select! {
                    () = link.wake.notified() => {}
                    () = tokio::time::sleep(POLL) => {}
                }
            }
        });
    }

    /// Send every record of `service` that waits, in the order of the
    /// chronicle, and say how many arrived. A cell whose server refuses or
    /// does not answer keeps what waits for it, in order, for a later round;
    /// the other cells go on.
    pub async fn deliver(&self, service: &Service) -> usize {
        let outbound = match service.outbound().await {
            Ok(outbound) => outbound,
            Err(e) => {
                tracing::error!(error = %e, "reading the records to send over the link");
                return 0;
            }
        };
        let mut stuck: Vec<String> = Vec::new();
        let mut sent = 0;
        for o in outbound {
            if stuck.contains(&o.to) {
                continue;
            }
            match self.send(&o).await {
                Ok(()) => match service.confirm_delivery(&o.gram.id, &o.to).await {
                    Ok(()) => sent += 1,
                    Err(e) => {
                        // It is sent again and recorded once there.
                        tracing::error!(error = %e, record = %o.gram.id, "keeping a delivery");
                        stuck.push(o.to);
                    }
                },
                Err(reason) => {
                    tracing::warn!(to = %o.to, record = %o.gram.id, %reason, "a record did not arrive; trying again later");
                    stuck.push(o.to);
                }
            }
        }
        sent
    }

    /// Send the record `o` to the server of its cell.
    async fn send(&self, o: &Outbound) -> Result<(), String> {
        let peer = self
            .peers
            .get(&o.to)
            .ok_or_else(|| format!("'{}' is no linked cell", o.to))?;
        let fact = o.gram.fact.as_deref().ok_or("the record names no fact")?;
        let claims = json!({
            "iss": self.own,
            "aud": o.to,
            "jti": o.gram.id,
            "iat": now(),
            "feit": format!("{}#{fact}", o.gram.basis),
            "velden": o.gram.fields,
            "datum": o.gram.recorded_on,
        });
        let jws = self.key.sign(&claims, &[]).map_err(|e| e.to_string())?;
        let response = self
            .http
            .post(format!("{}{MESSAGES_PATH}", peer.url))
            .header(reqwest::header::CONTENT_TYPE, "application/jose")
            .timeout(REQUEST_TIMEOUT)
            .body(jws)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(format!("{status}: {body}"))
    }

    /// The public key of the linked cell `cell`: the pinned one, or else
    /// the one read from its server, read anew when `refresh` says so.
    async fn key_of(&self, cell: &str, refresh: bool) -> Result<VerifyingKey, String> {
        let peer = self
            .peers
            .get(cell)
            .ok_or_else(|| format!("'{cell}' is no linked cell"))?;
        if let Some(jwk) = &peer.pinned {
            return VerifyingKey::from_jwk(jwk).map_err(|e| e.to_string());
        }
        let kept = self
            .fetched
            .lock()
            .map_err(|_| "the kept keys are poisoned".to_string())?
            .get(cell)
            .cloned();
        if let Some((jwk, read)) = kept
            && (!refresh || read.elapsed() < KEY_REFRESH)
        {
            return VerifyingKey::from_jwk(&jwk).map_err(|e| e.to_string());
        }
        let answer: Value = self
            .http
            .get(format!("{}{KEY_PATH}", peer.url))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if answer["cel"] != cell {
            return Err(format!(
                "the server at {} says it is {}, not '{cell}'",
                peer.url, answer["cel"]
            ));
        }
        let jwk: Jwk = serde_json::from_value(answer["sleutel"].clone())
            .map_err(|e| format!("the key of '{cell}': {e}"))?;
        let key = VerifyingKey::from_jwk(&jwk).map_err(|e| e.to_string())?;
        self.fetched
            .lock()
            .map_err(|_| "the kept keys are poisoned".to_string())?
            .insert(cell.to_string(), (jwk, Instant::now()));
        Ok(key)
    }

    /// The claims of the message `jws` when a linked cell signed it for
    /// this server, now.
    async fn verify(&self, jws: &str) -> Result<Jws, String> {
        let claimed = Jws::unverified(jws).map_err(|e| e.to_string())?;
        let from = claimed.text("iss").ok_or("no sender (`iss`)")?.to_string();
        let pinned = self
            .peers
            .get(&from)
            .ok_or_else(|| format!("'{from}' is no linked cell"))?
            .pinned
            .is_some();
        let message = match self.key_of(&from, false).await?.verify(jws) {
            Ok(message) => message,
            // The sender may have a new key since this one was read.
            Err(_) if !pinned => self
                .key_of(&from, true)
                .await?
                .verify(jws)
                .map_err(|e| e.to_string())?,
            Err(e) => return Err(e.to_string()),
        };
        if message.text("aud") != Some(self.own.as_str()) {
            return Err("the message is for another cell (`aud`)".into());
        }
        let sent = message
            .claims
            .get("iat")
            .and_then(Value::as_u64)
            .ok_or("no moment of sending (`iat`)")?;
        if now().abs_diff(sent) > MAX_SKEW.as_secs() {
            return Err("the message was not sent just now (`iat`)".into());
        }
        Ok(message)
    }
}

/// What a message holds beside who sent it.
struct Message {
    from: String,
    id: String,
    step: TakeOver,
}

fn read(message: &Jws) -> Result<Message, String> {
    let text = |name: &str| {
        message
            .text(name)
            .map(str::to_string)
            .ok_or_else(|| format!("the message has no `{name}`"))
    };
    let from = text("iss")?;
    let fields: Map<String, Value> = match message.claims.get("velden") {
        Some(Value::Object(fields)) => fields.clone(),
        None => Map::new(),
        Some(_) => return Err("`velden` is no object".into()),
    };
    let on: NaiveDate = text("datum")?
        .parse()
        .map_err(|_| "`datum` is no day".to_string())?;
    Ok(Message {
        id: text("jti")?,
        step: TakeOver {
            cell: from.clone(),
            fact: text("feit")?,
            fields,
            on,
        },
        from,
    })
}

/// A message of a linked cell: its record is taken over.
async fn receive(State(state): State<AppState>, body: String) -> Result<Json<Value>, ApiError> {
    let Some(link) = state.link() else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "there is no such route",
        ));
    };
    // One answer whatever check failed; the log names the check.
    let refused = |reason: String| {
        tracing::warn!(%reason, "a message over the link was refused");
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "the message is not one of a linked cell for this server",
        )
    };
    let verified = link.verify(body.trim()).await.map_err(refused)?;
    let message = read(&verified)
        .map_err(|reason| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, reason))?;
    state
        .service()
        .take_over(&message.from, &message.id, message.step, link.follow_clock)
        .await?;
    tracing::info!(from = %message.from, record = %message.id, "took a record over from a linked cell");
    Ok(Json(json!({ "ontvangen": message.id })))
}

/// Which parties of the case the executor asks something of by its policy,
/// and which of them this server is linked with. A deployment reads it to
/// give such a party a server of its own; it holds nothing the case itself
/// does not say, so it asks no login and exists without a link too.
pub(crate) async fn parties(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let service = state.service();
    let parties: Vec<Value> = service
        .asked_parties()
        .await?
        .iter()
        .map(|cel| {
            json!({
                "cel": cel,
                "cel_naam": service.cell_name(cel),
                "gekoppeld": service.is_linked(cel) || service.party() == Some(cel.as_str()),
            })
        })
        .collect();
    Ok(Json(json!({
        "uitvoerder": {
            "cel": service.executor(),
            "cel_naam": service.executor_name(),
        },
        "eigen_cel": service.own_cell(),
        "partijen": parties,
    })))
}

/// The public key messages of this server verify under.
async fn public_key(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let Some(link) = state.link() else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "there is no such route",
        ));
    };
    Ok(Json(json!({
        "cel": link.own,
        "sleutel": link.key.public_jwk(),
    })))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::PeerSetting;

    const OWN: &str = "ggd";
    const PEER: &str = "gemeente";

    /// A link of the cell `OWN` that knows `PEER` by the pinned public key
    /// of `peer`.
    fn link(peer: &SigningKey) -> Link {
        let settings = LinkSettings {
            party: None,
            peers: vec![PeerSetting {
                cell: PEER.into(),
                url: "https://gemeente.example.org".into(),
                key: Some(serde_json::to_string(peer.public_jwk()).unwrap()),
            }],
            key: None,
        };
        Link::open(&settings, OWN, Mode::Test, reqwest::Client::new())
            .unwrap()
            .unwrap()
    }

    fn claims() -> Value {
        json!({
            "iss": PEER, "aud": OWN, "jti": "1", "iat": now(),
            "feit": "wet#1#feit", "velden": { "a": 1 }, "datum": "2026-10-13",
        })
    }

    /// Whether the link refuses the claims after `change`, signed by `key`.
    async fn refused(link: &Link, key: &SigningKey, change: impl FnOnce(&mut Value)) -> bool {
        let mut c = claims();
        change(&mut c);
        link.verify(&key.sign(&c, &[]).unwrap()).await.is_err()
    }

    #[tokio::test]
    async fn a_message_of_the_peer_for_this_cell_sent_now_is_read() {
        let peer = SigningKey::generate().unwrap();
        let link = link(&peer);
        let jws = peer.sign(&claims(), &[]).unwrap();
        let message = read(&link.verify(&jws).await.unwrap()).unwrap();
        assert_eq!(message.from, PEER);
        assert_eq!(message.step.cell, PEER);
        assert_eq!(message.step.fact, "wet#1#feit");
        assert_eq!(message.step.on, "2026-10-13".parse::<NaiveDate>().unwrap());
    }

    #[tokio::test]
    async fn a_message_is_refused_for_each_claim_that_is_off() {
        let peer = SigningKey::generate().unwrap();
        let link = link(&peer);
        assert!(refused(&link, &peer, |c| c["aud"] = json!("rechtbank")).await);
        assert!(refused(&link, &peer, |c| c["iss"] = json!("rechtbank")).await);
        assert!(refused(&link, &peer, |c| c["iat"] = json!(now() - 3600)).await);
        assert!(refused(&link, &peer, |c| c["iat"] = json!(now() + 3600)).await);
        assert!(
            refused(&link, &peer, |c| {
                c.as_object_mut().unwrap().remove("iat");
            })
            .await
        );
        // Under another key than the pinned one, which is never read again
        // from the peer.
        assert!(refused(&link, &SigningKey::generate().unwrap(), |_| {}).await);
        assert!(!refused(&link, &peer, |_| {}).await);
    }

    #[test]
    fn a_message_without_a_day_a_fact_or_an_id_is_not_read() {
        let peer = SigningKey::generate().unwrap();
        for missing in ["datum", "feit", "jti"] {
            let mut c = claims();
            c.as_object_mut().unwrap().remove(missing);
            let jws = Jws::unverified(&peer.sign(&c, &[]).unwrap()).unwrap();
            assert!(read(&jws).is_err(), "{missing}");
        }
    }
}
