//! The parking card process through the HTTP API, in test mode with the
//! test login: an application by a citizen, the GGD's advice, the decision,
//! and what each account may and may not see and do on the way.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use regelrecht_auth::OidcConfig;
use regelrecht_wetsproces::app;
use regelrecht_wetsproces::config::{
    Config, DEFAULT_MANIFEST, DEFAULT_PORT, DEFAULT_SUBJECT_CLAIM, LoginChoice, Mode, StoreChoice,
};
use regelrecht_wetsproces::login::{Login, OidcLogin};
use serde_json::{Value, json};
use tower::ServiceExt;

const ADVICE: &str =
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";
const GEMEENTE: &str = "gemeente_wilgendaal";
const GGD: &str = "ggd_wilgendaal";

/// Made-up citizen numbers of the BSN test range.
const ANNA: &str = "999990032";
const BRAM: &str = "999990020";

fn config(mode: Mode) -> Config {
    Config {
        mode,
        store: StoreChoice::Memory,
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        port: DEFAULT_PORT,
        login: LoginChoice::TestLogin,
        subject_claim: DEFAULT_SUBJECT_CLAIM.into(),
    }
}

async fn server() -> Router {
    app::build_with(&config(Mode::Test), Login::TestLogin)
        .await
        .unwrap()
}

/// A login through Keycloak whose client was made without discovery; the
/// hosts are made up.
fn offline_login() -> Login {
    let issuer = "https://login.example.org/realms/voorbeeld";
    Login::Oidc(Arc::new(OidcLogin {
        config: OidcConfig {
            client_id: "wetsproces".into(),
            client_secret: "secret".into(),
            issuer_url: issuer.into(),
            required_role: "wetsproces-toegang".into(),
            extra_claims: vec![DEFAULT_SUBJECT_CLAIM.into()],
        },
        base_url: Some("https://wetsproces.example.org".into()),
        client: Arc::new(regelrecht_auth::test_utils::offline_client(issuer)),
        end_session_url: None,
    }))
}

/// One account's session on the server.
struct As<'a> {
    app: &'a Router,
    cookie: Option<String>,
}

impl As<'_> {
    /// `method` on `path` with `body` and, if given, an idempotency key: the
    /// status and the JSON answer.
    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(cookie) = &self.cookie {
            request = request.header("cookie", cookie);
        }
        if let Some(key) = key {
            request = request.header("Idempotency-Key", key);
        }
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.call(Method::GET, path, None, None).await
    }

    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, path, Some(body), None).await
    }
}

fn anonymous(app: &Router) -> As<'_> {
    As { app, cookie: None }
}

/// Log in with the test login as `sub`, with `roles` and `claims`.
async fn login<'a>(app: &'a Router, sub: &str, roles: &[&str], claims: Value) -> As<'a> {
    let body = json!({ "sub": sub, "rollen": roles, "claims": claims });
    let request = Request::post("/auth/testlogin")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get("set-cookie")
        .expect("a test login sets the session cookie")
        .to_str()
        .unwrap();
    assert!(cookie.starts_with("wetsproces_session="), "{cookie}");
    let cookie = cookie.split(';').next().unwrap().to_string();
    As {
        app,
        cookie: Some(cookie),
    }
}

async fn citizen<'a>(app: &'a Router, sub: &str, bsn: &str) -> As<'a> {
    login(app, sub, &["wetsproces-burger"], json!({ "bsn": bsn })).await
}

async fn employee<'a>(app: &'a Router, sub: &str, level: &str, cell: &str) -> As<'a> {
    let cell_role = format!("wetsproces-cel-{cell}");
    login(app, sub, &[level, &cell_role], json!({})).await
}

async fn admin(app: &Router) -> As<'_> {
    login(app, "admin-1", &["wetsproces-admin"], json!({})).await
}

async fn set_clock(app: &Router, day: &str) {
    let (status, klok) = admin(app)
        .await
        .call(
            Method::PUT,
            "/api/backoffice/klok",
            Some(json!({ "datum": day })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{klok}");
    assert_eq!(klok["datum"], json!(day));
}

async fn head(app: &Router, cell: &str) -> u64 {
    let reader = employee(app, "lezer-1", "wetsproces-lezer", cell).await;
    let (status, kroniek) = reader
        .get(&format!("/api/backoffice/cellen/{cell}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    kroniek["kop"].as_u64().unwrap()
}

/// An application as a citizen sends it: without the citizen number, which
/// the login fills.
fn application() -> Value {
    json!({
        "velden": {
            "kaart_als_bestuurder": true,
            "kaart_als_passagier": false,
            "aanvraag_ondertekend": true,
            "naam_aanvrager": "A. de Vries",
            "adres_aanvrager": "Voorbeeldlaan 2",
            "dagtekening": "2026-10-05",
            "gevraagde_beschikking": "gehandicaptenparkeerkaart",
        }
    })
}

fn findings() -> Value {
    json!({
        "langdurige_loopbeperking_volgens_college": true,
        "continu_afhankelijk_van_hulp_bestuurder_volgens_college": false,
        "permanent_rolstoelgebonden_volgens_college": false,
        "ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college": false,
        "geneeskundig_onderzoek_achterwege_gelaten": false,
        "bekend_dat_aanvrager_nog_steeds_voldoet": false,
        "keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet": false,
    })
}

fn advice(bsn: &str) -> Value {
    json!({
        "cel": GGD,
        "feit": ADVICE,
        "velden": {
            "bsn_aanvrager": bsn,
            "datum_onderzoek": "2026-10-16",
            "langdurige_loopbeperking": true,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        },
    })
}

/// Anna applies; the answer is her application.
async fn anna_applies(app: &Router) -> Value {
    let (status, aanvraag) = citizen(app, "anna", ANNA)
        .await
        .post("/api/indieningen", application())
        .await;
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    aanvraag
}

#[tokio::test]
async fn health_is_open_and_the_rest_needs_a_login() {
    let app = server().await;
    let nobody = anonymous(&app);
    let request = Request::get("/health").body(Body::empty()).unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
    for path in [
        "/api/casus",
        "/api/formulier",
        "/api/klok",
        "/api/dossier",
        "/api/backoffice/cellen/gemeente_wilgendaal/kroniek",
    ] {
        assert_eq!(nobody.get(path).await.0, StatusCode::UNAUTHORIZED, "{path}");
    }
    assert_eq!(
        nobody.post("/api/indieningen", application()).await.0,
        StatusCode::UNAUTHORIZED
    );
    // A login without a role of this server gets nothing either.
    let stranger = login(&app, "x", &["editor-reader"], json!({})).await;
    assert_eq!(stranger.get("/api/casus").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn case_forms_and_clock_for_citizens_and_employees() {
    let app = server().await;
    for who in [
        citizen(&app, "anna", ANNA).await,
        employee(&app, "lezer-1", "wetsproces-lezer", GEMEENTE).await,
    ] {
        let (status, casus) = who.get("/api/casus").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(casus["id"], json!("gehandicaptenparkeerkaart"));
        assert_eq!(casus["vandaag"], json!("2026-10-13"));
        let (status, vormen) = who.get("/api/formulier").await;
        assert_eq!(status, StatusCode::OK, "{vormen}");
        let aanvraag = &vormen["indieningen"]["besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49"];
        assert_eq!(aanvraag["soort"], json!("AANVRAAG"), "{vormen}");
        // The UI spec has the login fill the citizen number.
        assert_eq!(
            aanvraag["systeem"]["bsn_aanvrager"],
            json!({}),
            "{aanvraag}"
        );
        let (status, klok) = who.get("/api/klok").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            klok,
            json!({ "datum": "2026-10-13", "modus": "test", "bestuurbaar": true })
        );
    }
}

#[tokio::test]
async fn an_application_the_ggd_advises_on_is_granted_and_lands_in_the_dossier() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    assert_eq!(aanvraag["kind"], json!("AANVRAAG"));
    assert_eq!(aanvraag["recorded_on"], json!("2026-10-13"));
    // The citizen number comes from the login.
    assert_eq!(aanvraag["fields"]["bsn_aanvrager"], json!(ANNA));
    let op = aanvraag["id"].as_str().unwrap().to_string();

    set_clock(&app, "2026-10-16").await;
    let ggd = employee(&app, "ggd-1", "wetsproces-behandelaar", GGD).await;
    let (status, advies) = ggd
        .post("/api/backoffice/vastleggingen", advice(ANNA))
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    assert_eq!(advies["recording_actor"], json!(GGD));
    // The advice reaches the municipality's chronicle too: one step, two
    // cells.
    assert_eq!(head(&app, GGD).await, 1);
    assert_eq!(head(&app, GEMEENTE).await, 2);

    set_clock(&app, "2026-10-20").await;
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser", GEMEENTE).await;
    let (status, besluit) = beslisser
        .call(
            Method::POST,
            "/api/backoffice/besluiten",
            Some(json!({ "op": op, "bevindingen": findings() })),
            Some("besluit-1"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    assert_eq!(
        besluit["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(besluit["besluit"]["recorded_on"], json!("2026-10-20"));
    assert_eq!(head(&app, GEMEENTE).await, 3);

    // Anna's dossier: her application, the advice in both cells, and the
    // decision on her application.
    let (status, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    let kinds: Vec<&str> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), 4, "{dossier}");
    assert!(kinds.contains(&"AANVRAAG"), "{kinds:?}");
    // Bram has applied for nothing: his dossier is empty.
    let (status, dossier) = citizen(&app, "bram", BRAM).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dossier["grammen"], json!([]));
}

#[tokio::test]
async fn a_citizen_acts_only_for_themself() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    let bram = citizen(&app, "bram", BRAM).await;

    // The citizen number is not taken from the request.
    let mut forged = application();
    forged["velden"]["bsn_aanvrager"] = json!(ANNA);
    let (status, fout) = bram.post("/api/indieningen", forged).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert!(!fout.to_string().contains(ANNA), "{fout}");

    // A reference to a record of another's dossier is as unknown as one
    // that does not exist.
    let mut objection = application();
    objection["op"] = json!(op);
    let (status, fout) = bram.post("/api/indieningen", objection.clone()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");
    objection["op"] = json!("no-such-record");
    let (status, other) = bram.post("/api/indieningen", objection).await;
    assert_eq!((status, &other), (StatusCode::NOT_FOUND, &fout));
    // Another submission than the application must refer to the dossier.
    let mut loose = application();
    loose["artikel"] = json!("algemene_wet_bestuursrecht#6:4");
    let (status, _) = bram.post("/api/indieningen", loose).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(head(&app, GEMEENTE).await, 1);

    // The back office is closed to a citizen.
    for path in [
        "/api/backoffice/cellen/gemeente_wilgendaal/kroniek",
        "/api/backoffice/cellen/ggd_wilgendaal/kroniek",
    ] {
        assert_eq!(bram.get(path).await.0, StatusCode::FORBIDDEN, "{path}");
    }
    let (status, _) = bram
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": op, "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A citizen whose account has no citizen number cannot submit.
    let nameless = login(&app, "c", &["wetsproces-burger"], json!({})).await;
    assert_eq!(
        nameless.post("/api/indieningen", application()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(nameless.get("/api/dossier").await.0, StatusCode::FORBIDDEN);
    // An employee is no citizen.
    let lezer = employee(&app, "lezer-1", "wetsproces-lezer", GEMEENTE).await;
    assert_eq!(lezer.get("/api/dossier").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_employee_acts_only_for_their_cells_and_at_their_level() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    let decision = json!({ "op": op, "bevindingen": findings() });

    // A reader of the GGD does not read the municipality's chronicle.
    let ggd_lezer = employee(&app, "ggd-2", "wetsproces-lezer", GGD).await;
    let (status, _) = ggd_lezer
        .get("/api/backoffice/cellen/gemeente_wilgendaal/kroniek")
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // ... nor records for the GGD.
    let (status, _) = ggd_lezer
        .post("/api/backoffice/vastleggingen", advice(ANNA))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A decider of the GGD cannot tell the municipality's application from
    // one that does not exist.
    let ggd_beslisser = employee(&app, "ggd-3", "wetsproces-beslisser", GGD).await;
    let (status, fout) = ggd_beslisser
        .post("/api/backoffice/besluiten", decision.clone())
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");
    let (status, other) = ggd_beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": "no-such-record", "bevindingen": findings() }),
        )
        .await;
    assert_eq!((status, &other), (StatusCode::NOT_FOUND, &fout));

    // A handler of the municipality may read the application but not
    // decide on it.
    let behandelaar = employee(&app, "gemeente-2", "wetsproces-behandelaar", GEMEENTE).await;
    let (status, _) = behandelaar
        .post("/api/backoffice/besluiten", decision.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(head(&app, GEMEENTE).await, 1);

    // Only an admin sets the clock.
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser", GEMEENTE).await;
    let (status, _) = beslisser
        .call(
            Method::PUT,
            "/api/backoffice/klok",
            Some(json!({ "datum": "2026-10-20" })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The decider of the municipality decides.
    let (status, besluit) = beslisser.post("/api/backoffice/besluiten", decision).await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
}

#[tokio::test]
async fn a_retry_under_the_same_key_answers_the_same_and_records_nothing() {
    let app = server().await;
    let anna = citizen(&app, "anna", ANNA).await;
    let submit = |key: Option<&'static str>| {
        anna.call(Method::POST, "/api/indieningen", Some(application()), key)
    };
    let first = submit(Some("k-1")).await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    let again = submit(Some("k-1")).await;
    assert_eq!(again, first);
    assert_eq!(head(&app, GEMEENTE).await, 1);
    // The same key from another account is refused, not answered with
    // Anna's application.
    let (status, fout) = citizen(&app, "bram", BRAM)
        .await
        .call(
            Method::POST,
            "/api/indieningen",
            Some(application()),
            Some("k-1"),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{fout}");
    assert!(!fout.to_string().contains(ANNA), "{fout}");
    // Without a key, every request counts.
    for _ in 0..2 {
        assert_eq!(submit(None).await.0, StatusCode::OK);
    }
    assert_eq!(head(&app, GEMEENTE).await, 3);
    assert_eq!(submit(Some("met spatie")).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn what_the_cell_refuses_and_an_unknown_cell() {
    let app = server().await;
    let elders = employee(&app, "elders-1", "wetsproces-lezer", "elders").await;
    let (status, _) = elders.get("/api/backoffice/cellen/elders/kroniek").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let behandelaar = employee(&app, "ggd-1", "wetsproces-behandelaar", GGD).await;
    let mut unknown_fact = advice(ANNA);
    unknown_fact["feit"] = json!("geen#1#feit");
    let (status, fout) = behandelaar
        .post("/api/backoffice/vastleggingen", unknown_fact)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert!(fout["fout"].is_string(), "{fout}");
    assert_eq!(head(&app, GGD).await, 0);
}

#[tokio::test]
async fn the_test_login_exists_only_in_test_mode_without_oidc() {
    let body = json!({ "sub": "x", "rollen": ["wetsproces-admin"] }).to_string();
    let test_login = |app: Router| {
        let request = Request::post("/auth/testlogin")
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap();
        async move { app.oneshot(request).await.unwrap().status() }
    };
    let login_page = |app: Router| async move {
        let request = Request::get("/auth/login").body(Body::empty()).unwrap();
        app.oneshot(request).await.unwrap().status()
    };
    assert_eq!(test_login(server().await).await, StatusCode::OK);

    // Test mode with OIDC logs in through Keycloak.
    let with_oidc = app::build_with(&config(Mode::Test), offline_login())
        .await
        .unwrap();
    assert_eq!(test_login(with_oidc.clone()).await, StatusCode::NOT_FOUND);
    assert_eq!(login_page(with_oidc).await, StatusCode::TEMPORARY_REDIRECT);

    // Stable mode, on Postgres.
    let database = regelrecht_shared::postgres::test_utils::TestDatabase::create().await;
    let mut c = config(Mode::Stable);
    c.store = StoreChoice::Postgres {
        url: database.url().to_string(),
    };
    let stable = app::build_with(&c, offline_login()).await.unwrap();
    assert_eq!(test_login(stable.clone()).await, StatusCode::NOT_FOUND);
    assert_eq!(
        login_page(stable.clone()).await,
        StatusCode::TEMPORARY_REDIRECT
    );
    let request = Request::get("/api/casus").body(Body::empty()).unwrap();
    assert_eq!(
        stable.oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}
