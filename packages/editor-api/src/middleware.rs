pub use regelrecht_auth::middleware::{refresh_session_token, require_role};
pub use regelrecht_auth::security_headers::{EDITOR_CSP, security_headers};

use axum::extract::Request;
use axum::http::{HeaderValue, header::HeaderName};
use axum::middleware::Next;
use axum::response::Response;
use regelrecht_corpus::timing;
use std::time::Instant;

/// `Server-Timing` header name, added to every response.
static SERVER_TIMING: HeaderName = HeaderName::from_static("server-timing");

/// Per-request middleware that installs a [`timing::Recorder`] for the
/// duration of the request and, once the handler returns, emits the
/// collected phase durations plus `total` as a `Server-Timing` response
/// header — e.g. `lock;dur=40.0, gh_get;dur=210.3, gh_put;dur=580.1,
/// total;dur=850.0`.
///
/// The phases are recorded deep in the corpus write/build path (GitHub
/// Contents GET/PUT, per-source lock wait, cold clone/index) via the
/// task-local recorder; see [`regelrecht_corpus::timing`]. Endpoints that
/// touch none of that still get a `total`.
///
/// Same-origin note: the editor SPA is served by this very service, so
/// `Timing-Allow-Origin` is not needed for DevTools to read the header.
/// The phase names leak no secrets and the endpoints already sit behind
/// auth, so the header is always on.
pub async fn server_timing(req: Request, next: Next) -> Response {
    let recorder = timing::Recorder::new();
    let start = Instant::now();
    let mut response = timing::scope(recorder.clone(), next.run(req)).await;
    let header = recorder.server_timing_header(start.elapsed());
    if let Ok(value) = HeaderValue::from_str(&header) {
        response.headers_mut().insert(SERVER_TIMING.clone(), value);
    }
    response
}
