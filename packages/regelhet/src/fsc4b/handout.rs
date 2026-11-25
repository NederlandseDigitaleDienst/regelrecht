//! What the channel hands an app so that it can compute on the citizen's
//! own device: the source data the executor holds about the citizen of the
//! session ([`super::GEGEVENS`]), signed, and the case itself
//! ([`super::REGELWERKEN`]).
//!
//! The source data are the answers of the executor's cell that need only
//! the login field (`regelrecht_poc_chronolexografie::brongegevens`), with
//! the BSN of the session in it, never a value from the app; the records of
//! the citizen's dossier they rest on; and the facts that come only after
//! an application. The whole is a JWS under the organization's key from the
//! Stelseldocument, so the citizen can later show where a value came from,
//! and it travels under the session key like every answer of the channel.
//!
//! Handing out is not recorded. It is no legal fact of the case, so it does
//! not belong in a chronicle, and a log of handouts beside the chronicles is
//! not built yet.

use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::access::Principal;
use crate::api::{self, ApiError, AppState, CitizenResult};
use crate::fsc4b::session::channel;
use crate::fsc4b::{Fsc4b, now_unix};
use crate::service::SourceData;

/// How long a client may keep the case: it changes only with a deployment.
const CACHE_CONTROL: &str = "public, max-age=300";

fn internal(what: &str, e: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %e, "FSC voor Burgers: {what}");
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "the server failed on this request; the cause is in its log",
    )
}

/// The claims of the signed source data `data` about `subject`, by the
/// organization `iss` at the Unix time `iat`, with the hash of the case's
/// bundle (`crate::fsc4b::bundle_hash`), so that the unsigned bundle of
/// `/fsc4b/regelwerken` is covered by the signature too.
///
/// # Errors
///
/// When a record does not serialize, a fault in the build.
pub fn claims(
    iss: &str,
    subject: &str,
    iat: i64,
    bundel_hash: &str,
    data: &SourceData,
) -> serde_json::Result<Value> {
    Ok(json!({
        "iss": iss,
        "sub": subject,
        "iat": iat,
        "bundel_hash": bundel_hash,
        "peildatum": data.day,
        "casus": data.case,
        "cel": data.cell,
        "regelwerken": data.answers.regelwerken,
        "antwoorden": data.answers.antwoorden,
        "records": serde_json::to_value(&data.records)?,
        "bij_aanvraag": data.answers.bij_aanvraag,
    }))
}

/// The source data about the citizen `principal` is, signed by `fsc`:
/// `{ondertekend, gegevens}`, where `gegevens` is the payload of the JWS
/// `ondertekend`.
///
/// # Errors
///
/// 403 when the principal is no citizen, and what [`Service::source_data`]
/// refuses; 500 when signing fails.
///
/// [`Service::source_data`]: crate::service::Service::source_data
pub(crate) async fn source_data_for(
    state: &AppState,
    fsc: &Fsc4b,
    principal: &Principal,
) -> CitizenResult {
    let citizen = principal.citizen(state.subject_claim())?;
    let subject = citizen.subject();
    let data = state.service().source_data(subject).await?;
    let claims = claims(
        fsc.trust().own_oin().as_str(),
        subject,
        now_unix(),
        fsc.bundle_hash(),
        &data,
    )
    .map_err(|e| internal("serializing the source data", e))?;
    let signed = fsc
        .trust()
        .key()
        .sign(&claims, &[])
        .map_err(|e| internal("signing the source data", e))?;
    Ok(json!({ "ondertekend": signed, "gegevens": claims }))
}

/// `GET` [`super::REGELWERKEN`]: the files of the source the server runs,
/// by their path in the source, without a session.
pub(crate) async fn regelwerken_route(State(state): State<AppState>) -> Result<Response, ApiError> {
    let fsc = channel(&state)?;
    Ok(([(header::CACHE_CONTROL, CACHE_CONTROL)], Json(fsc.bundle())).into_response())
}

/// `GET` [`super::APPEARANCE`]: how the screens of the process look, the
/// same answer `GET /api/uiterlijk` gives anyone on the web
/// ([`api::appearance_of`]), without a session.
pub(crate) async fn appearance_route(State(state): State<AppState>) -> Result<Response, ApiError> {
    channel(&state)?;
    Ok(Json(api::appearance_of(&state)).into_response())
}
