//! `serve` — the local architecture-explorer server.
//!
//! A tiny Axum server (the same stack `editor-api` uses) that:
//!   - `GET /api/model` regenerates the architecture model **on-demand** from
//!     the working tree and returns it as JSON, cached on the newest source
//!     mtime so an unchanged tree does not pay the ~2 s extraction again;
//!   - serves the built explorer UI (Vite/Vue) on `/`.
//!
//! It binds `0.0.0.0` so it is reachable from the host in the containerised dev
//! setup (port range 7100–7300). Nothing here is deployed — it is a local
//! developer tool, started via `just arch-explore`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::services::{ServeDir, ServeFile};

use crate::build::{self, DeepScope};
use crate::prose;

/// Fixed default port inside the container's forwarded 7100–7300 range.
/// Override with `--port` or `ARCH_EXPLORE_PORT`.
const DEFAULT_PORT: u16 = 7180;

/// Parsed `serve` arguments.
struct ServeArgs {
    port: u16,
    manifest_path: Option<PathBuf>,
    /// Directory of built UI assets to serve on `/`. Defaults to
    /// `<repo>/packages/arch-extract/ui/dist`.
    ui_dir: Option<PathBuf>,
}

/// The last generated model, tagged with the source mtime it was built from.
struct Cached {
    mtime: Option<SystemTime>,
    json: Arc<str>,
}

struct AppState {
    manifest_path: Option<PathBuf>,
    repo_root: PathBuf,
    cache: Mutex<Option<Cached>>,
}

fn parse_args(args: &[String]) -> Result<ServeArgs, String> {
    let mut port = None;
    let mut manifest_path = None;
    let mut ui_dir = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--port" => {
                let raw = it.next().ok_or("--port needs a value")?;
                port = Some(raw.parse().map_err(|_| format!("invalid --port: {raw}"))?);
            }
            "--manifest-path" => {
                manifest_path = Some(PathBuf::from(
                    it.next().ok_or("--manifest-path needs a value")?,
                ));
            }
            "--ui-dir" => {
                ui_dir = Some(PathBuf::from(it.next().ok_or("--ui-dir needs a value")?));
            }
            "-h" | "--help" => {
                println!(
                    "arch-extract serve [--port <n>] [--manifest-path <p>] [--ui-dir <dir>]\n\n\
                     Serves the on-demand architecture model at /api/model and the explorer UI at /.\n\
                     Port also reads ARCH_EXPLORE_PORT; UI dir also reads ARCH_EXPLORE_UI_DIR."
                );
                std::process::exit(0);
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }

    let port = port
        .or_else(|| {
            std::env::var("ARCH_EXPLORE_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(DEFAULT_PORT);
    let ui_dir = ui_dir.or_else(|| std::env::var("ARCH_EXPLORE_UI_DIR").ok().map(PathBuf::from));

    Ok(ServeArgs {
        port,
        manifest_path,
        ui_dir,
    })
}

/// Entry point for the `serve` subcommand.
pub fn run(args: &[String]) -> ExitCode {
    let args = match parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("arch-extract serve: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Discover the repo root once (cheap: metadata only, no deep pass). This
    // fixes both the source tree we watch for changes and the default UI dir.
    let repo_root = match build::repo_root(args.manifest_path.as_deref()) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("arch-extract serve: could not resolve workspace: {e}");
            return ExitCode::FAILURE;
        }
    };

    let ui_dir = args
        .ui_dir
        .unwrap_or_else(|| repo_root.join("packages/arch-extract/ui/dist"));
    if !ui_dir.join("index.html").exists() {
        eprintln!(
            "arch-extract serve: warning: no built UI at {} — run `just arch-explore` (it builds the UI first), or build it with `npm --prefix packages/arch-extract/ui run build`.",
            ui_dir.display()
        );
    }

    let state = Arc::new(AppState {
        manifest_path: args.manifest_path,
        repo_root,
        cache: Mutex::new(None),
    });

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("arch-extract serve: failed to start async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    runtime.block_on(async move { serve(state, ui_dir, args.port).await })
}

async fn serve(state: Arc<AppState>, ui_dir: PathBuf, port: u16) -> ExitCode {
    let index = ui_dir.join("index.html");
    // SPA-style fallback: unknown paths serve index.html so the UI's client
    // routing (and a bare `/`) work; real asset requests are served directly.
    let assets = ServeDir::new(&ui_dir).not_found_service(ServeFile::new(index));

    let app = Router::new()
        .route("/api/model", get(model_handler))
        .route("/api/prose", get(prose_handler))
        .with_state(state)
        .fallback_service(assets);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("arch-extract serve: failed to bind {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };

    eprintln!("arch-explore: listening on http://0.0.0.0:{port}  (open http://localhost:{port})");
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("arch-extract serve: server error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `GET /api/model` — returns the model JSON, regenerating it only when a
/// `packages/**/src` file changed since the cached build.
async fn model_handler(State(state): State<Arc<AppState>>) -> Response {
    let latest = build::latest_src_mtime(&state.repo_root);

    // Fast path: serve from cache when the source tree is unchanged. The guard
    // is dropped before any await so it never crosses a suspension point.
    if let Some(json) = cached_json(&state, latest) {
        return json_response(json);
    }

    // Do the whole extraction (the ~2 s cost) on a blocking thread. The
    // `Box<dyn Error>` from `build_model` is not `Send`, so we flatten it to a
    // `String` inside the closure — that keeps the task's return type `Send`.
    let manifest = state.manifest_path.clone();
    let build_result = tokio::task::spawn_blocking(move || {
        let (model, _root) =
            build::build_model(manifest.as_deref(), &DeepScope::All).map_err(|e| e.to_string())?;
        model.to_json().map_err(|e| e.to_string())
    })
    .await;

    let json: Arc<str> = match build_result {
        Ok(Ok(text)) => Arc::from(text.as_str()),
        Ok(Err(msg)) => return server_error(format!("generating model: {msg}")),
        Err(e) => return server_error(format!("extraction task panicked: {e}")),
    };

    store_cache(&state, latest, json.clone());
    json_response(json)
}

/// `GET /api/prose` — the prose sidecar as a `{ node-id: markdown }` map, read
/// fresh from `packages/arch-extract/prose` on each request (a small directory,
/// so no caching is needed). Only entries with actual narrative are returned, so
/// the explorer overlays prose only where it exists. A missing directory yields
/// an empty object rather than an error.
async fn prose_handler(State(state): State<Arc<AppState>>) -> Response {
    let dir = state.repo_root.join(prose::PROSE_DIR);
    let entries = match prose::load_prose(&dir) {
        Ok(e) => e,
        Err(e) => return server_error(format!("loading prose: {e}")),
    };
    let map: std::collections::BTreeMap<&str, &str> = entries
        .values()
        .filter(|e| e.has_prose())
        .map(|e| (e.node.as_str(), e.body.as_str()))
        .collect();
    match serde_json::to_string(&map) {
        Ok(json) => json_response(Arc::from(json.as_str())),
        Err(e) => server_error(format!("serializing prose: {e}")),
    }
}

/// Returns the cached JSON when its mtime matches `latest`. Poisoned-lock safe
/// (recovers the inner value rather than panicking).
fn cached_json(state: &AppState, latest: Option<SystemTime>) -> Option<Arc<str>> {
    let guard = state.cache.lock().unwrap_or_else(PoisonError::into_inner);
    guard
        .as_ref()
        .filter(|c| c.mtime == latest)
        .map(|c| c.json.clone())
}

fn store_cache(state: &AppState, mtime: Option<SystemTime>, json: Arc<str>) {
    let mut guard = state.cache.lock().unwrap_or_else(PoisonError::into_inner);
    *guard = Some(Cached { mtime, json });
}

fn json_response(json: Arc<str>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        json.as_ref().to_owned(),
    )
        .into_response()
}

fn server_error(message: String) -> Response {
    eprintln!("arch-extract serve: {message}");
    (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
}
