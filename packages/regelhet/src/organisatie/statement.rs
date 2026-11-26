//! The statement a front signs about who is logged in, and how a process
//! checks one.
//!
//! A statement is a compact JWS (ES256) from `regelrecht_fsc4b::jose`, with
//! the RFC 7638 thumbprint of the front's key as `kid`, and the claims:
//!
//! | Claim | What |
//! |---|---|
//! | `iss` | the organisation's id |
//! | `aud` | `<process id>#burger` or `<process id>#backoffice` |
//! | `sub`, `rollen`, `claims`, `naam` | from the front's session, the roles already filtered by its surface |
//! | `inlog` | `testlogin` or `keycloak`: how the front logged them in |
//! | `modus` | `test` or `stable`, the front's mode |
//! | `iat`, `exp` | `exp` at most [`MAX_LIFETIME_SECS`] after `iat` |
//! | `jti` | unique; a process takes each one once |
//!
//! The front signs one per call to a process and one per hand-off of a
//! browser ([`Signer`]). A process ([`Acceptor`]) checks the signature with
//! the organisation's key, `iss`, `aud` (its own id and a side it serves),
//! the times, that every role is one of this server's, that a process in
//! stable mode gets no statement of test mode, and that the `jti` is new.
//! The key is pinned in the configuration or, in test mode, fetched from the
//! front and fetched again when a statement names another key.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use regelrecht_fsc4b::jose::{JoseError, Jwk, Jws, SigningKey, VerifyingKey, b64, random_bytes};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::access::ROLE_PREFIX;
use crate::config::{Membership, Mode, OrganisationKey};
use crate::organisatie::processes::Side;
use crate::store::Channel;
use crate::surface::Surface;

/// The longest a statement lives, in seconds: it is signed for one call or
/// one hand-off, which follow at once.
pub const MAX_LIFETIME_SECS: i64 = 60;

/// How far the clocks of front and process may differ, in seconds.
const CLOCK_SKEW_SECS: i64 = 30;

/// How long a process waits before it fetches the key again for a
/// statement that names another one: a front that rolled its key is found
/// within this time, and a stream of forged `kid`s costs one fetch per
/// period.
const REFETCH_AFTER: Duration = Duration::from_secs(10);

/// How long fetching the key may take.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Who is logged in at the front, as a statement carries them.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub sub: String,
    pub roles: BTreeSet<String>,
    pub claims: BTreeMap<String, Value>,
    pub name: Option<String>,
    /// How the front logged them in: [`Channel::TestLogin`] or
    /// [`Channel::Keycloak`].
    pub login: Channel,
    /// The front's mode.
    pub mode: Mode,
}

/// A statement a process took, with the side it was for and who signed it.
#[derive(Debug, Clone, PartialEq)]
pub struct Accepted {
    pub statement: Statement,
    pub side: Side,
    /// The organisation's id, the statement's `iss`.
    pub issuer: String,
}

/// Why a process refuses a statement. The variants say which check failed,
/// for the log and the tests; an answer to a caller says less.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StatementError {
    #[error("the statement: {0}")]
    Jose(#[from] JoseError),
    #[error("the organisation's key is not available: {0}")]
    NoKey(String),
    #[error("the statement is signed with another key")]
    UnknownKey,
    #[error("the statement is from `{0}`, not from this process's organisation")]
    Issuer(String),
    #[error("the statement is for `{0}`, not for a side this instance serves")]
    Audience(String),
    #[error("the statement has expired or is not valid yet")]
    Expired,
    #[error("the statement lives longer than {MAX_LIFETIME_SECS} seconds")]
    Lifetime,
    #[error("the statement carries `{0}`, which is no role of this server")]
    Role(String),
    #[error("a process in stable mode takes no statement of a front in test mode")]
    Mode,
    #[error("the statement was used before")]
    Replayed,
    #[error("the statement lacks `{0}` or holds it in another form")]
    Claim(&'static str),
}

/// The front's key, which signs its statements.
pub struct Signer {
    key: SigningKey,
    issuer: String,
    kid: String,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("issuer", &self.issuer)
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

impl Signer {
    /// Statements of the organisation `issuer`, signed with `key`.
    pub fn new(key: SigningKey, issuer: &str) -> Self {
        let kid = key.public_jwk().thumbprint();
        Self {
            key,
            issuer: issuer.to_string(),
            kid,
        }
    }

    /// The organisation's id.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The public key, as `GET /organisatie/v1/sleutel` gives it: a JWK with
    /// its `kid`, `alg` and `use`.
    pub fn public_jwk(&self) -> Jwk {
        Jwk {
            kid: Some(self.kid.clone()),
            alg: Some("ES256".into()),
            use_: Some("sig".into()),
            ..self.key.public_jwk().clone()
        }
    }

    /// `statement`, signed for `audience` at `now` (seconds since the epoch),
    /// valid for [`MAX_LIFETIME_SECS`].
    ///
    /// # Errors
    ///
    /// When the random `jti` or the signature fails.
    pub fn sign(
        &self,
        statement: &Statement,
        audience: &str,
        now: i64,
    ) -> Result<String, JoseError> {
        let claims = json!({
            "iss": self.issuer,
            "aud": audience,
            "sub": statement.sub,
            "rollen": statement.roles,
            "claims": statement.claims,
            "naam": statement.name,
            "inlog": statement.login.as_str(),
            "modus": statement.mode.to_string(),
            "iat": now,
            "exp": now + MAX_LIFETIME_SECS,
            "jti": b64(&random_bytes(16)?),
        });
        self.key.sign(&claims, &[("kid", &self.kid)])
    }
}

/// Where an acceptor gets the organisation's key.
enum Keys {
    Pinned(Arc<VerifyingKey>),
    Fetched {
        url: String,
        client: reqwest::Client,
        cached: RwLock<Option<Attempt>>,
    },
}

/// The last attempt to fetch the key: the key it gave, or the one kept from
/// before when it failed, and when it was.
struct Attempt {
    key: Option<Arc<VerifyingKey>>,
    at: Instant,
}

/// A process's check of the statements of its organisation.
pub struct Acceptor {
    issuer: String,
    process_id: String,
    surface: Surface,
    mode: Mode,
    keys: Keys,
    /// The `jti`s taken, with their `exp`, until they expire.
    used: Mutex<HashMap<String, i64>>,
}

impl std::fmt::Debug for Acceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Acceptor")
            .field("issuer", &self.issuer)
            .field("process_id", &self.process_id)
            .field("surface", &self.surface)
            .finish_non_exhaustive()
    }
}

impl Acceptor {
    /// The check of `membership` for an instance on `surface` in `mode`.
    ///
    /// # Errors
    ///
    /// When a pinned key is no P-256 JWK.
    pub fn build(
        membership: &Membership,
        surface: Surface,
        mode: Mode,
    ) -> Result<Self, StatementError> {
        let keys = match &membership.key {
            OrganisationKey::Jwk(text) => {
                let value: Value =
                    serde_json::from_str(text).map_err(|_| StatementError::Jose(JoseError::Key))?;
                Keys::Pinned(Arc::new(VerifyingKey::from_value(&value)?))
            }
            OrganisationKey::Url(url) => Keys::Fetched {
                url: url.clone(),
                client: regelrecht_auth::http_client(),
                cached: RwLock::new(None),
            },
        };
        Ok(Self {
            issuer: membership.organisation_id.clone(),
            process_id: membership.process_id.clone(),
            surface,
            mode,
            keys,
            used: Mutex::new(HashMap::new()),
        })
    }

    /// The organisation's id.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The statement in `token`, checked at `now` (seconds since the epoch)
    /// and marked as used.
    ///
    /// # Errors
    ///
    /// [`StatementError`] for the first check that fails.
    pub async fn accept(&self, token: &str, now: i64) -> Result<Accepted, StatementError> {
        let unverified = Jws::unverified(token)?;
        let key = self.key(unverified.header_text("kid")).await?;
        let jws = key.verify(token)?;
        let accepted = self.check(&jws, now)?;
        let jti = jws.text("jti").ok_or(StatementError::Claim("jti"))?;
        let exp = jws
            .claims
            .get("exp")
            .and_then(Value::as_i64)
            .ok_or(StatementError::Claim("exp"))?;
        self.take(jti, exp, now)?;
        Ok(accepted)
    }

    /// The claims of a verified statement, checked.
    fn check(&self, jws: &Jws, now: i64) -> Result<Accepted, StatementError> {
        let iss = jws.text("iss").ok_or(StatementError::Claim("iss"))?;
        if iss != self.issuer {
            return Err(StatementError::Issuer(iss.to_string()));
        }
        let aud = jws.text("aud").ok_or(StatementError::Claim("aud"))?;
        let side = aud
            .strip_prefix(self.process_id.as_str())
            .and_then(|rest| rest.strip_prefix('#'))
            .and_then(Side::from_name)
            .filter(|side| side.served_on(self.surface))
            .ok_or_else(|| StatementError::Audience(aud.to_string()))?;
        let time = |name| {
            jws.claims
                .get(name)
                .and_then(Value::as_i64)
                .ok_or(StatementError::Claim(name))
        };
        let (iat, exp) = (time("iat")?, time("exp")?);
        if exp - iat > MAX_LIFETIME_SECS {
            return Err(StatementError::Lifetime);
        }
        if exp < now - CLOCK_SKEW_SECS || iat > now + CLOCK_SKEW_SECS {
            return Err(StatementError::Expired);
        }
        let mode = match jws.text("modus") {
            Some("test") => Mode::Test,
            Some("stable") => Mode::Stable,
            _ => return Err(StatementError::Claim("modus")),
        };
        if self.mode == Mode::Stable && mode == Mode::Test {
            return Err(StatementError::Mode);
        }
        let login = match jws.text("inlog") {
            Some("testlogin") => Channel::TestLogin,
            Some("keycloak") => Channel::Keycloak,
            _ => return Err(StatementError::Claim("inlog")),
        };
        let sub = jws
            .text("sub")
            .filter(|s| !s.trim().is_empty())
            .ok_or(StatementError::Claim("sub"))?;
        let roles: BTreeSet<String> = jws
            .claims
            .get("rollen")
            .map(|r| serde_json::from_value(r.clone()))
            .transpose()
            .map_err(|_| StatementError::Claim("rollen"))?
            .unwrap_or_default();
        if let Some(foreign) = roles.iter().find(|r| !r.starts_with(ROLE_PREFIX)) {
            return Err(StatementError::Role(foreign.clone()));
        }
        let claims: BTreeMap<String, Value> = match jws.claims.get("claims") {
            None | Some(Value::Null) => BTreeMap::new(),
            Some(c) => {
                serde_json::from_value(c.clone()).map_err(|_| StatementError::Claim("claims"))?
            }
        };
        Ok(Accepted {
            statement: Statement {
                sub: sub.to_string(),
                roles,
                claims,
                name: jws.text("naam").map(str::to_string),
                login,
                mode,
            },
            side,
            issuer: iss.to_string(),
        })
    }

    /// Mark `jti`, valid until `exp`, as used, and forget those that expired.
    fn take(&self, jti: &str, exp: i64, now: i64) -> Result<(), StatementError> {
        // A panic elsewhere while the set was held leaves it whole: each
        // change is one insert or one retain.
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        used.retain(|_, until| *until >= now - CLOCK_SKEW_SECS);
        if used.insert(jti.to_string(), exp).is_some() {
            return Err(StatementError::Replayed);
        }
        Ok(())
    }

    /// The organisation's key for a statement that names `kid`.
    async fn key(&self, kid: Option<&str>) -> Result<Arc<VerifyingKey>, StatementError> {
        let fits = |key: &VerifyingKey| kid.is_none_or(|k| k == key.thumbprint());
        let (url, client, cached) = match &self.keys {
            Keys::Pinned(key) if fits(key) => return Ok(Arc::clone(key)),
            Keys::Pinned(_) => return Err(StatementError::UnknownKey),
            Keys::Fetched {
                url,
                client,
                cached,
            } => (url, client, cached),
        };
        // What the last attempt decides, unless it is due for another.
        let decided =
            |last: Option<&Attempt>| -> Option<Result<Arc<VerifyingKey>, StatementError>> {
                let last = last?;
                match &last.key {
                    Some(key) if fits(key) => Some(Ok(Arc::clone(key))),
                    _ if last.at.elapsed() >= REFETCH_AFTER => None,
                    Some(_) => Some(Err(StatementError::UnknownKey)),
                    None => Some(Err(StatementError::NoKey(format!(
                        "{url}: the last attempt failed"
                    )))),
                }
            };
        if let Some(result) = decided(cached.read().await.as_ref()) {
            return result;
        }
        let mut slot = cached.write().await;
        // Another request may have fetched it meanwhile.
        if let Some(result) = decided(slot.as_ref()) {
            return result;
        }
        match fetch(client, url).await {
            Ok(key) => {
                let key = Arc::new(key);
                *slot = Some(Attempt {
                    key: Some(Arc::clone(&key)),
                    at: Instant::now(),
                });
                if fits(&key) {
                    Ok(key)
                } else {
                    Err(StatementError::UnknownKey)
                }
            }
            Err(e) => {
                // A failed fetch keeps the key there was, and waits as long
                // as a successful one before the next try.
                let kept = slot.take().and_then(|a| a.key);
                *slot = Some(Attempt {
                    key: kept,
                    at: Instant::now(),
                });
                Err(e)
            }
        }
    }
}

/// The key at `url`, a JWK.
async fn fetch(client: &reqwest::Client, url: &str) -> Result<VerifyingKey, StatementError> {
    let failed = |e: &dyn std::fmt::Display| StatementError::NoKey(format!("{url}: {e}"));
    let response = client
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| failed(&e))?;
    if !response.status().is_success() {
        return Err(failed(&response.status()));
    }
    let value: Value = response.json().await.map_err(|e| failed(&e))?;
    Ok(VerifyingKey::from_value(&value)?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const PROCESS: &str = "tnt-a/parkeerkaart";
    const ORGANISATION: &str = "tnt-o/wilgendaal";
    const NOW: i64 = 1_800_000_000;

    fn statement(roles: &[&str]) -> Statement {
        Statement {
            sub: "beslisser-1".into(),
            roles: roles.iter().map(|r| r.to_string()).collect(),
            claims: BTreeMap::new(),
            name: Some("Beslisser".into()),
            login: Channel::TestLogin,
            mode: Mode::Test,
        }
    }

    fn acceptor(key: &SigningKey, surface: Surface, mode: Mode) -> Acceptor {
        Acceptor::build(
            &Membership {
                process_id: PROCESS.into(),
                organisation_id: ORGANISATION.into(),
                name: "Gemeente Wilgendaal".into(),
                url: None,
                key: OrganisationKey::Jwk(serde_json::to_string(key.public_jwk()).unwrap()),
            },
            surface,
            mode,
        )
        .unwrap()
    }

    fn new_signer() -> (Signer, SigningKey) {
        let key = SigningKey::generate().unwrap();
        let pem = key.to_pkcs8_pem().unwrap();
        (
            Signer::new(SigningKey::from_pkcs8_pem(&pem).unwrap(), ORGANISATION),
            key,
        )
    }

    #[tokio::test]
    async fn a_statement_is_taken_once() {
        let (signer, key) = new_signer();
        let acceptor = acceptor(&key, Surface::Backoffice, Mode::Test);
        let token = signer
            .sign(
                &statement(&["regelhet-beslisser"]),
                &format!("{PROCESS}#backoffice"),
                NOW,
            )
            .unwrap();
        let accepted = acceptor.accept(&token, NOW + 5).await.unwrap();
        assert_eq!(accepted.side, Side::Backoffice);
        assert_eq!(accepted.issuer, ORGANISATION);
        assert_eq!(accepted.statement, statement(&["regelhet-beslisser"]));
        assert_eq!(
            acceptor.accept(&token, NOW + 6).await.unwrap_err(),
            StatementError::Replayed
        );
    }

    #[tokio::test]
    async fn a_statement_with_another_key_audience_or_time_is_refused() {
        let (signer, key) = new_signer();
        let backoffice = acceptor(&key, Surface::Backoffice, Mode::Test);
        let aud = format!("{PROCESS}#backoffice");
        let s = statement(&["regelhet-beslisser"]);
        // Expired: signed two minutes ago.
        let old = signer.sign(&s, &aud, NOW - 120).unwrap();
        assert_eq!(
            backoffice.accept(&old, NOW).await.unwrap_err(),
            StatementError::Expired
        );
        // Another process, or the side this instance does not serve.
        for aud in [
            "tnt-a/ander#backoffice",
            &format!("{PROCESS}#burger"),
            PROCESS,
        ] {
            let token = signer.sign(&s, aud, NOW).unwrap();
            assert_eq!(
                backoffice.accept(&token, NOW).await.unwrap_err(),
                StatementError::Audience(aud.to_string())
            );
        }
        // Another organisation's key.
        let (other, _) = new_signer();
        let token = other.sign(&s, &aud, NOW).unwrap();
        assert_eq!(
            backoffice.accept(&token, NOW).await.unwrap_err(),
            StatementError::UnknownKey
        );
        // Another issuer with the right key.
        let stranger = Signer::new(
            SigningKey::from_pkcs8_pem(&key.to_pkcs8_pem().unwrap()).unwrap(),
            "tnt-x/elders",
        );
        let token = stranger.sign(&s, &aud, NOW).unwrap();
        assert_eq!(
            backoffice.accept(&token, NOW).await.unwrap_err(),
            StatementError::Issuer("tnt-x/elders".into())
        );
    }

    #[tokio::test]
    async fn a_role_without_the_prefix_or_test_mode_on_a_stable_process_is_refused() {
        let (signer, key) = new_signer();
        let aud = format!("{PROCESS}#backoffice");
        let token = signer
            .sign(&statement(&["regelhet-lezer", "admin"]), &aud, NOW)
            .unwrap();
        assert_eq!(
            acceptor(&key, Surface::Backoffice, Mode::Test)
                .accept(&token, NOW)
                .await
                .unwrap_err(),
            StatementError::Role("admin".into())
        );
        let token = signer
            .sign(&statement(&["regelhet-lezer"]), &aud, NOW)
            .unwrap();
        assert_eq!(
            acceptor(&key, Surface::Backoffice, Mode::Stable)
                .accept(&token, NOW)
                .await
                .unwrap_err(),
            StatementError::Mode
        );
    }

    #[tokio::test]
    async fn an_instance_on_both_sides_takes_a_statement_for_either() {
        let (signer, key) = new_signer();
        let all = acceptor(&key, Surface::All, Mode::Test);
        for side in [Side::Citizen, Side::Backoffice] {
            let token = signer
                .sign(
                    &statement(&["regelhet-burger"]),
                    &crate::organisatie::processes::audience(PROCESS, side),
                    NOW,
                )
                .unwrap();
            assert_eq!(all.accept(&token, NOW).await.unwrap().side, side);
        }
    }

    /// A front at a free port of the loopback that serves `jwk` as its key
    /// (or fails with 500 when it is `None`), and the number of requests.
    async fn front_serving(jwk: Option<Value>) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&hits);
        let app = axum::Router::new().route(
            "/organisatie/v1/sleutel",
            axum::routing::get(move || {
                counted.fetch_add(1, Ordering::SeqCst);
                let jwk = jwk.clone();
                async move {
                    match jwk {
                        Some(j) => Ok(axum::Json(j)),
                        None => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await });
        (format!("http://{address}/organisatie/v1/sleutel"), hits)
    }

    fn fetching(url: String) -> Acceptor {
        Acceptor::build(
            &Membership {
                process_id: PROCESS.into(),
                organisation_id: ORGANISATION.into(),
                name: "Gemeente Wilgendaal".into(),
                url: None,
                key: OrganisationKey::Url(url),
            },
            Surface::Backoffice,
            Mode::Test,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn a_fetched_key_is_kept_and_a_forged_kid_costs_no_fetch() {
        use std::sync::atomic::Ordering;
        let (signer, _) = new_signer();
        let (url, hits) = front_serving(Some(json!(signer.public_jwk()))).await;
        let acceptor = fetching(url);
        let aud = format!("{PROCESS}#backoffice");
        let s = statement(&["regelhet-lezer"]);
        for _ in 0..3 {
            let token = signer.sign(&s, &aud, NOW).unwrap();
            acceptor.accept(&token, NOW).await.unwrap();
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        // Another key within the wait: refused without asking the front.
        let (other, _) = new_signer();
        let token = other.sign(&s, &aud, NOW).unwrap();
        assert_eq!(
            acceptor.accept(&token, NOW).await.unwrap_err(),
            StatementError::UnknownKey
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_fetch_is_not_tried_again_at_once() {
        use std::sync::atomic::Ordering;
        let (signer, _) = new_signer();
        let (url, hits) = front_serving(None).await;
        let acceptor = fetching(url);
        let aud = format!("{PROCESS}#backoffice");
        for _ in 0..2 {
            let token = signer
                .sign(&statement(&["regelhet-lezer"]), &aud, NOW)
                .unwrap();
            assert!(matches!(
                acceptor.accept(&token, NOW).await.unwrap_err(),
                StatementError::NoKey(_)
            ));
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_public_key_names_itself() {
        let (signer, key) = new_signer();
        let jwk = signer.public_jwk();
        assert_eq!(
            jwk.kid.as_deref(),
            Some(key.public_jwk().thumbprint().as_str())
        );
        assert_eq!(jwk.thumbprint(), key.public_jwk().thumbprint());
        VerifyingKey::from_jwk(&jwk).unwrap();
    }
}
