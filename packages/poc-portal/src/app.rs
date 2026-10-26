//! The router: index, gate, static PoCs, proxied PoCs.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use tower::ServiceExt as _;
use tower_http::services::{ServeDir, ServeFile};

use crate::bron::{Bron, BronFout};
use crate::config::{upstream_url, Config};
use crate::gate;
use crate::pagina;
use crate::proxy;
use crate::registry::{Poc, Soort};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    /// Resolved upstream base URL per proxied slug.
    pub upstreams: Arc<HashMap<String, String>>,
    /// Base URL of the beleidsassistent per slug that has one.
    pub assistenten: Arc<HashMap<String, String>>,
    /// The sources per PoC, by `(slug, name)` ([`crate::bron`]).
    pub bronnen: Arc<HashMap<(String, String), Bron>>,
    /// Fetches a source's tarball; `None` if the client could not be built,
    /// and then every source on GitHub answers 502.
    pub github: Option<Arc<regelrecht_github::GithubClient>>,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let mut upstreams = HashMap::new();
        for poc in &config.registry.pocs {
            if poc.soort != Soort::Proxy {
                continue;
            }
            let env = format!("POC_UPSTREAM_{}", poc.slug.to_uppercase().replace('-', "_"));
            match upstream_url(config.deployment.as_deref(), poc, std::env::var(env).ok()) {
                Some(url) => {
                    tracing::info!(slug = %poc.slug, url = %url, "proxied PoC");
                    upstreams.insert(poc.slug.clone(), url);
                }
                None => tracing::warn!(
                    slug = %poc.slug,
                    "no upstream for this PoC — it will answer 503 until HOSTNAME names a \
                     deployment or POC_UPSTREAM_* is set"
                ),
            }
        }
        // De assistent draait als eigen proces per casus, op een poort die het
        // image zelf bepaalt (POC_ASSISTENT_<SLUG>). Staat hij er niet, dan
        // antwoordt /api met 503 en verbergt de app zijn assistent-paneel via
        // de health-probe die hij toch al doet.
        let mut assistenten = HashMap::new();
        for poc in &config.registry.pocs {
            if !poc.assistent {
                continue;
            }
            let env = format!(
                "POC_ASSISTENT_{}",
                poc.slug.to_uppercase().replace('-', "_")
            );
            match std::env::var(&env) {
                Ok(url) if !url.trim().is_empty() => {
                    tracing::info!(slug = %poc.slug, url = %url, "beleidsassistent");
                    assistenten.insert(poc.slug.clone(), url);
                }
                _ => tracing::info!(
                    slug = %poc.slug,
                    "geen beleidsassistent ({env} niet gezet); /api antwoordt 503"
                ),
            }
        }

        // Where each source comes from is read once, here: a local folder
        // (POC_BRON_<NAME>), the branch with its token, or neither.
        let mut bronnen = HashMap::new();
        for poc in &config.registry.pocs {
            for source in &poc.sources {
                bronnen.insert(
                    (poc.slug.clone(), source.name.clone()),
                    Bron::new(source, |k| std::env::var(k).ok()),
                );
            }
        }
        let github = match regelrecht_github::GithubClient::new() {
            Ok(c) => Some(Arc::new(c)),
            Err(e) => {
                tracing::error!(error = %e, "no GitHub client; sources on GitHub answer 502");
                None
            }
        };

        Self {
            config: Arc::new(config),
            upstreams: Arc::new(upstreams),
            assistenten: Arc::new(assistenten),
            bronnen: Arc::new(bronnen),
            github,
            http: reqwest::Client::new(),
        }
    }
}

/// Build the portal's router.
pub fn router(state: AppState) -> Router {
    let static_root = state.config.static_root.clone();

    Router::new()
        .route("/", get(index))
        .route("/health", get(|| async { "OK" }))
        .route("/_toegang/{slug}", post(toegang))
        .nest_service("/_assets", ServeDir::new(format!("{static_root}/_assets")))
        .fallback(
            get(poc_request)
                .post(poc_request)
                .put(poc_request)
                .patch(poc_request)
                .delete(poc_request),
        )
        .layer(axum::middleware::from_fn(
            regelrecht_auth::security_headers::security_headers(
                regelrecht_auth::security_headers::POC_CSP,
            ),
        ))
        .with_state(state)
}

async fn index(State(state): State<AppState>) -> Html<String> {
    Html(pagina::index(&state.config.registry))
}

/// Split `/napp/aanvrager/x` into `("napp", "/aanvrager/x")`.
fn split_slug(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix('/')?;
    match rest.find('/') {
        Some(i) => Some((&rest[..i], &rest[i..])),
        None if rest.is_empty() => None,
        None => Some((rest, "/")),
    }
}

/// Everything that is not the index, health, the login POST or the portal's own
/// assets belongs to a PoC — if the visitor is allowed in.
async fn poc_request(State(state): State<AppState>, request: Request) -> Response {
    let path = request.uri().path().to_string();
    let Some((slug, rest)) = split_slug(&path) else {
        return not_found();
    };
    let Some(poc) = state.config.registry.get(slug) else {
        return not_found();
    };

    if !heeft_toegang(&state, poc, &request) {
        // 401 with the login page in the body, not a redirect: the address the
        // visitor typed stays in the bar, so signing in continues to where
        // they were going. The same shape the platform's own authorization
        // wall uses.
        return (
            StatusCode::UNAUTHORIZED,
            Html(pagina::inloggen(poc, &path, false)),
        )
            .into_response();
    }

    // Een bron van deze poc (`sources` in het register), achter hetzelfde
    // wachtwoord: het portaal haalt haar op met het token, de browser ziet
    // dat nooit.
    // Alleen een bron uit het register; elk ander pad blijft van de poc.
    let bron = rest
        .strip_prefix("/bron/")
        .map(|naam| (slug.to_string(), naam.trim_end_matches('/').to_string()))
        .and_then(|sleutel| state.bronnen.get(&sleutel));
    if let Some(bron) = bron {
        return serveer_bron(&state, bron).await;
    }

    // De beleidsassistent draait als eigen proces naast dit portaal (hij spawnt
    // de Claude CLI, wat niet in een request-handler thuishoort). Alleen `/api`
    // van een poc die hem aan heeft staan gaat daarheen; de rest van de poc
    // wordt gewoon geserveerd.
    if poc.assistent && rest.starts_with("/api/") {
        return match state.assistenten.get(slug) {
            Some(base) => {
                // Het pad zoals de assistent het kent: zonder de slug ervoor.
                let mut parts = request.into_parts();
                let query = parts
                    .0
                    .uri
                    .query()
                    .map(|q| format!("?{q}"))
                    .unwrap_or_default();
                parts.0.uri = format!("{rest}{query}")
                    .parse::<Uri>()
                    .unwrap_or_else(|_| Uri::from_static("/"));
                proxy::forward(&state.http, base, Request::from_parts(parts.0, parts.1)).await
            }
            None => (
                StatusCode::SERVICE_UNAVAILABLE,
                "De beleidsassistent draait niet in deze omgeving.",
            )
                .into_response(),
        };
    }

    match poc.soort {
        Soort::Proxy => match state.upstreams.get(slug) {
            Some(base) => {
                let response = proxy::forward(&state.http, base, request).await;
                injecteer_strip(response, poc).await
            }
            None => (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("PoC {slug} is not reachable: no upstream configured"),
            )
                .into_response(),
        },
        Soort::Statisch => serve_static(&state.config.static_root, poc, rest, request).await,
    }
}

fn heeft_toegang(state: &AppState, poc: &Poc, request: &Request) -> bool {
    let naam = gate::cookie_naam(&poc.slug);
    let Some(cookies) = request.headers().get(header::COOKIE) else {
        return false;
    };
    let Ok(cookies) = cookies.to_str() else {
        return false;
    };
    cookies
        .split(';')
        .filter_map(|c| c.trim().split_once('='))
        .any(|(k, v)| {
            k == naam
                && gate::cookie_is_geldig(
                    &state.config.sleutel,
                    &poc.slug,
                    v,
                    time::OffsetDateTime::now_utc(),
                )
        })
}

/// A source as JSON, never cached by the browser: the portal holds the copy
/// ([`crate::bron::Cache`]). Not configured here is 503, a failed fetch
/// without a copy 502, each with the reason as text.
async fn serveer_bron(state: &AppState, bron: &Bron) -> Response {
    let uitkomst = bron.geef(state.github.as_deref()).await;
    let no_store = [(header::CACHE_CONTROL, "no-store")];
    match uitkomst {
        Ok(o) => (no_store, axum::Json(&*o)).into_response(),
        Err(e) => {
            let status = match e {
                BronFout::NietBeschikbaar(_) => StatusCode::SERVICE_UNAVAILABLE,
                BronFout::Ophalen(_) => StatusCode::BAD_GATEWAY,
            };
            tracing::warn!(source = %bron.naam, error = %e, "source not served");
            (status, no_store, e.to_string()).into_response()
        }
    }
}

/// Handle the password form.
async fn toegang(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    body: String,
) -> Response {
    let Some(poc) = state.config.registry.get(&slug) else {
        return not_found();
    };
    let velden = form_urlencoded_parse(&body);
    let ingevoerd = velden.get("wachtwoord").map(String::as_str).unwrap_or("");
    // Only ever a path on this host: an open redirect would turn the login
    // screen into a way to send someone somewhere else.
    let verder = velden
        .get("verder")
        .map(String::as_str)
        .filter(|p| p.starts_with(&format!("/{slug}/")) || **p == format!("/{slug}"))
        .unwrap_or("/")
        .to_string();

    let Some(verwacht) = state.config.wachtwoorden.get(&slug) else {
        // Startup refuses a PoC without a password, so this is unreachable;
        // answering "no" is the safe reading if it ever is reached.
        return not_found();
    };

    if !gate::wachtwoord_klopt(verwacht, ingevoerd) {
        tracing::warn!(slug = %slug, "wrong password");
        return (
            StatusCode::UNAUTHORIZED,
            Html(pagina::inloggen(poc, &verder, true)),
        )
            .into_response();
    }

    let waarde = gate::maak_cookie(
        &state.config.sleutel,
        &slug,
        time::OffsetDateTime::now_utc(),
    );
    // `Path=/{slug}` zonder sluitende schuine streep, en dat is geen slordigheid.
    //
    // Een browser stuurt een cookie met `Path=/napp/` wel naar `/napp/iets`,
    // maar niet naar `/napp` zelf: de sluitende streep maakt het pad strikter.
    // Napp stuurt `/napp/` permanent door naar `/napp` (zie main.rs, `nest`
    // routeert de lege rest niet), en precies op dat adres viel het cookie dan
    // weg. De poort zag geen cookie, toonde het formulier opnieuw, en met een
    // goed wachtwoord bleef je in een kringetje lopen -- terwijl een fout
    // wachtwoord wél netjes "onjuist" gaf, want dat pad komt nooit bij de
    // omleiding.
    //
    // `Path=/napp` dekt volgens de RFC zowel `/napp` als alles eronder, dus dit
    // is ruimer waar het moet en niet ruimer dan de poc zelf.
    let cookie = format!(
        "{naam}={waarde}; Path=/{slug}; Max-Age={max_age}; HttpOnly; Secure; SameSite=Lax",
        naam = gate::cookie_naam(&slug),
        max_age = gate::GELDIGHEID.whole_seconds(),
    );

    let mut response = Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, &verder)
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, v);
    }
    response
}

/// Minimal `application/x-www-form-urlencoded` parsing.
fn form_urlencoded_parse(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (percent_decode(k), percent_decode(v)))
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Serve a static PoC from `{static_root}/{slug}`, with its own SPA fallback.
async fn serve_static(static_root: &str, poc: &Poc, rest: &str, request: Request) -> Response {
    let slug = &poc.slug;
    let dir = format!("{static_root}/{slug}");
    let index = ServeFile::new(format!("{dir}/index.html"));
    let files = ServeDir::new(&dir).not_found_service(index);

    // The PoC is built with `base: /<slug>/`, so its own asset URLs already
    // carry the prefix. Strip it here so the file lookup is relative to the
    // PoC's own dist root.
    let mut parts = request.into_parts();
    let query = parts
        .0
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    parts.0.uri = format!("{rest}{query}")
        .parse::<Uri>()
        .unwrap_or_else(|_| Uri::from_static("/"));
    let request = Request::from_parts(parts.0, parts.1);

    let response = files
        .oneshot(request)
        .await
        .map(|r| r.map(Body::new))
        .unwrap_or_else(|e| match e {});

    injecteer_strip(response, poc).await
}

/// Splice the portal's notice into an HTML response from a PoC.
///
/// Only HTML: a JSON or JavaScript body with a `<div>` prepended is a corrupted
/// file, and the check is on the response's own `content-type` rather than the
/// path, because a SPA fallback serves `index.html` under any URL.
///
/// A body that is not valid UTF-8, or has no `<body...>`, is passed through
/// untouched. Failing to add the notice is bad; corrupting the page the notice
/// is about would be worse.
///
/// A PoC with `strook: false` in the register gets no strip; its status and
/// voorbehoud still show on the index and the login screen.
async fn injecteer_strip(response: Response, poc: &Poc) -> Response {
    // A PoC whose register entry turns the strip off (`strook: false`).
    if !poc.strook {
        return response;
    }
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/html"));
    if !is_html {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MAX_HTML).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(html) = std::str::from_utf8(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };

    // After the opening <body ...>, so the strip is the first thing in the
    // document rather than a sibling of <html>.
    let Some(open) = html.find("<body") else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let Some(rel) = html[open..].find('>') else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let split = open + rel + 1;

    let mut out = String::with_capacity(html.len() + 512);
    out.push_str(&html[..split]);
    out.push_str(&pagina::voorbehoud_strip(poc));
    out.push_str(&html[split..]);

    // The body grew, and a stale content-length truncates the page.
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(out))
}

/// Cap on an HTML page the portal rewrites. A PoC's index is a few hundred
/// kilobytes at most; anything larger is not a document to splice a notice into.
const MAX_HTML: usize = 8 * 1024 * 1024;

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Niet gevonden").into_response()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_path_splits_into_slug_and_remainder() {
        assert_eq!(
            split_slug("/napp/aanvrager/x"),
            Some(("napp", "/aanvrager/x"))
        );
        assert_eq!(split_slug("/napp/"), Some(("napp", "/")));
        assert_eq!(split_slug("/napp"), Some(("napp", "/")));
        assert_eq!(split_slug("/"), None);
    }

    #[test]
    fn a_form_body_is_parsed_and_decoded() {
        let f = form_urlencoded_parse("wachtwoord=hunter%402&verder=%2Fnapp%2F");
        assert_eq!(f.get("wachtwoord").map(String::as_str), Some("hunter@2"));
        assert_eq!(f.get("verder").map(String::as_str), Some("/napp/"));
    }

    #[test]
    fn a_plus_in_a_password_is_a_space() {
        let f = form_urlencoded_parse("wachtwoord=a+b");
        assert_eq!(f.get("wachtwoord").map(String::as_str), Some("a b"));
    }

    fn poc() -> Poc {
        crate::registry::Registry::from_yaml(crate::REGISTRY_YAML)
            .expect("registry")
            .get("napp")
            .expect("napp")
            .clone()
    }

    fn antwoord(content_type: &str, body: &'static str) -> Response {
        Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .expect("response")
    }

    async fn tekst(response: Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .expect("body");
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[tokio::test]
    async fn the_notice_lands_just_inside_the_body() {
        let html = antwoord(
            "text/html",
            "<html><head></head><body class=x><h1>Hoi</h1></body></html>",
        );
        let out = tekst(injecteer_strip(html, &poc()).await).await;
        let strip = out.find("data-poc-portaal").expect("strip present");
        let body = out.find("<body").expect("body");
        let h1 = out.find("<h1>").expect("h1");
        assert!(body < strip && strip < h1, "{out}");
    }

    #[tokio::test]
    async fn only_html_is_rewritten() {
        // A JSON or JS body with a <div> prepended is a corrupted file, and the
        // PoCs fetch their laws and their WASM glue over exactly those types.
        for ct in ["application/json", "text/javascript", "application/wasm"] {
            let out = tekst(injecteer_strip(antwoord(ct, "{\"a\":1}"), &poc()).await).await;
            assert_eq!(out, "{\"a\":1}", "{ct} must pass through untouched");
        }
    }

    #[tokio::test]
    async fn html_without_a_body_tag_is_left_alone() {
        let out =
            tekst(injecteer_strip(antwoord("text/html", "<p>fragment</p>"), &poc()).await).await;
        assert_eq!(out, "<p>fragment</p>");
    }

    #[tokio::test]
    async fn the_stale_content_length_is_dropped() {
        // The body grows; leaving the original length truncates the page.
        let response = Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CONTENT_LENGTH, "13")
            .body(Body::from("<body>Hoi</body>"))
            .expect("response");
        let out = injecteer_strip(response, &poc()).await;
        assert!(out.headers().get(header::CONTENT_LENGTH).is_none());
    }

    #[tokio::test]
    async fn a_poc_without_a_strip_is_left_alone() {
        let mut p = poc();
        assert!(p.strook, "the strip is on by default");
        p.strook = false;
        let page = "<html><body><h1>Hoi</h1></body></html>";
        let out = tekst(injecteer_strip(antwoord("text/html", page), &p).await).await;
        assert_eq!(out, page);
    }

    #[test]
    fn the_register_turns_the_strip_off_for_chronolexografie_only() {
        let registry =
            crate::registry::Registry::from_yaml(crate::REGISTRY_YAML).expect("registry");
        let without: Vec<&str> = registry
            .pocs
            .iter()
            .filter(|p| !p.strook)
            .map(|p| p.slug.as_str())
            .collect();
        assert_eq!(without, ["chronolexografie"]);
    }

    #[tokio::test]
    async fn the_notice_names_the_status_and_the_voorbehoud() {
        let p = poc();
        let out = tekst(injecteer_strip(antwoord("text/html", "<body></body>"), &p).await).await;
        assert!(out.contains(p.status.label()), "{out}");
        assert!(out.contains("wetsvoorstel"), "{out}");
    }
}
