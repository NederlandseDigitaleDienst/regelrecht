//! What the channel publishes without a login: the discovery configuration
//! of §4.2 and the OpenAPI specification §2.2.1 asks of a digital service.
//!
//! The discovery configuration names two services, each with its version
//! `v1` and the URL its routes hang under:
//!
//! ```json
//! {
//!   "sessionApi": { "v1": "https://…/fsc4b/session" },
//!   "wetsprocesBurgerApi": { "v1": "https://…/fsc4b/burger" }
//! }
//! ```
//!
//! §4.2.2 allows nothing but versions and URLs in it, while §2.2.4 asks the
//! specification of each service there as well; the specification is at
//! [`super::OPENAPI`], a fixed path beside them, and the docs page says so.
//!
//! The OpenAPI specification is written by hand (`openapi.json`); a test
//! holds its paths and methods to [`super::ROUTES`], and the end-to-end
//! tests call every one of them on a running server.

use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::api::{ApiError, AppState};
use crate::fsc4b::session::channel;
use crate::fsc4b::{CITIZEN_BASE, Fsc4b, SESSION_BASE};

/// The name of the session service in the discovery configuration, as in
/// the example of §4.2.5.
pub const SESSION_SERVICE: &str = "sessionApi";

/// The name of the citizen's service in the discovery configuration.
pub const CITIZEN_SERVICE: &str = "wetsprocesBurgerApi";

/// How long a client may keep the discovery configuration and the
/// specification (§4.2.4: "appropriate HTTP cache headers").
const CACHE_CONTROL: &str = "public, max-age=300";

/// The OpenAPI specification, without its `servers`.
const OPENAPI_JSON: &str = include_str!("openapi.json");

/// The discovery configuration of `fsc4b`.
pub fn discovery(fsc4b: &Fsc4b) -> Value {
    let base = fsc4b.public_url();
    json!({
        SESSION_SERVICE: { "v1": format!("{base}{SESSION_BASE}") },
        CITIZEN_SERVICE: { "v1": format!("{base}{CITIZEN_BASE}") },
    })
}

/// The OpenAPI specification with `fsc4b`'s address as its server.
///
/// # Errors
///
/// When the specification compiled in is no JSON object, a fault in the
/// build that the tests catch.
pub fn openapi(fsc4b: &Fsc4b) -> Result<Value, serde_json::Error> {
    let mut spec: Value = serde_json::from_str(OPENAPI_JSON)?;
    if let Some(spec) = spec.as_object_mut() {
        spec.insert("servers".into(), json!([{ "url": fsc4b.public_url() }]));
    }
    Ok(spec)
}

pub(crate) async fn discovery_route(State(state): State<AppState>) -> Result<Response, ApiError> {
    let fsc = channel(&state)?;
    Ok((
        [(header::CACHE_CONTROL, CACHE_CONTROL)],
        Json(discovery(fsc)),
    )
        .into_response())
}

pub(crate) async fn openapi_route(State(state): State<AppState>) -> Result<Response, ApiError> {
    let fsc = channel(&state)?;
    let spec = openapi(fsc).map_err(|e| {
        tracing::error!(error = %e, "the OpenAPI specification does not parse");
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "the server failed on this request; the cause is in its log",
        )
    })?;
    Ok(([(header::CACHE_CONTROL, CACHE_CONTROL)], Json(spec)).into_response())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn the_openapi_specification_describes_exactly_the_routes() {
        let spec: Value = serde_json::from_str(OPENAPI_JSON).unwrap();
        assert_eq!(spec["openapi"].as_str().map(|v| &v[..2]), Some("3."));
        let described: BTreeSet<(String, String)> = spec["paths"]
            .as_object()
            .unwrap()
            .iter()
            .flat_map(|(path, ops)| {
                ops.as_object()
                    .unwrap()
                    .keys()
                    .filter(|k| !k.starts_with("x-") && *k != "parameters")
                    .map(move |method| (method.clone(), path.clone()))
            })
            .collect();
        let routed: BTreeSet<(String, String)> = crate::fsc4b::ROUTES
            .iter()
            .map(|(m, p)| ((*m).to_string(), (*p).to_string()))
            .collect();
        assert_eq!(described, routed);
    }
}
