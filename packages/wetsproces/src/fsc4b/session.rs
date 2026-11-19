//! `CreateSession` over HTTPS (§4.3.4): the challenge, and the answer that
//! gives a session.
//!
//! 1. `POST /fsc4b/session/v1/challenge`, no body: the server makes a nonce
//!    of [`NONCE_BYTES`] random bytes and an ephemeral ECDH key pair, keeps
//!    the private half under the nonce for [`NONCE_TTL_SECS`] (the minute
//!    of step 23 of figure 10), and answers with a JWS signed by the
//!    organization's root key: `sub: challenge`, the nonce, and the
//!    ephemeral public key as base64 DER (§4.3.4.2.2.4).
//! 2. `POST /fsc4b/session/v1/complete` with `{ nonce,
//!    encrypted_challenge_result }`: the server takes the challenge (once:
//!    the row is gone after this, whatever follows), decrypts the JWE with
//!    the ephemeral key, checks the presentation ([`super::presentation`]),
//!    and keeps a session under the SHA-256 of a fresh token. It answers
//!    with the token, encrypted under the session key the app chose
//!    (steps 26 to 30 of figure 10), as a JWE with `alg: dir`.
//!
//! Every refusal answers 401 with the same message; the reason goes to the
//! log, without the nonce, the token or a key.

use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::api::{ApiError, AppState};
use crate::fsc4b::jose::{self, EphemeralKey, JoseError};
use crate::fsc4b::presentation::{Refusal, verify_presentation};
use crate::fsc4b::store::{Fsc4bStoreError, StoredChallenge, StoredSession, TokenHash};
use crate::fsc4b::{Fsc4b, now_unix};
use regelrecht_fsc4b::wire::CHALLENGE_SUB;

/// The random bytes of a nonce; §4.3.4.2.2.3 asks at least 16.
pub const NONCE_BYTES: usize = 16;

/// How long a challenge waits for its answer: one minute (step 23).
pub const NONCE_TTL_SECS: i64 = 60;

/// How long a session lasts, at most; never past the credential's `exp`.
pub const SESSION_TTL_SECS: i64 = 15 * 60;

/// The random bytes of a session token, written as hex: 48 alphanumeric
/// characters and 192 bits, where §4.3.4.2.2.10 asks at least 22 and 128.
pub const TOKEN_BYTES: usize = 24;

/// The media type of a compact JWS in an answer.
pub const JWT_MEDIA_TYPE: &str = "application/jwt";

/// The media type of a compact JWE in an answer.
pub const JOSE_MEDIA_TYPE: &str = "application/jose";

/// Why a challenge or its answer failed.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no challenge waits under this nonce: unknown, used, or older than a minute")]
    UnknownNonce,
    #[error("the presentation does not decrypt: {0}")]
    Decrypt(JoseError),
    #[error("{0}")]
    Refused(#[from] Refusal),
    #[error("{0}")]
    Store(#[from] Fsc4bStoreError),
    #[error("the cryptographic library: {0}")]
    Crypto(JoseError),
}

impl SessionError {
    /// The answer to the app: 401 for every refusal, alike, and 500 for the
    /// server's own failures; the cause goes to the log.
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Store(Fsc4bStoreError::Full) => {
                tracing::warn!("FSC voor Burgers: too many challenges wait");
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "too many sessions are being started; try again in a minute",
                )
            }
            Self::Store(_) | Self::Crypto(_) => {
                tracing::error!(error = ?self, "FSC voor Burgers: CreateSession failed");
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the server failed on this request; the cause is in its log",
                )
            }
            _ => {
                // Debug, not Display: a refusal may quote what the app sent,
                // and Debug escapes it for the log.
                tracing::warn!(reason = ?self, "FSC voor Burgers: presentation refused");
                ApiError::new(StatusCode::UNAUTHORIZED, "the presentation is not accepted")
            }
        }
    }
}

/// A new challenge at the Unix time `now`, as the JWS the app receives.
///
/// # Errors
///
/// When the store or the library fails.
pub async fn challenge(fsc: &Fsc4b, now: i64) -> Result<String, SessionError> {
    let nonce = jose::b64(&jose::random_bytes(NONCE_BYTES).map_err(SessionError::Crypto)?);
    let ephemeral = EphemeralKey::generate().map_err(SessionError::Crypto)?;
    let claims = json!({
        "sub": CHALLENGE_SUB,
        "nonce": nonce,
        "ephemeral_organization_ec_public_key": ephemeral.public_der_b64().map_err(SessionError::Crypto)?,
    });
    let jws = fsc
        .trust()
        .key()
        .sign(&claims, &[])
        .map_err(SessionError::Crypto)?;
    let kept = StoredChallenge {
        ephemeral_key: ephemeral.to_bytes().map_err(SessionError::Crypto)?,
        expires_at: now.saturating_add(NONCE_TTL_SECS),
    };
    fsc.store().put_challenge(&nonce, kept, now).await?;
    Ok(jws)
}

/// The body of `POST /v1/complete` (§4.3.4.2.2.8).
#[derive(Debug, Deserialize)]
pub struct Complete {
    pub nonce: String,
    pub encrypted_challenge_result: String,
}

/// A fresh session token from the CSPRNG.
fn new_token() -> Result<String, SessionError> {
    let bytes = jose::random_bytes(TOKEN_BYTES).map_err(SessionError::Crypto)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The answer to a challenge at the Unix time `now`: the session token,
/// encrypted under the session key, as a compact JWE.
///
/// # Errors
///
/// [`SessionError::UnknownNonce`] when no challenge waits under the nonce,
/// [`SessionError::Decrypt`] and [`SessionError::Refused`] when the
/// presentation fails, and the store's and library's failures.
pub async fn complete(fsc: &Fsc4b, body: &Complete, now: i64) -> Result<String, SessionError> {
    // Taken first: from here on the nonce is spent, whether this succeeds
    // or not, so a failed answer cannot be tried again.
    let challenge = fsc
        .store()
        .take_challenge(&body.nonce, now)
        .await?
        .ok_or(SessionError::UnknownNonce)?;
    let ephemeral =
        EphemeralKey::from_bytes(&challenge.ephemeral_key).map_err(SessionError::Crypto)?;
    let vp = ephemeral
        .decrypt(&body.encrypted_challenge_result)
        .map_err(SessionError::Decrypt)?;
    let verified = verify_presentation(fsc.trust(), &vp, &body.nonce, now)?;
    let credential = &verified.credential;
    let expires_at = now.saturating_add(SESSION_TTL_SECS).min(credential.exp);
    // Within the leeway on `exp` a credential still verifies, but a session
    // ending before it starts is no session.
    if expires_at <= now {
        return Err(Refusal::Expired.into());
    }
    let token = new_token()?;
    let session = StoredSession {
        aes_key: verified.session_key.clone(),
        bsn: credential.bsn.clone(),
        scope: credential.scope.clone(),
        app_key_thumbprint: credential.app_key.thumbprint(),
        expires_at,
    };
    fsc.store()
        .put_session(&TokenHash::of(&token), session, now)
        .await?;
    tracing::info!(
        app_key = %credential.app_key.thumbprint(),
        "FSC voor Burgers: session created"
    );
    verified
        .session_key
        .encrypt(token.as_bytes())
        .map_err(SessionError::Crypto)
}

/// The channel of `state`; the routes exist only when it is open.
pub(crate) fn channel(state: &AppState) -> Result<&Fsc4b, ApiError> {
    state
        .fsc4b()
        .map(AsRef::as_ref)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "FSC voor Burgers is not configured"))
}

/// A compact token as the body of an answer.
pub(crate) fn token_response(media_type: &'static str, token: String) -> Response {
    ([(header::CONTENT_TYPE, media_type)], token).into_response()
}

pub(crate) async fn challenge_route(State(state): State<AppState>) -> Result<Response, ApiError> {
    let fsc = channel(&state)?;
    let jws = challenge(fsc, now_unix())
        .await
        .map_err(SessionError::into_api_error)?;
    Ok(token_response(JWT_MEDIA_TYPE, jws))
}

pub(crate) async fn complete_route(
    State(state): State<AppState>,
    Json(body): Json<Complete>,
) -> Result<Response, ApiError> {
    let fsc = channel(&state)?;
    let jwe = complete(fsc, &body, now_unix())
        .await
        .map_err(SessionError::into_api_error)?;
    Ok(token_response(JOSE_MEDIA_TYPE, jwe))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::fsc4b::testing::{TestApp, TestStelsel};

    const NOW: i64 = 1_800_000_000;

    #[tokio::test]
    async fn a_challenge_answered_too_late_is_refused() {
        let stelsel = TestStelsel::new();
        let fsc = stelsel.channel();
        let app = TestApp::new();
        let jws = challenge(&fsc, NOW).await.unwrap();
        let credential = stelsel.credential(&app, NOW, |_| {});
        let body = app.answer(&fsc, &jws, &credential, stelsel.own_oin());
        let e = complete(&fsc, &body, NOW + NONCE_TTL_SECS)
            .await
            .unwrap_err();
        assert!(matches!(e, SessionError::UnknownNonce), "{e}");
    }

    #[tokio::test]
    async fn a_nonce_is_answered_once() {
        let stelsel = TestStelsel::new();
        let fsc = stelsel.channel();
        let app = TestApp::new();
        let jws = challenge(&fsc, NOW).await.unwrap();
        let credential = stelsel.credential(&app, NOW, |_| {});
        let body = app.answer(&fsc, &jws, &credential, stelsel.own_oin());
        complete(&fsc, &body, NOW + 1).await.unwrap();
        let e = complete(&fsc, &body, NOW + 2).await.unwrap_err();
        assert!(matches!(e, SessionError::UnknownNonce), "{e}");
    }

    #[tokio::test]
    async fn a_session_ends_with_its_credential() {
        let stelsel = TestStelsel::new();
        let fsc = stelsel.channel();
        let app = TestApp::new();
        let jws = challenge(&fsc, NOW).await.unwrap();
        let credential = stelsel.credential(&app, NOW, |c| {
            c.insert("exp".into(), (NOW + 120).into());
        });
        let body = app.answer(&fsc, &jws, &credential, stelsel.own_oin());
        let jwe = complete(&fsc, &body, NOW).await.unwrap();
        let token = String::from_utf8(app.session_key().decrypt(&jwe).unwrap()).unwrap();
        let hash = TokenHash::of(&token);
        assert!(
            fsc.store()
                .session(&hash, NOW + 119)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            fsc.store()
                .session(&hash, NOW + 120)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_credential_in_the_leeway_after_its_exp_gives_no_session() {
        let stelsel = TestStelsel::new();
        let fsc = stelsel.channel();
        let app = TestApp::new();
        let jws = challenge(&fsc, NOW).await.unwrap();
        let credential = stelsel.credential(&app, NOW - 3600, |c| {
            c.insert("exp".into(), (NOW - 10).into());
        });
        let body = app.answer(&fsc, &jws, &credential, stelsel.own_oin());
        let e = complete(&fsc, &body, NOW).await.unwrap_err();
        assert!(matches!(e, SessionError::Refused(Refusal::Expired)), "{e}");
    }

    #[tokio::test]
    async fn a_tampered_presentation_does_not_decrypt() {
        let stelsel = TestStelsel::new();
        let fsc = stelsel.channel();
        let app = TestApp::new();
        let jws = challenge(&fsc, NOW).await.unwrap();
        let credential = stelsel.credential(&app, NOW, |_| {});
        let mut body = app.answer(&fsc, &jws, &credential, stelsel.own_oin());
        body.encrypted_challenge_result =
            crate::fsc4b::testing::flip_ciphertext_bit(&body.encrypted_challenge_result);
        let e = complete(&fsc, &body, NOW).await.unwrap_err();
        assert!(
            matches!(e, SessionError::Decrypt(JoseError::Decrypt)),
            "{e}"
        );
    }
}
