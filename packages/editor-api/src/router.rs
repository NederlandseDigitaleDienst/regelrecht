//! The editor-api's HTTP routes, each group behind its role gate.
//!
//! [`router`] is the one place the routes are put together: the binary serves
//! it, and tests send requests through it, so a test sees the gates production
//! has. What it leaves to the caller is what differs per deployment: the
//! session store (Postgres, or memory for local development), request tracing
//! and the static frontend.
//!
//! A group of routes shares one role gate (`editor-reader`, `editor-writer`,
//! `editor-admin`, `harvester-reader`; Keycloak composite roles make a writer
//! also a reader). Groups whose handlers take `Extension<AccountRecord>` also
//! load the caller's account row; see [`gated`] for the order of the two.

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;

use crate::state::AppState;
use crate::{
    accounts, corpus_handlers, enrich_review, favorites, feature_flags, github_oauth,
    harvest_proxy, health, middleware, task_requests, tasks_api, traject_integrity, traject_runs,
    trajects, user_notes, user_settings,
};

/// Whether a route group's handlers need the caller's account row.
#[derive(Clone, Copy)]
enum Account {
    /// The handlers take `Extension<AccountRecord>`: load it per request.
    Load,
    /// The handlers read the session only.
    Skip,
}

/// Put `routes` behind the `role` gate, and load the caller's account first
/// when the handlers need it.
///
/// The role is checked before the account is loaded: a caller without the
/// role gets 401 or 403 without a database round trip. With `route_layer` the
/// layer added last runs first, hence the order below.
fn gated(
    state: &AppState,
    role: &'static str,
    account: Account,
    routes: Router<AppState>,
) -> Router<AppState> {
    let routes = match account {
        Account::Load => routes.route_layer(from_fn_with_state(
            state.clone(),
            accounts::account_middleware,
        )),
        Account::Skip => routes,
    };
    routes.route_layer(from_fn_with_state(
        state.clone(),
        middleware::require_role::<AppState>(role),
    ))
}

/// The editor-api's routes with their gates, bound to `state`.
///
/// The caller wraps the result in a session layer: the role gates, the
/// account loading and the token refresh all read the session. Without OIDC
/// (`state.config.oidc` unset, local development only) every role gate
/// passes, there is no token to refresh, and the GitHub OAuth routes are not
/// mounted, since linking an account needs a logged-in user.
///
/// # Panics
///
/// When two groups register the same method on the same path: axum refuses
/// to merge overlapping routes. That is a bug in this file, met at startup
/// and in `tests/router_gates_test.rs`.
pub fn router(state: AppState) -> Router {
    let auth_enabled = state.config.is_auth_enabled();

    let mut routes = Router::new()
        .route("/health", get(health::live))
        .route("/health/ready", get(health::ready))
        .merge(regelrecht_auth::auth_routes::<AppState>())
        .merge(public_api_routes())
        .merge(gated(
            &state,
            "editor-reader",
            Account::Skip,
            reader_routes(),
        ))
        .merge(gated(
            &state,
            "editor-writer",
            Account::Skip,
            writer_routes(),
        ))
        .merge(gated(&state, "editor-admin", Account::Skip, admin_routes()))
        .merge(gated(
            &state,
            "harvester-reader",
            Account::Skip,
            harvest_admin_routes(),
        ))
        .merge(gated(
            &state,
            "editor-reader",
            Account::Load,
            traject_reader_routes(),
        ))
        .merge(gated(
            &state,
            "editor-writer",
            Account::Load,
            traject_writer_routes(),
        ))
        .merge(gated(
            &state,
            "editor-reader",
            Account::Load,
            user_notes_reader_routes(),
        ))
        .merge(gated(
            &state,
            "editor-writer",
            Account::Load,
            user_notes_writer_routes(),
        ))
        .merge(gated(
            &state,
            "editor-reader",
            Account::Load,
            tasks_reader_routes(),
        ))
        .merge(gated(
            &state,
            "editor-writer",
            Account::Load,
            tasks_writer_routes(),
        ));
    if auth_enabled {
        routes = routes
            // GitHub user-OAuth link flow (spike). The sealed token cookie is
            // bound to the account, so it sits behind the same gate as traject
            // writes. Unconfigured, the handlers answer 501 themselves.
            .merge(gated(
                &state,
                "editor-writer",
                Account::Load,
                github_oauth::github_oauth_routes(),
            ))
            // The relay is public (no session or account): GitHub redirects
            // the browser here on the fixed callback host and it is sent on to
            // the deployment it came from.
            .merge(github_oauth::github_relay_route());
    }

    let refresh_state = auth_enabled.then(|| state.clone());
    let routes = routes
        .with_state(state)
        // Innermost: wraps the routed handlers most tightly, so the
        // request-scoped timing recorder is installed (and read back into the
        // `Server-Timing` header) around the code that records the phases.
        .layer(axum::middleware::from_fn(middleware::server_timing));
    match refresh_state {
        // Inside the caller's session layer (the session is loaded) and outside
        // the role gates, so a gate sees fresh roles or a dropped auth marker.
        Some(refresh_state) => routes.layer(from_fn_with_state(
            refresh_state,
            middleware::refresh_session_token::<AppState>,
        )),
        None => routes,
    }
}

// Body-size caps for write routes. The 1 MiB scenario cap is generous for
// a single Gherkin file (real-world scenarios are a few KiB) and the 5 MiB
// law cap leaves headroom for federated regulations that can reach a few
// hundred KiB. Documents are markdown/text working notes — 1 MiB matches
// the scenario cap so a paste of a kamerstuk's full text fits without
// accidentally accepting a multi-megabyte binary blob.
const MAX_SCENARIO_BODY: usize = 1024 * 1024;
const MAX_LAW_BODY: usize = 5 * 1024 * 1024;
const MAX_DOCUMENT_BODY: usize = 1024 * 1024;
// Twice the 64 KiB note-value cap (user_notes::MAX_BODY_VALUE_BYTES):
// room for JSON escaping/overhead while still rejecting blobs early.
const MAX_NOTE_BODY: usize = 128 * 1024;
// Uploaded PDF/Word documents (converted to markdown async). Much larger
// than the text caps since it carries a binary; still bounded to reject
// oversized files early (Postgres stores the bytes transiently).
const MAX_UPLOAD_BODY: usize = 25 * 1024 * 1024;

/// Public API routes — accessible without authentication. The corpus
/// reads under `/api/corpus/...` here serve the **global** view (no
/// per-traject overlay). Edits and traject-scoped reads live under
/// `/api/trajects/{tid}/corpus/...` (see `traject_reader_routes` and
/// `traject_writer_routes`).
fn public_api_routes() -> Router<AppState> {
    Router::new()
        .route("/api/sources", get(corpus_handlers::list_sources))
        .route("/api/corpus/laws", get(corpus_handlers::list_corpus_laws))
        .route(
            "/api/corpus/laws/{law_id}",
            get(corpus_handlers::get_corpus_law),
        )
        .route(
            "/api/corpus/laws/{law_id}/versions",
            get(corpus_handlers::get_corpus_law_versions),
        )
        .route(
            "/api/corpus/laws/{law_id}/outputs",
            get(corpus_handlers::list_law_outputs),
        )
        .route(
            "/api/corpus/laws/{law_id}/implementors",
            get(corpus_handlers::list_law_implementors),
        )
        .route(
            "/api/corpus/laws/{law_id}/scenarios",
            get(corpus_handlers::list_scenarios),
        )
        .route(
            "/api/corpus/laws/{law_id}/scenarios/{filename}",
            get(corpus_handlers::get_scenario),
        )
        .route(
            "/api/corpus/laws/{law_id}/annotations",
            get(corpus_handlers::get_annotations),
        )
        .route("/api/feature-flags", get(feature_flags::list_feature_flags))
        // The upload formats that convert without a language model. The
        // editor's upload confirmation reads this, so the frontend keeps no
        // second list of formats; the content is static and not sensitive,
        // hence public.
        .route(
            "/api/document-upload-formats",
            get(corpus_handlers::list_document_upload_formats),
        )
        // Harvest status — forwarded to pipeline-api. Read-only DB lookup,
        // safe to expose unauthenticated. (The search endpoint lives behind
        // auth because it triggers outbound requests to zoekservice.overheid.nl
        // and would otherwise be an amplification vector.)
        .route("/api/harvest/status", get(harvest_proxy::proxy_harvest))
}

/// Reader routes — `editor-reader` covers user-scoped reads (favorites,
/// settings), the user's own Corpus juris favorites (any account may keep
/// them; they touch nothing but the account), and harvest search (search is
/// behind auth because it triggers outbound requests and would otherwise be
/// an amplification vector).
fn reader_routes() -> Router<AppState> {
    Router::new()
        .route("/api/favorites", get(favorites::list))
        .route(
            "/api/favorites/{law_id}",
            axum::routing::put(favorites::add).delete(favorites::remove),
        )
        .route("/api/user/settings", get(user_settings::list))
        .route("/api/harvest/search", get(harvest_proxy::proxy_harvest))
}

/// Writer routes — `editor-writer` covers user-scoped writes. Corpus
/// edits live under `/api/trajects/{tid}/corpus/...` in `traject_writer_routes`
/// (membership + traject path on every request, making "no traject = no
/// write" an URL-level invariant rather than a runtime check). Harvest
/// POSTs (which trigger outbound requests) sit here so anonymous callers
/// can never reach them.
fn writer_routes() -> Router<AppState> {
    Router::new()
        // 4 KiB is ample for `{"value":"<one allowed enum>"}` and rejects
        // oversized bodies before deserialization.
        .route(
            "/api/user/settings/{key}",
            axum::routing::put(user_settings::set)
                .layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/harvest",
            axum::routing::post(harvest_proxy::proxy_harvest),
        )
        .route(
            "/api/harvest/batch",
            axum::routing::post(harvest_proxy::proxy_harvest),
        )
}

/// Personal notes: private per-user notes on a law (Postgres,
/// never git). Handlers extract `Extension<AccountRecord>`, so
/// `account_middleware` must run per request, same as traject routes.
/// Reads at reader tier, mutations at writer tier, mirroring how
/// settings split across `reader_routes`/`writer_routes`
/// (which stay Session-based and therefore don't carry the account
/// middleware).
fn user_notes_reader_routes() -> Router<AppState> {
    Router::new().route("/api/user/notes/{law_id}", get(user_notes::list))
}

fn user_notes_writer_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/user/notes/{law_id}",
            axum::routing::post(user_notes::create)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_NOTE_BODY)),
        )
        .route(
            "/api/user/notes/{law_id}/{note_id}",
            axum::routing::put(user_notes::update)
                .delete(user_notes::remove)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_NOTE_BODY)),
        )
}

/// Personal tasks: review tasks of async jobs, tied to the account that
/// asked for the job. The same split as the user notes: reads at reader
/// tier, resolving one (the change plus its audit stamp) at writer tier,
/// both behind `account_middleware` because the handlers extract
/// `Extension<AccountRecord>`.
fn tasks_reader_routes() -> Router<AppState> {
    Router::new()
        .route("/api/tasks", get(tasks_api::list))
        .route("/api/tasks/{task_id}", get(tasks_api::detail))
        .route("/api/tasks/jobs/{job_id}", get(enrich_review::job_tasks))
}

fn tasks_writer_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/tasks/{task_id}/resolve",
            axum::routing::post(tasks_api::resolve),
        )
        .route(
            "/api/tasks/jobs/{job_id}/apply",
            axum::routing::post(enrich_review::apply),
        )
}

/// Admin routes — `editor-admin` covers destructive/global operations
/// (full corpus reload, feature flag toggles).
fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/corpus/reload",
            axum::routing::post(corpus_handlers::reload_corpus),
        )
        .route(
            "/api/feature-flags/{key}",
            axum::routing::put(feature_flags::update_feature_flag),
        )
}

/// Harvester-admin proxy — the merged "Beheer" section in the editor reaches
/// the standalone harvester-admin API through here. A single wildcard route
/// forwards every method (GET reads, POST enqueue, DELETE) to that service,
/// rewriting `/api/harvest-admin/<x>` → `/api/<x>` and forwarding the
/// session cookie. Gated on `harvester-reader` as defence-in-depth; the
/// harvester-admin service is the real enforcer for writer/admin actions
/// (writes stay OIDC-only there — editor-api never mints a service token).
fn harvest_admin_routes() -> Router<AppState> {
    Router::new().route(
        "/api/harvest-admin/{*rest}",
        axum::routing::any(harvest_proxy::proxy_harvest_admin),
    )
}

/// Traject-scoped reads (editor-reader tier): the trajects you are a member
/// of and their corpus. Reading needs no write privilege: a Keycloak
/// `editor-reader` invited to a traject must see its edits in progress.
/// Handlers extract `Extension<AccountRecord>`, so the account row is loaded
/// per request, and each handler re-checks traject membership; the role here
/// is the broader "an authenticated reader at all" gate. Writes are in
/// `traject_writer_routes`.
///
/// The corpus routes sit under the traject because a traject is per tab (the
/// SPA route names it), not a server session: the traject id is in every URL,
/// and every `/api/trajects/{id}/corpus/...` route goes through the same
/// membership check.
fn traject_reader_routes() -> Router<AppState> {
    Router::new()
        .route("/api/trajects", get(trajects::list))
        .route("/api/trajects/{id}", get(trajects::get))
        .route(
            "/api/trajects/{traject_ref}/sources",
            get(corpus_handlers::list_traject_sources),
        )
        // The traject's runs (harvests, enrichments, conversions) for the
        // Verwerkingen overview: a read, open to every member.
        .route("/api/trajects/{traject_ref}/runs", get(traject_runs::list))
        .route(
            "/api/trajects/{traject_ref}/corpus/laws",
            get(corpus_handlers::list_traject_corpus_laws),
        )
        .route(
            "/api/trajects/{traject_ref}/favorites",
            get(favorites::list_traject),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/changed-laws",
            get(corpus_handlers::list_traject_changed_laws),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}",
            get(corpus_handlers::get_traject_corpus_law),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/versions",
            get(corpus_handlers::get_traject_corpus_law_versions),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/outputs",
            get(corpus_handlers::list_traject_law_outputs),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/implementors",
            get(corpus_handlers::list_traject_law_implementors),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios",
            get(corpus_handlers::list_traject_scenarios),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios/{filename}",
            get(corpus_handlers::get_traject_scenario),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/annotations",
            get(corpus_handlers::get_traject_annotations),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/documents",
            get(corpus_handlers::list_traject_documents),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/documents/jobs",
            get(corpus_handlers::list_traject_document_convert_jobs),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/documents/jobs/{job_id}",
            axum::routing::delete(corpus_handlers::cancel_traject_document_convert_job),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/documents/{*doc_path}",
            get(corpus_handlers::get_traject_document),
        )
        // Integrity report over the traject's own corpus repo. A read, so
        // reader-tier like its neighbours: seeing what is wrong with a
        // traject you are a member of needs no write privilege.
        .route(
            "/api/trajects/{traject_ref}/integrity",
            get(traject_integrity::get_traject_integrity),
        )
}

/// Traject-scoped writes (editor-writer tier). Mutations to traject
/// metadata + membership AND the traject-branch corpus content all
/// require write privilege. Axum merges the reader and writer
/// routers at the same path by method: a PUT against
/// `/corpus/laws/{lid}/scenarios/{fn}` resolves to the writer
/// handler, while a GET on the same path resolves to the reader
/// handler above.
fn traject_writer_routes() -> Router<AppState> {
    Router::new()
        .route("/api/trajects", axum::routing::post(trajects::create))
        .route(
            "/api/trajects/{id}",
            axum::routing::patch(trajects::update).delete(trajects::delete),
        )
        .route(
            "/api/trajects/{id}/repo",
            axum::routing::put(trajects::move_repo),
        )
        .route(
            "/api/trajects/{traject_ref}/favorites/{law_id}",
            axum::routing::put(favorites::add_traject).delete(favorites::remove_traject),
        )
        .route(
            "/api/trajects/{id}/leave",
            axum::routing::post(trajects::leave),
        )
        .route(
            "/api/trajects/{id}/members",
            axum::routing::post(trajects::add_member),
        )
        .route(
            "/api/trajects/{id}/members/{account_id}",
            axum::routing::patch(trajects::update_member).delete(trajects::remove_member),
        )
        .route(
            "/api/trajects/{id}/invites/{email}",
            axum::routing::delete(trajects::remove_invite),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios/{filename}",
            axum::routing::put(corpus_handlers::save_scenario)
                .delete(corpus_handlers::delete_scenario)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_SCENARIO_BODY)),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/annotations",
            axum::routing::put(corpus_handlers::save_annotations)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_SCENARIO_BODY)),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}",
            axum::routing::put(corpus_handlers::save_law)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_LAW_BODY)),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/documents/upload",
            axum::routing::post(corpus_handlers::upload_traject_document)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_UPLOAD_BODY)),
        )
        .route(
            // Static segment wins over `{law_id}` in the router, so this
            // does not shadow the per-law routes.
            "/api/trajects/{traject_ref}/corpus/laws/upload",
            axum::routing::post(corpus_handlers::upload_traject_law)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_UPLOAD_BODY)),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws",
            axum::routing::post(corpus_handlers::create_traject_law)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_LAW_BODY)),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/enrich",
            axum::routing::post(task_requests::request_enrich),
        )
        .route(
            // Promote: copy a law from the central corpus into the traject's
            // repository (the same write path and authorization as save_law).
            "/api/trajects/{traject_ref}/corpus/laws/{law_id}/promote",
            axum::routing::post(corpus_handlers::promote_corpus_law),
        )
        .route(
            // Traject-scoped harvest through the task mechanism: the BWB
            // download on the worker plus a chained enrich task (the
            // law_convert pattern).
            "/api/trajects/{traject_ref}/corpus/harvest",
            axum::routing::post(task_requests::request_traject_harvest),
        )
        .route(
            "/api/trajects/{traject_ref}/corpus/documents/{*doc_path}",
            axum::routing::put(corpus_handlers::save_traject_document)
                .delete(corpus_handlers::delete_traject_document)
                .layer(axum::extract::DefaultBodyLimit::max(MAX_DOCUMENT_BODY)),
        )
}
