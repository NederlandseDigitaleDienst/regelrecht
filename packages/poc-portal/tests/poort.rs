//! The gate, through the real router.
//!
//! The unit tests prove the cookie cannot be forged; these prove the router
//! actually asks. Those are different claims, and the second is the one that
//! breaks when a route is registered in the wrong order or a layer is added
//! after the fallback.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt as _;

use regelrecht_poc_portal::app::{router, AppState};
use regelrecht_poc_portal::config::Config;
use regelrecht_poc_portal::gate;

const SECRET: &str = "0123456789abcdef0123456789abcdef";

const REGISTER: &str = r#"
pocs:
  - slug: alfa
    titel: Alfa
    samenvatting: De eerste
    soort: statisch
    bron: poc-alfa
    status: verkenning
    voorbehoud: Een demonstratie op verzonnen gegevens, geen geldend recht.
  - slug: beta
    titel: Beta
    samenvatting: De tweede
    soort: statisch
    bron: poc-beta
    status: gevalideerd
    voorbehoud: Doorgelopen met experts, maar nog steeds een demonstratie.
"#;

/// Build a portal over a register of two static PoCs.
fn app() -> axum::Router {
    // Only the variables every test agrees on are set here, and they are always
    // set to the same values. The static root is not among them: it differs per
    // test, and a process-wide variable that differs per test is a race.
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    let config = Config::from_env(REGISTER)
        .expect("config")
        .met_static_root("/nonexistent");
    router(AppState::new(config))
}

/// Een portaal waarin alfa een beleidsassistent heeft en beta niet.
fn app_met_assistent() -> axum::Router {
    let register = REGISTER.replace(
        "    bron: poc-alfa\n",
        "    bron: poc-alfa\n    assistent: true\n",
    );
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    let config = Config::from_env(&register)
        .expect("config")
        .met_static_root("/nonexistent");
    router(AppState::new(config))
}

fn cookie_voor(slug: &str) -> String {
    let sleutel = gate::Sleutel::new(SECRET).expect("key");
    format!(
        "{}={}",
        gate::cookie_naam(slug),
        gate::maak_cookie(&sleutel, slug, time::OffsetDateTime::now_utc())
    )
}

async fn get(app: &axum::Router, pad: &str, cookie: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::builder().uri(pad).method("GET");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn the_index_is_public() {
    let (status, body) = get(&app(), "/", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Alfa") && body.contains("Beta"));
}

#[tokio::test]
async fn health_is_public() {
    let (status, body) = get(&app(), "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn a_poc_without_a_cookie_asks_for_the_password() {
    let (status, body) = get(&app(), "/alfa/", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("Wachtwoord"));
    assert!(body.contains(r#"action="/_toegang/alfa""#));
}

#[tokio::test]
async fn a_deep_link_keeps_its_address_so_signing_in_continues_there() {
    // The reason for 401-with-a-body rather than a redirect.
    let (status, body) = get(&app(), "/alfa/beleid/detail", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains(r#"value="/alfa/beleid/detail""#));
}

#[tokio::test]
async fn one_pocs_cookie_does_not_open_another() {
    // The property the whole per-PoC design exists for, verified through the
    // router rather than only over the signing function.
    let app = app();
    let (status, _) = get(&app, "/beta/", Some(&cookie_voor("alfa"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_valid_cookie_gets_past_the_gate() {
    // The static root points at nothing, so "past the gate" shows up as a 404
    // from the file service rather than the 401 the gate would return.
    let app = app();
    let (status, _) = get(&app, "/alfa/", Some(&cookie_voor("alfa"))).await;
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unknown_poc_is_not_found_rather_than_a_password_prompt() {
    let (status, _) = get(&app(), "/bestaat-niet/", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn post_wachtwoord(app: &axum::Router, slug: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/_toegang/{slug}"))
                .method("POST")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response")
}

#[tokio::test]
async fn the_right_password_sets_a_cookie_scoped_to_that_poc() {
    let response =
        post_wachtwoord(&app(), "alfa", "wachtwoord=alfa-geheim&verder=%2Falfa%2F").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("set-cookie");
    assert!(cookie.starts_with("poc_alfa="));
    // Zonder sluitende schuine streep: een browser stuurt een cookie met
    // `Path=/alfa/` niet naar `/alfa` zelf. Een poc die zijn eigen basispad
    // naar de kale vorm omleidt (napp doet dat) raakte het cookie daar kwijt,
    // en de bezoeker kreeg met een goed wachtwoord het formulier terug.
    assert!(cookie.contains("Path=/alfa;"), "{cookie}");
    assert!(!cookie.contains("Path=/alfa/"), "{cookie}");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("Secure"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");

    assert_eq!(
        response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/alfa/")
    );
}

#[tokio::test]
async fn the_wrong_password_sets_no_cookie() {
    let response = post_wachtwoord(&app(), "alfa", "wachtwoord=fout&verder=%2Falfa%2F").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn another_pocs_password_does_not_open_this_one() {
    let response = post_wachtwoord(&app(), "alfa", "wachtwoord=beta-geheim").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn the_return_path_cannot_send_a_visitor_off_this_host() {
    // `verder` comes from the form, so it is attacker-controlled. Anything
    // that is not a path inside this PoC falls back to the index.
    for kwaad in [
        "https%3A%2F%2Fkwaadaardig.nl",
        "%2F%2Fkwaadaardig.nl",
        "%2Fbeta%2F",
        "%2Fetc%2Fpasswd",
    ] {
        let response = post_wachtwoord(
            &app(),
            "alfa",
            &format!("wachtwoord=alfa-geheim&verder={kwaad}"),
        )
        .await;
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/"),
            "verder={kwaad} should not be followed",
        );
    }
}

#[tokio::test]
async fn a_poc_page_carries_the_notice_and_its_assets_do_not() {
    // The third place the disclaimer lives, and the one that survives a
    // forwarded deep link or a screenshot: inside the PoC's own page. Its
    // assets must stay untouched — a <div> prepended to a JS bundle is a
    // broken PoC.
    let dir = std::env::temp_dir().join(format!("poc-strip-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("alfa")).expect("mkdir");
    std::fs::write(
        dir.join("alfa/index.html"),
        "<html><body><h1>Alfa</h1></body></html>",
    )
    .expect("index");
    std::fs::write(dir.join("alfa/app.js"), "console.log(1)").expect("js");

    // The static root is set on the config, not through POC_STATIC_DIR: that
    // variable is process-wide, and setting it here raced the other tests in
    // this binary (one failure in five runs).
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    let config = Config::from_env(REGISTER)
        .expect("config")
        .met_static_root(dir.to_string_lossy().into_owned());
    let app = router(AppState::new(config));

    let (_, html) = get(&app, "/alfa/", Some(&cookie_voor("alfa"))).await;
    assert!(html.contains("data-poc-portaal"), "{html}");
    assert!(html.contains("demonstratie"), "{html}");

    let (_, js) = get(&app, "/alfa/app.js", Some(&cookie_voor("alfa"))).await;
    assert_eq!(js, "console.log(1)", "an asset must not be rewritten");

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_deep_link_into_a_poc_gets_its_index_and_a_missing_file_a_404() {
    // A PoC with its own router puts the case and scenario in the path
    // (`/chronolexografie/<case>/<scenario>`). A hard refresh there must get
    // the PoC's index with 200, not a 404 a proxy might replace; a missing
    // asset keeps its 404.
    let dir = std::env::temp_dir().join(format!("poc-diep-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("alfa")).expect("mkdir");
    std::fs::write(
        dir.join("alfa/index.html"),
        "<html><body><h1>Alfa</h1></body></html>",
    )
    .expect("index");

    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    let config = Config::from_env(REGISTER)
        .expect("config")
        .met_static_root(dir.to_string_lossy().into_owned());
    let app = router(AppState::new(config));

    let (status, html) = get(
        &app,
        "/alfa/een-casus/een-scenario",
        Some(&cookie_voor("alfa")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<h1>Alfa</h1>"), "{html}");
    assert!(html.contains("data-poc-portaal"), "{html}");

    let (status, _) = get(&app, "/alfa/assets/weg.js", Some(&cookie_voor("alfa"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn the_assistant_answers_503_when_it_is_not_running() {
    // Niet 404: het endpoint bestaat, de assistent draait alleen niet in deze
    // omgeving. De app doet toch al een health-probe en verbergt zijn paneel.
    // Wel achter de poort — /api is van de poc, niet openbaar.
    let app = app_met_assistent();
    let (zonder, _) = get(&app, "/alfa/api/health", None).await;
    assert_eq!(zonder, StatusCode::UNAUTHORIZED);

    let (met, _) = get(&app, "/alfa/api/health", Some(&cookie_voor("alfa"))).await;
    assert_eq!(met, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_poc_without_an_assistant_serves_api_as_a_normal_path() {
    // beta heeft `assistent: false`, dus /api is voor hem een gewoon pad dat
    // de statische bestandsdienst afhandelt (hier: 404, want de map bestaat
    // niet). Geen 503, want er valt niets te proxyen.
    let app = app_met_assistent();
    let (status, _) = get(&app, "/beta/api/health", Some(&cookie_voor("beta"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn every_answer_carries_the_security_headers() {
    // The layer sits over the fallback too, which is where the PoCs are
    // served — the mistake `serve_static_and_secure` in editor-api documents.
    let app = app();
    for (pad, cookie) in [
        ("/", None),
        ("/alfa/", None),
        ("/alfa/", Some(cookie_voor("alfa"))),
    ] {
        let mut req = Request::builder().uri(pad).method("GET");
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        let response = app
            .clone()
            .oneshot(req.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let csp = response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(csp.contains("worker-src 'self' blob:"), "{pad}: {csp}");
        assert!(
            response.headers().contains_key("x-content-type-options"),
            "{pad}"
        );
    }
}

#[tokio::test]
async fn a_config_without_a_password_refuses_to_start() {
    // The failure mode this guards is a PoC in the register that is served to
    // anyone because its env var was forgotten.
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    // Ook de wachtwoorden van het basisregister zetten, en niet erop vertrouwen
    // dat een andere test dat al deed: de omgeving is procesbreed en de tests
    // draaien parallel. Liep deze eerst, dan struikelde `from_env` over
    // POC_PW_ALFA en ging de assertie over gamma nooit op -- een test die faalt
    // op de volgorde waarin hij toevallig draait, niet op wat hij bewaakt.
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    std::env::remove_var("POC_PW_GAMMA");
    let register = format!(
        "{REGISTER}  - slug: gamma\n    titel: G\n    samenvatting: S\n    soort: statisch\n    \
         bron: b\n    status: verkenning\n    voorbehoud: Een demonstratie, geen geldend recht.\n"
    );
    // `Config` holds the signing key and every password, so it deliberately
    // does not derive Debug — hence matching rather than `expect_err`.
    match Config::from_env(&register) {
        Ok(_) => panic!("a PoC without a password must not start"),
        Err(e) => assert!(e.to_string().contains("POC_PW_GAMMA"), "{e}"),
    }
}

/// A portal in which alfa has two sources: `poortlokaal` from a local folder
/// (`POC_BRON_POORTLOKAAL`) and `poortzonder` with neither a folder nor a
/// token. The names are this test's own, so no other test sets their
/// variables.
fn app_met_bronnen(dir: &std::path::Path) -> axum::Router {
    let bron = |naam: &str| {
        format!(
            "      - name: {naam}\n        github:\n          owner: example-org\n          \
             repo: regelrecht-corpus-example\n          branch: poc/x\n          path: x\n        \
             token_env: POORT_{}_TOKEN_NOOIT_GEZET\n",
            naam.to_uppercase()
        )
    };
    let register = REGISTER.replace(
        "    bron: poc-alfa\n",
        &format!(
            "    bron: poc-alfa\n    sources:\n{}{}",
            bron("poortlokaal"),
            bron("poortzonder")
        ),
    );
    std::env::set_var("POC_COOKIE_SECRET", SECRET);
    std::env::set_var("POC_PW_ALFA", "alfa-geheim");
    std::env::set_var("POC_PW_BETA", "beta-geheim");
    std::env::set_var("POC_BRON_POORTLOKAAL", dir);
    let config = Config::from_env(&register)
        .expect("config")
        .met_static_root("/nonexistent");
    router(AppState::new(config))
}

#[tokio::test]
async fn a_source_is_served_behind_the_password_of_its_poc() {
    let dir = std::env::temp_dir().join(format!("poc-poort-bron-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("casussen")).expect("dir");
    std::fs::write(dir.join("casussen/c.yaml"), "cel: c").expect("file");
    let app = app_met_bronnen(&dir);

    // Without the password, the login page; with another PoC's, too.
    let (status, _) = get(&app, "/alfa/bron/poortlokaal", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = get(&app, "/alfa/bron/poortlokaal", Some(&cookie_voor("beta"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // With it, the files of the local folder, by their path under it.
    let (status, body) = get(&app, "/alfa/bron/poortlokaal", Some(&cookie_voor("alfa"))).await;
    std::fs::remove_dir_all(&dir).expect("cleanup");
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["naam"], "poortlokaal");
    assert_eq!(json["versie"], "lokaal");
    assert_eq!(json["bestanden"]["casussen/c.yaml"], "cel: c");

    // A source without a folder or a token says so. A path for a source not
    // in the register stays the PoC's own (here the static root, which is
    // empty), and so does a source of another PoC.
    let (status, body) = get(&app, "/alfa/bron/poortzonder", Some(&cookie_voor("alfa"))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.starts_with("bron niet beschikbaar"), "{body}");
    assert!(
        body.contains("POORT_POORTZONDER_TOKEN_NOOIT_GEZET"),
        "{body}"
    );
    for (pad, poc) in [
        ("/alfa/bron/onbekend", "alfa"),
        ("/beta/bron/poortlokaal", "beta"),
    ] {
        let (status, body) = get(&app, pad, Some(&cookie_voor(poc))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{pad}");
        assert!(!body.contains("bestanden"), "{pad}: {body}");
    }
}
