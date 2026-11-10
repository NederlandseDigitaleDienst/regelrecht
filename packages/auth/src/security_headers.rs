//! The single definition of the HTTP security headers every RegelRecht
//! service sends.
//!
//! Two kinds of server answer on a `*.regelrecht.rijks.app` host: the Axum
//! binaries in this workspace (editor-api, harvester-admin) and the nginx
//! images that serve the pre-built sites (docs, lawmaking). Those cannot
//! share code, so they share values instead: the constants below are the
//! source, `deploy/nginx/security-headers.conf` is the nginx transcription,
//! and `script/security-headers.test.mjs` fails the build when the two drift
//! apart. Grafana is the third kind — it renders its own headers from
//! `GF_SECURITY_*` and is configured in `packages/grafana/Dockerfile`.
//!
//! Everything except the CSP is identical everywhere. The CSP is not: it
//! describes what one particular document is allowed to load, so it is
//! chosen per application from the constants below.

use std::future::Future;
use std::pin::Pin;

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;

/// `preload` keeps the domain eligible for the browser preload list, which
/// only accepts it with `includeSubDomains` and a year or more.
pub const STRICT_TRANSPORT_SECURITY: &str = "max-age=31536000; includeSubDomains; preload";

pub const X_CONTENT_TYPE_OPTIONS: &str = "nosniff";

pub const X_FRAME_OPTIONS: &str = "DENY";

pub const REFERRER_POLICY: &str = "strict-origin-when-cross-origin";

pub const PERMISSIONS_POLICY: &str = "geolocation=(), camera=(), microphone=()";

/// CSP for a service that only ever answers JSON and redirects.
///
/// `default-src 'none'` is the correct policy for a document that is never
/// rendered: nothing may be loaded because nothing is meant to be. The three
/// directives after it do not fall back to `default-src` and have to be
/// spelled out.
pub const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// CSP for the editor SPA, which `editor-api` serves itself.
///
/// Two allowances are load-bearing and neither is `unsafe-eval`:
///
/// * `'wasm-unsafe-eval'` — the law engine is a WebAssembly module, and
///   instantiating one counts as compiling code. This grants exactly that
///   and nothing else; `'unsafe-eval'` would additionally hand out
///   `eval()` and `new Function()`.
/// * `blob:` in `script-src` — `useEngine.js` fetches the wasm-bindgen glue
///   from this origin, wraps it in a `Blob` and `import()`s the resulting
///   URL. A blob URL inherits the origin that created it, so the code still
///   comes from us; without the scheme the import is blocked.
///
/// `style-src 'unsafe-inline'` is unavoidable: the NDD web components build
/// a `<style>` element and assign its `textContent`, and several templates
/// carry a `style="…"` attribute. `script-src` gets no such allowance —
/// the built bundle has no inline script, and Vue runs runtime-only, so
/// nothing needs `'unsafe-eval'` either.
///
/// `worker-src 'self'` is spelled out because it would otherwise fall back
/// to `script-src` (not to `default-src`) and inherit the `blob:` above.
/// Nothing in the bundle starts a worker, so the blob allowance stays
/// confined to the one import that needs it.
///
/// GitHub and wetten.overheid.nl are absent from `connect-src`: that traffic
/// runs through `editor-api`, so the browser only ever talks to its own
/// origin.
pub const EDITOR_CSP: &str = "default-src 'self'; \
     script-src 'self' 'wasm-unsafe-eval' blob:; \
     worker-src 'self'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data:; \
     font-src 'self'; \
     connect-src 'self'; \
     object-src 'none'; \
     frame-src 'none'; \
     frame-ancestors 'none'; \
     base-uri 'none'; \
     form-action 'self'";

/// CSP for the PoC portal, which serves several demo frontends from one origin.
///
/// One directive differs from [`EDITOR_CSP`], and it is a real difference
/// rather than a copy that drifted: `worker-src 'self' blob:`.
///
/// The OCW PoCs run their population simulation in a module worker
/// (`usePopulation.js`: `new Worker(new URL('../sim/simWorker.js', …), { type:
/// 'module' })`), and that worker instantiates the WASM engine itself the same
/// way the main thread does — fetch the wasm-bindgen glue, wrap it in a Blob,
/// import the blob URL. The editor never starts a worker, which is why
/// `EDITOR_CSP` deliberately pins `worker-src` to `'self'` and keeps the
/// `blob:` allowance confined to the one import that needs it. Here the worker
/// is the thing that needs it, so the allowance has to extend to `worker-src`
/// as well. Narrowing this back to `'self'` breaks the beleidsview's
/// simulation, silently, in the browser only.
///
/// Everything else matches: `'wasm-unsafe-eval'` and not `'unsafe-eval'`, and
/// `style-src 'unsafe-inline'` for the NDD components, which build a `<style>`
/// element and assign its `textContent`.
pub const POC_CSP: &str = "default-src 'self'; \
     script-src 'self' 'wasm-unsafe-eval' blob:; \
     worker-src 'self' blob:; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data:; \
     font-src 'self'; \
     connect-src 'self'; \
     object-src 'none'; \
     frame-src 'none'; \
     frame-ancestors 'none'; \
     base-uri 'none'; \
     form-action 'self'";

type SecurityHeadersFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

/// Middleware that stamps the security headers onto every response, with
/// `csp` as the `Content-Security-Policy`.
///
/// Apply it as the **outermost** layer, and in particular after any
/// `fallback_service`: `Router::layer` only wraps the routes and the
/// fallback that exist at the moment it is called, so a fallback registered
/// later is served without headers. That is how the editor's static files —
/// the only documents a browser actually renders — went uncovered.
pub fn security_headers(
    csp: &'static str,
) -> impl Fn(Request, Next) -> SecurityHeadersFuture + Clone + Send + Sync + 'static {
    move |request: Request, next: Next| {
        Box::pin(async move {
            let mut response = next.run(request).await;
            let headers = response.headers_mut();
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static(X_CONTENT_TYPE_OPTIONS),
            );
            headers.insert(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_static(X_FRAME_OPTIONS),
            );
            headers.insert(
                header::REFERRER_POLICY,
                HeaderValue::from_static(REFERRER_POLICY),
            );
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static(csp),
            );
            headers.insert(
                "permissions-policy",
                HeaderValue::from_static(PERMISSIONS_POLICY),
            );
            headers.insert(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static(STRICT_TRANSPORT_SECURITY),
            );
            response
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::middleware as axum_middleware;
    use axum::routing::get;
    use tower::ServiceExt;

    async fn headers_of(app: Router, uri: &str) -> axum::http::HeaderMap {
        app.oneshot(
            axum::http::Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response")
        .headers()
        .clone()
    }

    #[tokio::test]
    async fn every_header_is_set() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum_middleware::from_fn(security_headers(API_CSP)));

        let headers = headers_of(app, "/test").await;
        assert_eq!(headers["x-content-type-options"], X_CONTENT_TYPE_OPTIONS);
        assert_eq!(headers["x-frame-options"], X_FRAME_OPTIONS);
        assert_eq!(headers["referrer-policy"], REFERRER_POLICY);
        assert_eq!(headers["permissions-policy"], PERMISSIONS_POLICY);
        assert_eq!(
            headers["strict-transport-security"],
            STRICT_TRANSPORT_SECURITY
        );
        assert_eq!(headers["content-security-policy"], API_CSP);
    }

    /// Pins the ordering rule documented on [`security_headers`], in both
    /// directions. It fails silently in production: every API route still
    /// answers with headers, and only the rendered document does not.
    #[tokio::test]
    async fn a_fallback_registered_after_the_layer_is_uncovered() {
        let uncovered = Router::new()
            .route("/api/thing", get(|| async { "ok" }))
            .layer(axum_middleware::from_fn(security_headers(EDITOR_CSP)))
            .fallback(|| async { "index.html" });

        assert!(
            headers_of(uncovered.clone(), "/api/thing")
                .await
                .contains_key("content-security-policy")
        );
        assert!(
            !headers_of(uncovered, "/deep/link")
                .await
                .contains_key("content-security-policy"),
            "if axum ever starts covering a later fallback, drop this test \
             rather than the layer ordering it guards"
        );

        let covered = Router::new()
            .route("/api/thing", get(|| async { "ok" }))
            .fallback(|| async { "index.html" })
            .layer(axum_middleware::from_fn(security_headers(EDITOR_CSP)));

        assert_eq!(
            headers_of(covered, "/deep/link").await["content-security-policy"],
            EDITOR_CSP
        );
    }

    fn directive<'a>(csp: &'a str, name: &str) -> Option<&'a str> {
        csp.split(';')
            .map(str::trim)
            .find(|d| d.split_whitespace().next() == Some(name))
    }

    /// A CSP that grants `unsafe-eval` or inline script is what an external
    /// scan reports as "CSP with certain insecure settings". Inline *styles*
    /// are a separate, unavoidable case and are allowed here.
    ///
    /// The check runs over whichever directive actually governs script, so a
    /// policy that drops `script-src` and widens `default-src` instead is
    /// caught rather than skipped.
    #[test]
    fn no_policy_grants_eval_or_inline_script() {
        for csp in [API_CSP, EDITOR_CSP, POC_CSP] {
            assert!(!csp.contains("'unsafe-eval'"), "{csp}");
            let governing = directive(csp, "script-src")
                .or_else(|| directive(csp, "default-src"))
                .unwrap_or_else(|| panic!("{csp} governs script through neither directive"));
            assert!(!governing.contains("'unsafe-inline'"), "{governing}");
        }
    }

    /// Directives that do not inherit from `default-src`. Leaving one out is
    /// the classic hole in an otherwise strict policy, and so is naming one
    /// with a value that permits everything.
    #[test]
    fn every_policy_closes_the_non_inheriting_directives() {
        for csp in [API_CSP, EDITOR_CSP, POC_CSP] {
            for name in ["frame-ancestors", "base-uri"] {
                assert_eq!(
                    directive(csp, name),
                    Some(format!("{name} 'none'").as_str()),
                    "in {csp}"
                );
            }
            let form_action = directive(csp, "form-action").expect("form-action");
            assert!(
                form_action == "form-action 'none'" || form_action == "form-action 'self'",
                "{form_action}"
            );
        }
    }

    /// The PoC portal's one deliberate widening over the editor's policy.
    ///
    /// Pinned in both directions. Narrowing `worker-src` back to `'self'`
    /// looks like a tightening and breaks the OCW beleidsview's simulation in
    /// the browser only — no test, no log, just a worker that never starts.
    /// Widening the editor's would hand a `blob:` worker to a policy that has
    /// no use for one.
    #[test]
    fn only_the_poc_policy_allows_a_blob_worker() {
        assert_eq!(
            directive(POC_CSP, "worker-src"),
            Some("worker-src 'self' blob:")
        );
        assert_eq!(
            directive(EDITOR_CSP, "worker-src"),
            Some("worker-src 'self'")
        );
    }
}
