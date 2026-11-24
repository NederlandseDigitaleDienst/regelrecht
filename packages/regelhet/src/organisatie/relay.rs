//! What the front asks of its processes, and how it sends a browser on.
//!
//! The front calls a process inside the cluster (`intern` in the file of
//! processes), with a statement for the side of the person logged in in the
//! header `Authorization: Organisatie <statement>`. It passes on only `GET`,
//! and only the routes of [`ALLOWED`]: what the pages of the front need to
//! sum up a process, and nothing that acts. A process that does not answer
//! within the timeout gives `502` for itself alone.
//!
//! A browser that opens a process goes there itself: `GET
//! /api/processen/{id}/openen?naar=<path>` answers `303` to the process's
//! `publiek` address, `/auth/organisatie?token=<statement>&naar=<path>`, where
//! the process makes its own session ([`crate::login::organisation_handoff`]).

use std::collections::BTreeSet;

use axum::Extension;
use axum::body::Body;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::task::JoinSet;

use crate::access::{ORGANISATION_SCHEME, Principal};
use crate::api::ApiError;
use crate::fsc4b::now_unix;
use crate::login::Host;
use crate::organisatie::processes::{Process, Side};
use crate::organisatie::{FrontState, PROCESS_TIMEOUT};

/// The routes of a process the front passes on, per side, after `api/`; a
/// `*` stands for one segment.
pub const ALLOWED: [(Side, &str); 9] = [
    (Side::Backoffice, "casus"),
    (Side::Backoffice, "ik"),
    (Side::Backoffice, "backoffice/werkvoorraad"),
    (Side::Backoffice, "backoffice/vervolg"),
    (Side::Backoffice, "backoffice/cellen/*/kroniek"),
    (Side::Citizen, "casus"),
    (Side::Citizen, "ik"),
    (Side::Citizen, "dossier"),
    (Side::Citizen, "formulier"),
];

/// Whether the front passes `path` (after `api/`, decoded) on to `side`. A
/// segment that a URL parser could read as anything but one segment (a
/// backslash, `?`, `#`, `%`, a control character) passes nowhere, so the
/// path the process receives is the path that was checked.
pub fn allowed(side: Side, path: &str) -> bool {
    if path
        .chars()
        .any(|c| matches!(c, '\\' | '?' | '#' | '%') || c.is_control())
    {
        return false;
    }
    ALLOWED.iter().any(|(s, pattern)| {
        *s == side && {
            let mut given = path.split('/');
            let mut wanted = pattern.split('/');
            loop {
                match (given.next(), wanted.next()) {
                    (None, None) => break true,
                    (Some(g), Some("*")) if !g.is_empty() && g != ".." && g != "." => {}
                    (Some(g), Some(w)) if g == w => {}
                    _ => break false,
                }
            }
        }
    })
}

/// What a process answered, or that it did not.
enum Answer {
    Given { status: StatusCode, body: Vec<u8> },
    Unreachable,
}

/// `GET <intern of side>/api/<path>` of `process` on behalf of `principal`.
async fn call(
    state: &FrontState,
    principal: &Principal,
    process: &Process,
    side: Side,
    path_and_query: &str,
) -> Answer {
    let statement = state.statement(principal, side);
    let token = match state
        .front
        .signer
        .sign(&statement, &process.audience(side), now_unix())
    {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(error = %e, "signing a statement");
            return Answer::Unreachable;
        }
    };
    let url = format!("{}/api/{path_and_query}", process.side(side).intern);
    let sent = state
        .http_client
        .get(&url)
        .timeout(PROCESS_TIMEOUT)
        .header(
            header::AUTHORIZATION,
            format!("{ORGANISATION_SCHEME} {token}"),
        )
        .send()
        .await;
    let response = match sent {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(process = %process.id, error = %e, "a process did not answer");
            return Answer::Unreachable;
        }
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    match response.bytes().await {
        Ok(body) => Answer::Given {
            status,
            body: body.to_vec(),
        },
        Err(e) => {
            tracing::warn!(process = %process.id, error = %e, "a process's answer broke off");
            Answer::Unreachable
        }
    }
}

/// The answer of a process that did not answer.
fn unreachable(process: &Process) -> ApiError {
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        format!("{} is not reachable", process.naam),
    )
}

/// `GET /api/processen/{id}/api/{*pad}`: the process's own answer, for a
/// route of [`ALLOWED`]; any other is `404`, as a route that does not exist.
pub async fn relay(
    State(state): State<FrontState>,
    Extension(principal): Extension<Principal>,
    Path((id, path)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Response, ApiError> {
    let process = state.process(&id)?;
    let side = state.side_of(&principal)?;
    if !allowed(side, &path) {
        tracing::warn!(process = %id, path, "a route the front does not pass on");
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "there is no such route",
        ));
    }
    let path_and_query = match query {
        Some(q) if !q.is_empty() => format!("{path}?{q}"),
        _ => path,
    };
    match call(&state, &principal, process, side, &path_and_query).await {
        Answer::Given { status, body } => {
            let mut response = Response::new(Body::from(body));
            *response.status_mut() = status;
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(response)
        }
        Answer::Unreachable => Err(unreachable(process)),
    }
}

/// What `openen` takes.
#[derive(Debug, Deserialize)]
pub struct Open {
    naar: Option<String>,
}

/// `GET /api/processen/{id}/openen?naar=<path>`: `303` to the process, with
/// a statement for the side of `principal`, to the path `naar` there (its
/// start without it).
pub async fn open(
    State(state): State<FrontState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
    Query(open): Query<Open>,
) -> Result<Response, ApiError> {
    let process = state.process(&id)?;
    let side = state.side_of(&principal)?;
    let naar = match open.naar.as_deref() {
        None | Some("") => "/",
        Some(path) => crate::login::local_path(path).ok_or_else(|| {
            ApiError::new(StatusCode::BAD_REQUEST, "`naar` is a path on the process")
        })?,
    };
    let token = state
        .front
        .signer
        .sign(
            &state.statement(&principal, side),
            &process.audience(side),
            now_unix(),
        )
        .map_err(|e| {
            tracing::error!(error = %e, "signing a statement");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the server failed on this request; the cause is in its log",
            )
        })?;
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("token", &token)
        .append_pair("naar", naar)
        .finish();
    tracing::info!(sub = %principal.sub(), process = %id, naar, "handing over");
    let target = format!("{}/auth/organisatie?{query}", process.side(side).publiek);
    Ok(Redirect::to(&target).into_response())
}

/// Per process its `id`, `naam`, the `titel` of its case and whether it
/// answered, in the order of the file; the processes are asked at once.
pub async fn cases(state: &FrontState, principal: &Principal, side: Side) -> Vec<Value> {
    let mut asked = JoinSet::new();
    for (index, process) in state.front.processes.iter().enumerate() {
        let (state, principal, process) = (state.clone(), principal.clone(), process.clone());
        asked.spawn(async move {
            let answer = call(&state, &principal, &process, side, "casus").await;
            (index, summary(&process, answer))
        });
    }
    let mut summaries: Vec<(usize, Value)> = asked.join_all().await;
    summaries.sort_by_key(|(index, _)| *index);
    summaries.into_iter().map(|(_, summary)| summary).collect()
}

/// A process as the overview shows it, from its answer to `GET /api/casus`.
fn summary(process: &Process, answer: Answer) -> Value {
    let case = match answer {
        Answer::Given { status, body } if status.is_success() => {
            serde_json::from_slice::<Value>(&body).ok()
        }
        _ => None,
    };
    let title = case
        .as_ref()
        .and_then(|c| c.get("titel"))
        .and_then(Value::as_str);
    json!({
        "id": process.id,
        "naam": process.naam,
        "titel": title,
        "bereikbaar": case.is_some(),
    })
}

/// The accounts of the test login: those of the processes, for the sides
/// the front serves, and the test citizens, each account once. A process
/// that does not answer, or logs in through Keycloak, adds none.
pub async fn test_personas(state: &FrontState) -> Vec<Value> {
    let surface = state.surface();
    let sides: Vec<Side> = [Side::Citizen, Side::Backoffice]
        .into_iter()
        .filter(|s| s.served_on(surface))
        .collect();
    let mut asked = JoinSet::new();
    for process in &state.front.processes {
        for &side in &sides {
            let url = format!("{}/auth/testpersonen", process.side(side).intern);
            let client = state.http_client.clone();
            asked.spawn(async move { (side, personas_at(&client, &url).await) });
        }
    }
    let mut personas: Vec<Value> = Vec::new();
    if Side::Citizen.served_on(surface) {
        personas.extend(crate::login::citizen_personas(state.subject_claim()));
    }
    let mut found: Vec<(Side, Vec<Value>)> = asked.join_all().await;
    // Citizens first, as a process lists them.
    found.sort_by_key(|(side, _)| *side != Side::Citizen);
    personas.extend(
        found
            .into_iter()
            .flat_map(|(side, list)| list.into_iter().filter(move |p| fits(p, side))),
    );
    let mut seen = BTreeSet::new();
    personas.retain(|p| seen.insert(p.get("sub").and_then(Value::as_str).map(str::to_string)));
    personas
}

/// Whether the test account `persona` belongs on `side`.
fn fits(persona: &Value, side: Side) -> bool {
    let citizen = persona.get("soort").and_then(Value::as_str) == Some("burger");
    citizen == (side == Side::Citizen)
}

/// The accounts the test login at `url` offers, or none.
async fn personas_at(client: &reqwest::Client, url: &str) -> Vec<Value> {
    let response = match client.get(url).timeout(PROCESS_TIMEOUT).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::info!(url, status = %r.status(), "a process offers no test accounts");
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!(url, error = %e, "a process did not answer");
            return Vec::new();
        }
    };
    match response.json::<Value>().await {
        Ok(Value::Array(list)) => list,
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_front_passes_on_only_the_listed_routes_of_the_side() {
        assert!(allowed(Side::Backoffice, "backoffice/werkvoorraad"));
        assert!(allowed(
            Side::Backoffice,
            "backoffice/cellen/gemeente_wilgendaal/kroniek"
        ));
        assert!(allowed(Side::Citizen, "dossier"));
        assert!(!allowed(Side::Citizen, "backoffice/werkvoorraad"));
        assert!(!allowed(Side::Backoffice, "dossier"));
        assert!(!allowed(Side::Backoffice, "backoffice/besluiten"));
        assert!(!allowed(Side::Backoffice, "backoffice/cellen//kroniek"));
        assert!(!allowed(Side::Backoffice, "backoffice/cellen/../kroniek"));
        assert!(!allowed(Side::Backoffice, "backoffice/werkvoorraad/x"));
        assert!(!allowed(Side::Citizen, "dossier/waarom/x"));
        // What a URL parser would read past the checked segment.
        for sneaky in [
            "backoffice/cellen/..\\..\\backoffice\\besluiten?/kroniek",
            "backoffice/cellen/x?y/kroniek",
            "backoffice/cellen/x#y/kroniek",
            "backoffice/cellen/x%2F/kroniek",
            "backoffice/cellen/x\n/kroniek",
        ] {
            assert!(!allowed(Side::Backoffice, sneaky), "{sneaky}");
        }
    }
}
