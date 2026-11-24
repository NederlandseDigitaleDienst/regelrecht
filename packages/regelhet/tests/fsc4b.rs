//! The FSC voor Burgers channel end to end: a server on a port of its own,
//! and the reference client (`regelrecht_regelhet::fsc4b::client`) going
//! through the whole protocol against it, on both stores; and the refusals
//! that need a running server to show.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_auth::OidcConfig;
use regelrecht_regelhet::app::{self, StartupError};
use regelrecht_regelhet::config::{
    Config, DEFAULT_MANIFEST, DEFAULT_PORT, DEFAULT_SUBJECT_CLAIM, Fsc4bChoice, LoginChoice, Mode,
    StoreChoice,
};
use regelrecht_regelhet::fsc4b::client::{self, App, Directory, Session};
use regelrecht_regelhet::fsc4b::demo::{
    CREDENTIAL, DEMO_ORGANIZATION_OIN, STELSELBEHEERDER, STELSELDOCUMENT,
};
use regelrecht_regelhet::fsc4b::jose::{AesKey, SigningKey, VerifyingKey};
use regelrecht_regelhet::fsc4b::testing::{
    OWN_OIN, TestApp, TestStelsel, decide_as_municipality, flip_ciphertext_bit,
};
use regelrecht_regelhet::fsc4b::{self, ROUTES, now_unix};
use regelrecht_regelhet::login::{Login, OidcLogin};
use regelrecht_regelhet::surface::Surface;
use regelrecht_shared::postgres::test_utils::TestDatabase;
use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// Made-up citizen numbers of the BSN test range.
const ANNA: &str = "999990032";
const BRAM: &str = "999990020";

/// A server listening on a port of its own, until the test ends.
struct Server {
    base: String,
    http: reqwest::Client,
}

fn config(mode: Mode, store: StoreChoice, fsc4b: Fsc4bChoice) -> Config {
    Config {
        mode,
        // Stable mode serves one side; the channel belongs to the citizens'.
        surface: match mode {
            Mode::Stable => Surface::Citizen,
            Mode::Test => Surface::All,
        },
        store,
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        case: None,
        frontend: PathBuf::from("/nonexistent/frontend"),
        port: DEFAULT_PORT,
        login: LoginChoice::TestLogin,
        subject_claim: None,
        fsc4b,
        accent: None,
        title: None,
        link: Default::default(),
        role: regelrecht_regelhet::config::Role::Process(None),
    }
}

async fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    (listener, base)
}

async fn serve(listener: TcpListener, config: &Config, login: Login) -> reqwest::Client {
    let router = app::build_with(config, login).await.unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await });
    reqwest::Client::new()
}

/// A server in test mode on the demo Stelsel, with `store`.
async fn demo_server(store: StoreChoice) -> Server {
    let (listener, base) = listener().await;
    let fsc4b = Fsc4bChoice::Demo {
        public_url: base.clone(),
    };
    let http = serve(
        listener,
        &config(Mode::Test, store, fsc4b),
        Login::TestLogin,
    )
    .await;
    Server { base, http }
}

/// A server in test mode on a Stelsel the test holds the keys of.
async fn held_server() -> (Server, TestStelsel, tempfile::TempDir) {
    let (listener, base) = listener().await;
    let stelsel = TestStelsel::at(&base);
    let dir = tempfile::tempdir().unwrap();
    let fsc4b = Fsc4bChoice::On(stelsel.settings(dir.path(), &base));
    let c = config(Mode::Test, StoreChoice::Memory, fsc4b);
    let http = serve(listener, &c, Login::TestLogin).await;
    (Server { base, http }, stelsel, dir)
}

impl Server {
    async fn directory(&self) -> Directory {
        let beheerder: Value = self
            .http
            .get(format!("{}{STELSELBEHEERDER}", self.base))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let document = self
            .http
            .get(format!("{}{STELSELDOCUMENT}", self.base))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        Directory::discover(
            &self.http,
            &document,
            &VerifyingKey::from_value(&beheerder).unwrap(),
            DEMO_ORGANIZATION_OIN,
        )
        .await
        .unwrap()
    }

    /// A session of a fresh app for `bsn`, on the demo Stelsel.
    async fn citizen(&self, bsn: &str) -> Session {
        let app = App::generate().unwrap();
        let credential = client::demo_credential(&self.http, &self.base, &app, bsn)
            .await
            .unwrap();
        client::create_session(&self.http, &self.directory().await, &app, &credential)
            .await
            .unwrap()
    }

    async fn status(&self, method: &str, path: &str) -> StatusCode {
        let url = format!("{}{path}", self.base);
        let request = match method {
            "get" => self.http.get(url),
            _ => self.http.post(url),
        };
        request.send().await.unwrap().status()
    }
}

/// An application as a citizen sends it: only what the citizen fills in;
/// the server fills the citizen number from the credential and the rest of
/// what the UI spec has the system fill.
fn application() -> Value {
    json!({ "velden": {
        "kaart_als_bestuurder": true,
        "kaart_als_passagier": false,
        "adres_aanvrager": "Voorbeeldlaan 2",
    }})
}

/// The ids of the submissions and decisions in a dossier.
fn ids(dossier: &Value) -> Vec<String> {
    dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        // Not the facts recorded beside them, such as the request to the GGD
        // an application brings about.
        .filter(|g| g["kind"] != "EXECUTOGRAM")
        .map(|g| g["id"].as_str().unwrap().to_string())
        .collect()
}

/// The whole protocol on `server`: an application through FSC voor
/// Burgers lands in the citizen's own dossier, with the BSN of the
/// credential in it. The id of the application comes back.
async fn the_whole_protocol(server: &Server) -> String {
    let anna = server.citizen(ANNA).await;
    let (status, aanvraag) = anna
        .call(
            &server.http,
            "indieningen",
            &application(),
            Some("aanvraag-1"),
        )
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    let id = aanvraag["id"].as_str().unwrap().to_string();
    assert_eq!(
        aanvraag["fields"]["bsn_aanvrager"],
        json!(ANNA),
        "{aanvraag}"
    );
    // The policy of the municipality counts only a login with DigiD as a
    // signature (article 8), so through this channel the application does
    // not count as signed. A session of this channel gives no name: the
    // field stays empty.
    assert_eq!(aanvraag["fields"]["aanvraag_ondertekend"], json!(false));
    assert_eq!(aanvraag["fields"]["naam_aanvrager"], json!(null));
    // The same key again answers as the first time.
    let (_, again) = anna
        .call(
            &server.http,
            "indieningen",
            &application(),
            Some("aanvraag-1"),
        )
        .await
        .unwrap();
    assert_eq!(again, aanvraag);
    let (status, dossier) = anna
        .call(&server.http, "dossier", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{dossier}");
    assert_eq!(ids(&dossier), std::slice::from_ref(&id));
    for op in ["casus", "formulier"] {
        let (status, answer) = anna.call(&server.http, op, &json!({}), None).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{op}: {answer}");
    }
    // A submission that fills the login field itself is refused, encrypted.
    let mut forged = application();
    forged["velden"]["bsn_aanvrager"] = json!(BRAM);
    let (status, answer) = anna
        .call(&server.http, "indieningen", &forged, None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{answer}");
    id
}

#[tokio::test]
async fn the_reference_client_runs_the_whole_protocol_in_memory() {
    let server = demo_server(StoreChoice::Memory).await;
    the_whole_protocol(&server).await;
}

#[tokio::test]
async fn the_reference_client_runs_the_whole_protocol_on_postgres_and_the_channel_is_kept() {
    let database = TestDatabase::create().await;
    let server = demo_server(StoreChoice::postgres(database.url().to_string())).await;
    let id = the_whole_protocol(&server).await;
    let pool = sqlx::PgPool::connect(database.url()).await.unwrap();
    let (by, via): (String, String) =
        sqlx::query_as("SELECT recorded_by, recorded_via FROM wetsproces.grams WHERE gram_id = $1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(by.starts_with("fsc4b:"), "{by}");
    assert_eq!(via, "fsc4b");
    // The session is kept under the hash of its token, never the token.
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM wetsproces.fsc4b_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kept, 1);
}

#[tokio::test]
async fn every_route_of_the_specification_answers() {
    let server = demo_server(StoreChoice::Memory).await;
    let spec: Value = server
        .http
        .get(format!("{}{}", server.base, fsc4b::OPENAPI))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(spec["servers"][0]["url"], json!(server.base));
    for (method, path) in ROUTES {
        let status = server.status(method, path).await;
        assert!(
            status != StatusCode::NOT_FOUND && status != StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path}: {status}"
        );
    }
    let discovery: Value = server
        .http
        .get(format!("{}{}", server.base, fsc4b::DISCOVERY))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        discovery["sessionApi"]["v1"],
        json!(format!("{}/fsc4b/session", server.base))
    );
}

#[tokio::test]
async fn citizen_a_does_not_read_or_refer_to_the_dossier_of_citizen_b() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    let bram = server.citizen(BRAM).await;
    let (_, aanvraag) = anna
        .call(&server.http, "indieningen", &application(), None)
        .await
        .unwrap();
    let annas = aanvraag["id"].as_str().unwrap();
    let (status, dossier) = bram
        .call(&server.http, "dossier", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert!(ids(&dossier).is_empty(), "{dossier}");
    let objection = json!({ "velden": {}, "artikel": "x", "op": annas });
    let (status, answer) = bram
        .call(&server.http, "indieningen", &objection, None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "{answer}");
}

#[tokio::test]
async fn a_session_of_fsc_voor_burgers_reaches_no_backoffice_or_browser_route() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    for (method, path) in [
        ("get", "/api/backoffice/cellen/gemeente_wilgendaal/kroniek"),
        ("post", "/api/backoffice/besluiten"),
        ("put", "/api/backoffice/klok"),
        ("get", "/api/dossier"),
    ] {
        let url = format!("{}{path}", server.base);
        let request = match method {
            "get" => server.http.get(url),
            "put" => server.http.put(url),
            _ => server.http.post(url),
        };
        let response = request
            .header("Authorization", &anna.token)
            .header("content-type", "application/json")
            .body(anna.key.encrypt(b"{}").unwrap())
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path}"
        );
    }
    for path in [
        "/fsc4b/burger/v1/backoffice/klok",
        "/fsc4b/burger/v1/kroniek",
    ] {
        assert_eq!(
            server.status("post", path).await,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_session_token_with_another_sessions_key_opens_nothing() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    let bram = server.citizen(BRAM).await;
    // Anna's token, with Bram's key: Bram cannot use a token he saw.
    let mixed = Session {
        token: anna.token.clone(),
        key: bram.key.clone(),
        citizen_api: anna.citizen_api.clone(),
    };
    let (status, _) = mixed
        .call(&server.http, "dossier", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Or a token with a key nobody had.
    let guessed = Session {
        key: AesKey::generate().unwrap(),
        ..anna.clone()
    };
    let (status, _) = guessed
        .call(&server.http, "dossier", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // A made-up token.
    let made_up = Session {
        token: "0".repeat(48),
        ..anna
    };
    let (status, _) = made_up
        .call(&server.http, "dossier", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// One app as the reference client and as the test support know it, for
/// a credential the test mints itself.
fn apps() -> (App, TestApp) {
    let test_app = TestApp::new();
    let key = SigningKey::from_pkcs8_pem(&test_app.key.to_pkcs8_pem().unwrap()).unwrap();
    (App::from_key(key), test_app)
}

/// A challenge and a presentation for it by a fresh app with a credential
/// for Anna, on `server`.
async fn answer(server: &Server, aud: &str) -> (Directory, client::Challenge, String) {
    let directory = server.directory().await;
    let app = App::generate().unwrap();
    let credential = client::demo_credential(&server.http, &server.base, &app, ANNA)
        .await
        .unwrap();
    let challenge = client::challenge(&server.http, &directory).await.unwrap();
    let key = AesKey::generate().unwrap();
    let jwe = client::presentation(&app, &challenge, &credential, aud, &key).unwrap();
    (directory, challenge, jwe)
}

#[tokio::test]
async fn a_nonce_answered_twice_is_refused_the_second_time() {
    let server = demo_server(StoreChoice::Memory).await;
    let (directory, challenge, jwe) = answer(&server, DEMO_ORGANIZATION_OIN).await;
    let first = client::complete(&server.http, &directory, &challenge.nonce, &jwe)
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let second = client::complete(&server.http, &directory, &challenge.nonce, &jwe)
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_nonce_answered_twice_on_two_instances_is_refused_the_second_time() {
    // Two servers on one database, as two instances behind one address.
    let database = TestDatabase::create().await;
    let store = StoreChoice::postgres(database.url().to_string());
    let (one_listener, base) = listener().await;
    let (two_listener, two_base) = listener().await;
    let stelsel = TestStelsel::at(&base);
    let dir = tempfile::tempdir().unwrap();
    let c = config(
        Mode::Test,
        store,
        Fsc4bChoice::On(stelsel.settings(dir.path(), &base)),
    );
    let http = serve(one_listener, &c, Login::TestLogin).await;
    serve(two_listener, &c, Login::TestLogin).await;
    let (app, test_app) = apps();
    let credential = stelsel.credential(&test_app, now_unix(), |_| {});
    let one = Directory {
        oin: OWN_OIN.into(),
        organization_key: stelsel.own.verifying_key().unwrap(),
        session_api: format!("{base}/fsc4b/session"),
        citizen_api: format!("{base}/fsc4b/burger"),
    };
    let two = Directory {
        session_api: format!("{two_base}/fsc4b/session"),
        citizen_api: format!("{two_base}/fsc4b/burger"),
        organization_key: stelsel.own.verifying_key().unwrap(),
        oin: OWN_OIN.into(),
    };
    let challenge = client::challenge(&http, &one).await.unwrap();
    let key = AesKey::generate().unwrap();
    let jwe = client::presentation(&app, &challenge, &credential, OWN_OIN, &key).unwrap();
    let first = client::complete(&http, &two, &challenge.nonce, &jwe)
        .await
        .unwrap();
    assert_eq!(
        first.status(),
        StatusCode::OK,
        "the other instance knows the nonce"
    );
    let second = client::complete(&http, &one, &challenge.nonce, &jwe)
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_nonce_signed_for_another_verifier_gives_no_session() {
    let server = demo_server(StoreChoice::Memory).await;
    let (directory, challenge, jwe) = answer(&server, "00000099000000000077").await;
    let response = client::complete(&server.http, &directory, &challenge.nonce, &jwe)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response.text().await.unwrap();
    // The answer says no more than that.
    assert_eq!(body, r#"{"fout":"the presentation is not accepted"}"#);
}

#[tokio::test]
async fn a_tampered_presentation_gives_no_session() {
    let server = demo_server(StoreChoice::Memory).await;
    let (directory, challenge, jwe) = answer(&server, DEMO_ORGANIZATION_OIN).await;
    let response = client::complete(
        &server.http,
        &directory,
        &challenge.nonce,
        &flip_ciphertext_bit(&jwe),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_expired_credential_gives_no_session() {
    let (server, stelsel, _dir) = held_server().await;
    let (app, test_app) = apps();
    let directory = Directory {
        oin: OWN_OIN.into(),
        organization_key: stelsel.own.verifying_key().unwrap(),
        session_api: format!("{}/fsc4b/session", server.base),
        citizen_api: format!("{}/fsc4b/burger", server.base),
    };
    let expired = stelsel.credential(&test_app, now_unix() - 7200, |_| {});
    let e = client::create_session(&server.http, &directory, &app, &expired)
        .await
        .unwrap_err();
    assert!(
        matches!(
            e,
            client::ClientError::Status {
                status: StatusCode::UNAUTHORIZED,
                ..
            }
        ),
        "{e}"
    );
    let good = stelsel.credential(&test_app, now_unix(), |_| {});
    client::create_session(&server.http, &directory, &app, &good)
        .await
        .unwrap();
}

#[tokio::test]
async fn without_configuration_no_route_of_the_channel_exists() {
    let (listener, base) = listener().await;
    let c = config(Mode::Test, StoreChoice::Memory, Fsc4bChoice::Off);
    let http = serve(listener, &c, Login::TestLogin).await;
    let server = Server { base, http };
    for (method, path) in ROUTES
        .into_iter()
        .chain([("get", STELSELDOCUMENT), ("post", CREDENTIAL)])
    {
        assert_eq!(
            server.status(method, path).await,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    assert_eq!(server.status("get", "/health").await, StatusCode::OK);
}

#[tokio::test]
async fn on_a_real_stelsel_the_demo_routes_do_not_exist() {
    let (server, _stelsel, _dir) = held_server().await;
    for (method, path) in [
        ("get", STELSELDOCUMENT),
        ("get", STELSELBEHEERDER),
        ("post", CREDENTIAL),
    ] {
        assert_eq!(
            server.status(method, path).await,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    assert_eq!(server.status("get", fsc4b::DISCOVERY).await, StatusCode::OK);
}

/// A login through Keycloak whose client was made without discovery; the
/// hosts are made up.
fn offline_login() -> Login {
    let issuer = "https://login.example.org/realms/voorbeeld";
    Login::Oidc(Arc::new(OidcLogin {
        config: OidcConfig {
            client_id: "regelhet".into(),
            client_secret: "secret".into(),
            issuer_url: issuer.into(),
            required_role: "regelhet-toegang".into(),
            extra_claims: vec![DEFAULT_SUBJECT_CLAIM.into()],
        },
        base_url: Some("https://regelhet.example.org".into()),
        client: Arc::new(regelrecht_auth::test_utils::offline_client(issuer)),
        end_session_url: None,
    }))
}

#[tokio::test]
async fn stable_mode_refuses_the_demo_stelsel_and_registers_no_demo_route() {
    let database = TestDatabase::create().await;
    let store = StoreChoice::postgres(database.url().to_string());
    let demo = Fsc4bChoice::Demo {
        public_url: "https://regelhet.example.org".into(),
    };
    let e = app::build_with(&config(Mode::Stable, store.clone(), demo), offline_login())
        .await
        .unwrap_err();
    assert!(
        matches!(e, StartupError::Fsc4b(fsc4b::Fsc4bError::StableDemo)),
        "{e}"
    );
    // On a real Stelsel stable mode starts, without the demo's routes.
    let (listener, base) = listener().await;
    let stelsel = TestStelsel::at(&base);
    let dir = tempfile::tempdir().unwrap();
    let c = config(
        Mode::Stable,
        store,
        Fsc4bChoice::On(stelsel.settings(dir.path(), &base)),
    );
    let http = serve(listener, &c, offline_login()).await;
    let server = Server { base, http };
    assert_eq!(
        server.status("post", CREDENTIAL).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        server.status("get", STELSELDOCUMENT).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(server.status("get", fsc4b::DISCOVERY).await, StatusCode::OK);
}

#[tokio::test]
async fn a_submission_sent_again_as_it_was_is_recorded_once() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    let mut payload = application();
    payload["idempotency_key"] = json!("aanvraag-replay");
    let body = anna.key.encrypt(payload.to_string().as_bytes()).unwrap();
    let (status, first) = anna
        .send(&server.http, "indieningen", body.clone())
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{first}");
    // Whoever copied the request sends it again, byte for byte.
    let (status, again) = anna.send(&server.http, "indieningen", body).await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, first);
    let (_, dossier) = anna
        .call(&server.http, "dossier", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(ids(&dossier).len(), 1, "{dossier}");
}

#[tokio::test]
async fn a_body_that_is_no_json_object_is_refused_under_the_session_key() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    let body = anna.key.encrypt(b"[]").unwrap();
    let (status, answer) = anna.send(&server.http, "dossier", body).await.unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(answer["fout"].is_string(), "{answer}");
}

/// A decider of the municipality decides on `op`; the id of the decision.
async fn decide(server: &Server, op: &str) -> String {
    decide_as_municipality(&server.http, &server.base, op).await
}

#[tokio::test]
async fn an_app_asks_why_a_decision_of_its_own_dossier_came_out_so() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    let (_, aanvraag) = anna
        .call(&server.http, "indieningen", &application(), None)
        .await
        .unwrap();
    let id = decide(&server, aanvraag["id"].as_str().unwrap()).await;
    let (status, waarom) = anna
        .call(&server.http, "waarom", &json!({ "id": id }), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{waarom}");
    assert!(
        !waarom["ingevuld"].as_array().unwrap().is_empty(),
        "{waarom}"
    );
    // Not for another citizen; and an id is required.
    let bram = server.citizen(BRAM).await;
    let (status, _) = bram
        .call(&server.http, "waarom", &json!({ "id": id }), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = anna
        .call(&server.http, "waarom", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn with_the_frontend_built_an_unknown_fsc4b_path_still_answers_404() {
    let dir = tempfile::tempdir().unwrap();
    // The build of the surface the test runs on, both sides in test mode.
    std::fs::create_dir(dir.path().join("all")).unwrap();
    std::fs::write(dir.path().join("all/index.html"), "<!doctype html>").unwrap();
    let (listener, base) = listener().await;
    let mut c = config(Mode::Test, StoreChoice::Memory, Fsc4bChoice::Off);
    c.frontend = dir.path().to_path_buf();
    let http = serve(listener, &c, Login::TestLogin).await;
    let server = Server { base, http };
    assert_eq!(
        server.status("get", fsc4b::DISCOVERY).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(server.status("get", "/").await, StatusCode::OK);
}
