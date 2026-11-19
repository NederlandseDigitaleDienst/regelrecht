//! A demo Stelsel for test mode: at start the server makes a
//! Stelselbeheerder, a test Activatiedienst and its own organization key,
//! each a fresh key pair that lives only in memory, and signs a
//! Stelseldocument that lists them. The channel then trusts that document
//! through the same checks as a real one ([`super::trust`]), marked as a
//! demo, which stable mode refuses ([`super::Fsc4b::open`]).
//!
//! Three routes serve it, and exist only in this case:
//!
//! - `GET /fsc4b/demo/stelseldocument`: the signed Stelseldocument;
//! - `GET /fsc4b/demo/stelselbeheerder`: the Stelselbeheerder's public key,
//!   which an app would otherwise have built in;
//! - `POST /fsc4b/demo/activatiedienst/v1/credential`: a credential for the
//!   app key in a self-signed certificate (§4.3.4.1) and a BSN from the test
//!   range. There is no DigiD here: whoever can call it gets a credential
//!   for any test BSN, which is the point of a demo and why it is test mode
//!   only.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::json;

use crate::api::{ApiError, AppState};
use crate::fsc4b::jose::{JoseError, Jwk, Jws, SigningKey, VerifyingKey};
use crate::fsc4b::presentation::SELF_SIGNED_CERTIFICATE;
use crate::fsc4b::session::{JWT_MEDIA_TYPE, channel, token_response};
use crate::fsc4b::trust::{Trust, TrustInput};
use crate::fsc4b::{DISCOVERY, now_unix};

/// `GET`: the demo Stelseldocument.
pub const STELSELDOCUMENT: &str = "/fsc4b/demo/stelseldocument";
/// `GET`: the demo Stelselbeheerder's public key.
pub const STELSELBEHEERDER: &str = "/fsc4b/demo/stelselbeheerder";
/// `POST`: a credential of the test Activatiedienst.
pub const CREDENTIAL: &str = "/fsc4b/demo/activatiedienst/v1/credential";

/// The made-up OIN of the organization in the demo.
pub const DEMO_ORGANIZATION_OIN: &str = "00000099000000000001";
/// The made-up OIN of the test Activatiedienst.
pub const DEMO_ACTIVATIEDIENST_OIN: &str = "00000099000000000002";
/// The scope of the demo.
pub const DEMO_SCOPE: &str = "nl.regelrecht.wetsproces.demo";
/// What a test BSN starts with: the range of numbers made for testing,
/// which no person holds.
pub const TEST_BSN_PREFIX: &str = "99999";
/// How long a demo credential lasts.
pub const DEMO_CREDENTIAL_TTL_SECS: i64 = 24 * 60 * 60;

/// The keys and document of a demo Stelsel.
pub struct Demo {
    beheerder: SigningKey,
    activatiedienst: SigningKey,
    stelseldocument: String,
}

impl std::fmt::Debug for Demo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Demo")
    }
}

/// A Stelseldocument (version 1) signed by `beheerder`, listing one
/// organization and one Activatiedienst.
///
/// # Errors
///
/// When signing fails.
pub fn stelseldocument(
    beheerder: &SigningKey,
    organization: (&str, &Jwk, &str),
    activatiedienst: (&str, &Jwk),
) -> Result<String, JoseError> {
    let (oin, key, discovery_url) = organization;
    beheerder.sign(
        &json!({
            "version": 1,
            "organizations": [{
                "oin": oin, "name": "Demo-organisatie", "discovery_url": discovery_url, "public_key": key,
            }],
            "app_managers": [{
                "oin": activatiedienst.0, "name": "Test-Activatiedienst",
                "discovery_url": "", "public_key": activatiedienst.1,
            }],
            "document_types": [],
        }),
        &[],
    )
}

/// A credential (§4.3.1) of `issuer`, for the app key `app` and the BSN
/// `bsn`, from `now` until `exp`.
///
/// # Errors
///
/// When signing fails.
pub fn credential(
    issuer: &SigningKey,
    issuer_oin: &str,
    scope: &str,
    app: &Jwk,
    person: (&str, &str, &str),
    now: i64,
    exp: i64,
) -> Result<String, JoseError> {
    let (bsn, given_name, family_name) = person;
    issuer.sign(
        &json!({
            "cnf": { "jwk": app },
            "scope": scope,
            "iss": issuer_oin,
            "iat": now,
            "nbf": now,
            "exp": exp,
            "bsn": bsn,
            "given_name": given_name,
            "family_name": family_name,
        }),
        &[],
    )
}

impl Demo {
    /// A fresh demo Stelsel for a server at `public_url`, and the trust in
    /// it.
    ///
    /// # Errors
    ///
    /// When the library fails, or the document it signs does not pass the
    /// checks of a real one.
    pub fn generate(public_url: &str) -> Result<(Trust, Self), JoseError> {
        let beheerder = SigningKey::generate()?;
        let activatiedienst = SigningKey::generate()?;
        let own = SigningKey::generate()?;
        let document = stelseldocument(
            &beheerder,
            (
                DEMO_ORGANIZATION_OIN,
                own.public_jwk(),
                &format!("{public_url}{DISCOVERY}"),
            ),
            (DEMO_ACTIVATIEDIENST_OIN, activatiedienst.public_jwk()),
        )?;
        let trust = Trust::build(TrustInput {
            own_oin: DEMO_ORGANIZATION_OIN,
            key: own,
            scope: DEMO_SCOPE,
            stelseldocument: &document,
            stelselbeheerder: &beheerder.verifying_key()?,
        })
        .map_err(|e| {
            tracing::error!(error = %e, "the demo Stelsel does not pass its own checks");
            JoseError::Crypto
        })?
        .into_demo();
        Ok((
            trust,
            Self {
                beheerder,
                activatiedienst,
                stelseldocument: document,
            },
        ))
    }

    pub fn stelseldocument(&self) -> &str {
        &self.stelseldocument
    }

    /// The Stelselbeheerder's public key.
    pub fn stelselbeheerder(&self) -> &Jwk {
        self.beheerder.public_jwk()
    }

    /// A credential of the test Activatiedienst for the app key `app`.
    ///
    /// # Errors
    ///
    /// When signing fails.
    pub fn issue(
        &self,
        app: &Jwk,
        person: (&str, &str, &str),
        now: i64,
    ) -> Result<String, JoseError> {
        credential(
            &self.activatiedienst,
            DEMO_ACTIVATIEDIENST_OIN,
            DEMO_SCOPE,
            app,
            person,
            now,
            now.saturating_add(DEMO_CREDENTIAL_TTL_SECS),
        )
    }
}

/// The routes of the demo Stelsel.
pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route(STELSELDOCUMENT, get(stelseldocument_route))
        .route(STELSELBEHEERDER, get(stelselbeheerder_route))
        .route(CREDENTIAL, post(credential_route))
}

fn demo_of(state: &AppState) -> Result<&Demo, ApiError> {
    channel(state)?
        .demo()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "there is no demo Stelsel"))
}

async fn stelseldocument_route(State(state): State<AppState>) -> Result<Response, ApiError> {
    let demo = demo_of(&state)?;
    Ok(token_response(JWT_MEDIA_TYPE, demo.stelseldocument.clone()))
}

async fn stelselbeheerder_route(State(state): State<AppState>) -> Result<Json<Jwk>, ApiError> {
    Ok(Json(demo_of(&state)?.stelselbeheerder().clone()))
}

/// A request for a demo credential.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialRequest {
    /// A self-signed certificate (§4.3.4.1) of the app key.
    pub certificate: String,
    pub bsn: String,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub family_name: Option<String>,
}

/// The app key of a self-signed certificate, once its signature is checked
/// against that key (§4.3.4.1, the `certificate_type_self_signed` case).
///
/// # Errors
///
/// When the certificate is malformed, its `cnf` holds no valid key, it is
/// not signed by that key, or its `sub` is wrong.
pub fn self_signed_key(certificate: &str) -> Result<VerifyingKey, JoseError> {
    let unverified = Jws::unverified(certificate)?;
    let jwk = unverified
        .claims
        .get("cnf")
        .and_then(|c| c.get("jwk"))
        .ok_or(JoseError::Malformed("a certificate without `cnf.jwk`"))?;
    let key = VerifyingKey::from_value(jwk)?;
    let verified = key.verify(certificate)?;
    if verified.text("sub") != Some(SELF_SIGNED_CERTIFICATE) {
        return Err(JoseError::Malformed(
            "a self-signed certificate with another `sub`",
        ));
    }
    Ok(key)
}

fn is_test_bsn(bsn: &str) -> bool {
    bsn.len() == 9 && bsn.starts_with(TEST_BSN_PREFIX) && bsn.bytes().all(|b| b.is_ascii_digit())
}

async fn credential_route(
    State(state): State<AppState>,
    Json(request): Json<CredentialRequest>,
) -> Result<Response, ApiError> {
    let demo = demo_of(&state)?;
    if !is_test_bsn(&request.bsn) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "the test Activatiedienst gives credentials for test BSNs only ({TEST_BSN_PREFIX}xxxx)"
            ),
        ));
    }
    let key = self_signed_key(&request.certificate).map_err(|e| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("the certificate: {e}"),
        )
    })?;
    let given = request.given_name.as_deref().unwrap_or("Test");
    let family = request.family_name.as_deref().unwrap_or("Burger");
    let vc = demo
        .issue(key.jwk(), (&request.bsn, given, family), now_unix())
        .map_err(|e| {
            tracing::error!(error = %e, "the test Activatiedienst failed to sign");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the server failed on this request; the cause is in its log",
            )
        })?;
    Ok(token_response(JWT_MEDIA_TYPE, vc))
}

/// The self-signed certificate of `key` (§4.3.4.1).
///
/// # Errors
///
/// When signing fails.
pub fn self_signed_certificate(key: &SigningKey) -> Result<String, JoseError> {
    key.sign(
        &json!({ "sub": SELF_SIGNED_CERTIFICATE, "cnf": { "jwk": key.public_jwk() } }),
        &[],
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_stelsel_passes_the_checks_of_a_real_one_and_is_marked() {
        let (trust, demo) = Demo::generate("http://localhost:7180").unwrap();
        assert!(trust.is_demo());
        assert!(trust.issuer(DEMO_ACTIVATIEDIENST_OIN, None).is_some());
        let app = SigningKey::generate().unwrap();
        let vc = demo
            .issue(app.public_jwk(), ("999990019", "T", "B"), 1_800_000_000)
            .unwrap();
        let checked =
            crate::fsc4b::presentation::verify_credential(&trust, &vc, 1_800_000_001).unwrap();
        assert_eq!(checked.bsn, "999990019");
        assert_eq!(demo.stelselbeheerder().crv, "P-256");
    }

    #[test]
    fn a_self_signed_certificate_proves_its_own_key_only() {
        let key = SigningKey::generate().unwrap();
        let certificate = self_signed_certificate(&key).unwrap();
        assert_eq!(
            self_signed_key(&certificate).unwrap().jwk(),
            key.public_jwk()
        );
        // Someone else's key in `cnf`, signed with mine.
        let other = SigningKey::generate().unwrap();
        let forged = key
            .sign(
                &json!({ "sub": SELF_SIGNED_CERTIFICATE, "cnf": { "jwk": other.public_jwk() } }),
                &[],
            )
            .unwrap();
        assert_eq!(self_signed_key(&forged).unwrap_err(), JoseError::Signature);
    }

    #[test]
    fn only_test_bsns() {
        assert!(is_test_bsn("999990019"));
        assert!(!is_test_bsn("123456782"));
        assert!(!is_test_bsn("99999001"));
    }
}
