//! The citizen's routes for a session of CreateSession, under
//! `/fsc4b/burger/v1/`.
//!
//! Every request is a `POST` with the session token in the `Authorization`
//! header (bare, as in §4.4.1.1.2.9, or after `Bearer `) and a body that is
//! a JWE (`alg: dir`, `enc: A256GCM`) under the session key, holding a JSON
//! object: the submission for `indieningen`, `{}` for the others. Without a
//! body that decrypts under the session's key, the token alone opens
//! nothing. The answer is a JWE under the same key, with the JSON the
//! browser routes answer, an error included, and the HTTP status beside it.
//!
//! The handlers are those of the browser routes ([`crate::api`]), given a
//! [`Principal`] with the citizen's role and the credential's BSN under the
//! subject claim: what they check and record does not depend on the
//! channel, except that the store keeps it beside each record.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::access::Principal;
use crate::api::{self, ApiError, AppState, CitizenResult};
use crate::fsc4b::jose::AesKey;
use crate::fsc4b::session::{JOSE_MEDIA_TYPE, channel, token_response};
use crate::fsc4b::store::TokenHash;
use crate::fsc4b::{Fsc4b, now_unix};
use crate::service::Submit;

/// What prefixes the account of an app in the store: the thumbprint of its
/// key follows.
pub const ACCOUNT_PREFIX: &str = "fsc4b:";

/// An authenticated request: who, the key its answer goes under, and what
/// it asked.
pub struct Authenticated {
    pub principal: Principal,
    pub key: AesKey,
    pub payload: Value,
}

fn unauthenticated() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "start a session with /fsc4b/session/v1/challenge first",
    )
}

/// The session token of `headers`.
fn token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let token = value.strip_prefix("Bearer ").unwrap_or(value).trim();
    (!token.is_empty()).then_some(token)
}

/// The session of `headers` and the decrypted `body`, at the Unix time
/// `now`.
///
/// # Errors
///
/// 401 when there is no live session under the token or the body does not
/// decrypt under its key; 400 when what decrypts is no JSON object; 500
/// when the store fails.
pub async fn authenticate(
    fsc: &Fsc4b,
    subject_claim: &str,
    headers: &HeaderMap,
    body: &str,
    now: i64,
) -> Result<Authenticated, ApiError> {
    let token = token(headers).ok_or_else(unauthenticated)?;
    let session = fsc
        .store()
        .session(&TokenHash::of(token), now)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "FSC voor Burgers: reading a session");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the server failed on this request; the cause is in its log",
            )
        })?
        .ok_or_else(unauthenticated)?;
    // A body under another key than the session's: a token without its key,
    // or another session's request. Either way, nothing is opened.
    let plaintext = session.aes_key.decrypt(body.trim()).map_err(|e| {
        tracing::warn!(reason = ?e, app_key = %session.app_key_thumbprint, "FSC voor Burgers: request refused");
        unauthenticated()
    })?;
    let payload = match serde_json::from_slice(&plaintext) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "the request decrypts to no JSON object",
            ));
        }
    };
    let principal = Principal::fsc4b_citizen(
        format!("{ACCOUNT_PREFIX}{}", session.app_key_thumbprint),
        subject_claim,
        &session.bsn,
    );
    Ok(Authenticated {
        principal,
        key: session.aes_key,
        payload,
    })
}

/// The answer `result` encrypted under `key`, with its status.
fn encrypted(key: &AesKey, result: CitizenResult) -> Response {
    let (status, body) = match result {
        Ok(value) => (StatusCode::OK, value),
        Err(e) => (e.status(), e.body()),
    };
    match key.encrypt(body.to_string().as_bytes()) {
        Ok(jwe) => (status, token_response(JOSE_MEDIA_TYPE, jwe)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "FSC voor Burgers: encrypting an answer");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the server failed on this request; the cause is in its log",
            )
            .into_response()
        }
    }
}

/// Which citizen's route a request is for.
enum Route {
    Case,
    Forms,
    Dossier,
    Submit,
}

async fn handle(state: &AppState, route: Route, headers: &HeaderMap, body: &str) -> Response {
    let fsc = match channel(state) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };
    let request = match authenticate(fsc, state.subject_claim(), headers, body, now_unix()).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    let p = &request.principal;
    let result = match route {
        Route::Case => Ok(api::case_of(state)),
        Route::Forms => api::forms_for(state, p).await,
        Route::Dossier => api::dossier_of(state, p).await,
        Route::Submit => match serde_json::from_value::<Submit>(request.payload) {
            Ok(s) => api::submit_for(state, p, headers, s).await,
            Err(e) => Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("the submission does not read: {e}"),
            )),
        },
    };
    encrypted(&request.key, result)
}

pub(crate) async fn case_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    handle(&state, Route::Case, &headers, &body).await
}

pub(crate) async fn forms_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    handle(&state, Route::Forms, &headers, &body).await
}

pub(crate) async fn dossier_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    handle(&state, Route::Dossier, &headers, &body).await
}

pub(crate) async fn submit_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    handle(&state, Route::Submit, &headers, &body).await
}
