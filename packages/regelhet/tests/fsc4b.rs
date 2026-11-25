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
    Config, DEFAULT_MANIFEST, DEFAULT_PORT, DEFAULT_SUBJECT_CLAIM, Fsc4bChoice, Fsc4bOrigins,
    LoginChoice, Mode, StoreChoice,
};
use regelrecht_regelhet::fsc4b::client::{self, App, Directory, Session};
use regelrecht_regelhet::fsc4b::demo::{
    CREDENTIAL, DEMO_ORGANIZATION_OIN, STELSELBEHEERDER, STELSELDOCUMENT,
};
use regelrecht_regelhet::fsc4b::jose::{AesKey, SigningKey, VerifyingKey};
use regelrecht_regelhet::fsc4b::testing::{
    OWN_OIN, TestApp, TestStelsel, decide_as_municipality, flip_ciphertext_bit, test_login,
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
        fsc4b_origins: Default::default(),
        accent: None,
        vlam: None,
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
    demo_server_for(store, Fsc4bOrigins::default()).await
}

/// A server in test mode on the demo Stelsel, with `store`, answering the
/// browser apps of `origins`.
async fn demo_server_for(store: StoreChoice, origins: Fsc4bOrigins) -> Server {
    let (listener, base) = listener().await;
    let fsc4b = Fsc4bChoice::Demo {
        public_url: base.clone(),
    };
    let mut c = config(Mode::Test, store, fsc4b);
    c.fsc4b_origins = origins;
    let http = serve(listener, &c, Login::TestLogin).await;
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

/// The source data of `session` on `server`, asked with `body`, after
/// checking that `ondertekend` verifies under the organization's key from
/// the Stelseldocument and that `gegevens` is its payload.
async fn source_data(server: &Server, session: &Session, body: &Value) -> Value {
    let (status, answer) = session
        .call(&server.http, "gegevens", body, None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{answer}");
    let key = server.directory().await.organization_key;
    let verified = key.verify(answer["ondertekend"].as_str().unwrap()).unwrap();
    assert_eq!(Value::Object(verified.claims), answer["gegevens"]);
    answer["gegevens"].clone()
}

/// The answers of the municipality's cell that take the BSN alone. The
/// others drop out: the advice of the GGD is read for an application (by
/// its date), a valid card on the day of a decision, and the business
/// permits by a KvK number.
const HANDED_OUT: [&str; 2] = [
    "aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt",
    "verzoek_om_geneeskundig_onderzoek_staat_open",
];

/// The fact by which the GGD advises the municipality.
const ADVICE: &str =
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";

#[tokio::test]
async fn the_source_data_are_the_answers_on_the_bsn_of_the_session_signed() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    // A BSN in the request is not read: the session says whose data these
    // are.
    let data = source_data(&server, &anna, &json!({ "bsn_aanvrager": BRAM })).await;
    assert_eq!(data["sub"], json!(ANNA));
    assert_eq!(data["iss"], json!(DEMO_ORGANIZATION_OIN));
    assert_eq!(data["casus"], json!("gehandicaptenparkeerkaart"));
    assert_eq!(data["cel"], json!("gemeente_wilgendaal"));
    let names: Vec<&str> = data["antwoorden"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names, HANDED_OUT, "{data:#}");
    for (name, answer) in data["antwoorden"].as_object().unwrap() {
        assert_eq!(
            answer["parameters"],
            json!({ "bsn_aanvrager": ANNA }),
            "{name}"
        );
    }
    // The advice of the GGD comes only after an application: it is named
    // as still to come, not handed out.
    assert_eq!(
        data["bij_aanvraag"],
        json!([{
            "feit": ADVICE,
            "taak": "Geneeskundig advies uitbrengen",
        }])
    );
    assert_eq!(data["records"], json!([]));
    // Never granted a card, so the cell says nothing: no answer, not "no".
    assert_eq!(
        data["antwoorden"][HANDED_OUT[0]]["waarde"],
        json!(null),
        "{data:#}"
    );
    assert!(
        !data["regelwerken"].as_array().unwrap().is_empty(),
        "{data:#}"
    );
}

/// An admin of the server at `base` sets the test clock on `day`.
async fn set_clock(http: &reqwest::Client, base: &str, cookie: &str, day: &str) {
    let response = http
        .put(format!("{base}/api/backoffice/klok"))
        .header("cookie", cookie)
        .json(&json!({ "datum": day }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// An admin enters the GGD's advice on `bsn` in the admin panel, from the
/// open request about `bsn`, on 2026-10-16; the clock then stands on
/// 2026-10-20.
async fn ggd_advises(http: &reqwest::Client, base: &str, bsn: &str) {
    let admin = test_login(http, base, "admin-1", &["regelhet-admin"]).await;
    set_clock(http, base, &admin, "2026-10-16").await;
    let panel: Value = http
        .get(format!("{base}/api/backoffice/beheer/partijen"))
        .header("cookie", &admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let verzoek = panel["partijen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["cel"] == json!("ggd_wilgendaal"))
        .and_then(|p| p["verzoeken"].as_array())
        .and_then(|all| {
            all.iter()
                .find(|r| r["verzoek"]["fields"]["bsn_aanvrager"] == json!(bsn))
        })
        .map(|r| r["verzoek"]["id"].clone())
        .unwrap_or_else(|| panic!("an open request about {bsn}: {panel}"));
    let advies = json!({
        "cel": "ggd_wilgendaal",
        "feit": ADVICE,
        "verzoek": verzoek,
        "velden": {
            "datum_onderzoek": "2026-10-16",
            "langdurige_loopbeperking": true,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        },
    });
    let response = http
        .post(format!("{base}/api/backoffice/beheer/vastleggingen"))
        .header("cookie", &admin)
        .json(&advies)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    set_clock(http, base, &admin, "2026-10-20").await;
}

#[tokio::test]
async fn after_a_grant_the_source_data_hold_it_and_the_records_it_rests_on() {
    let server = demo_server(StoreChoice::Memory).await;
    let anna = server.citizen(ANNA).await;
    let (status, aanvraag) = anna
        .call(&server.http, "indieningen", &application(), None)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    let aanvraag = aanvraag["id"].as_str().unwrap().to_string();
    // While the request to the GGD is open, the source data say so.
    let data = source_data(&server, &anna, &json!({})).await;
    assert_eq!(
        data["antwoorden"]["verzoek_om_geneeskundig_onderzoek_staat_open"]["waarde"],
        json!(true),
        "{data:#}"
    );
    ggd_advises(&server.http, &server.base, ANNA).await;
    let besluit = decide_as_municipality(&server.http, &server.base, &aanvraag).await;
    let data = source_data(&server, &anna, &json!({})).await;
    assert_eq!(data["peildatum"], json!("2026-10-20"));
    let eerder = &data["antwoorden"]["aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt"];
    assert_eq!(eerder["waarde"], json!(true), "{data:#}");
    assert_eq!(
        eerder["herkomst"][0]["reductie"],
        json!("reductie_geldige_gehandicaptenparkeerkaart")
    );
    let records: Vec<&str> = data["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["id"].as_str().unwrap())
        .collect();
    assert!(records.contains(&aanvraag.as_str()), "{records:?}");
    assert!(records.contains(&besluit.as_str()), "{records:?}");
    // The provenance says which reduction answered, not how far it read the
    // municipality's chronicle: that holds everyone's records.
    assert!(eerder["herkomst"][0].get("gelezen").is_none(), "{eerder}");
    assert!(
        eerder["herkomst"][0].get("tot_en_met").is_none(),
        "{eerder}"
    );
    // Another citizen gets none of it, also when he names Anna's number.
    let bram = server.citizen(BRAM).await;
    let data = source_data(&server, &bram, &json!({ "bsn_aanvrager": ANNA })).await;
    assert_eq!(data["sub"], json!(BRAM));
    assert_eq!(data["records"], json!([]));
    assert_ne!(
        data["antwoorden"]["aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt"]["waarde"],
        json!(true)
    );
    let text = data.to_string();
    for id in &records {
        assert!(!text.contains(id), "Bram's source data name {id}: {text}");
    }
}

#[tokio::test]
async fn the_case_bundle_loads_as_a_source_without_a_session() {
    use regelrecht_poc_chronolexografie::bundel::{self, Bron, BronStand};
    let server = demo_server(StoreChoice::Memory).await;
    let response = server
        .http
        .get(format!("{}{}", server.base, fsc4b::REGELWERKEN))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bestanden: std::collections::BTreeMap<String, String> = response.json().await.unwrap();
    assert!(
        bestanden.contains_key("ui_spec.yaml"),
        "{:?}",
        bestanden.keys()
    );
    let bron = Bron {
        naam: "regelhet".into(),
        bestanden,
        ..Bron::default()
    };
    let mut stand = BronStand::default();
    let cases = bundel::uit_bron(&bron, &mut stand).unwrap();
    assert!(stand.fouten.is_empty(), "{:?}", stand.fouten);
    let ids: Vec<&str> = cases.iter().map(|c| c.casus.id.as_str()).collect();
    assert_eq!(ids, ["gehandicaptenparkeerkaart"]);
    // The signed source data name the hash of exactly this bundle.
    let anna = server.citizen(ANNA).await;
    let data = source_data(&server, &anna, &json!({})).await;
    assert_eq!(
        data["bundel_hash"],
        json!(fsc4b::bundle_hash(&bron.bestanden))
    );
}

const APP_ORIGIN: &str = "https://burgerapp.example.org";

/// A preflight for `POST` on `path` from `origin`: the origin it allows,
/// if any.
async fn preflight(server: &Server, path: &str, origin: &str) -> Option<String> {
    let response = server
        .http
        .request(reqwest::Method::OPTIONS, format!("{}{path}", server.base))
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .header("access-control-request-headers", "authorization")
        .send()
        .await
        .unwrap();
    response
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap().to_string())
}

#[tokio::test]
async fn the_channel_answers_the_configured_browser_apps_and_no_others() {
    let origins = Fsc4bOrigins::parse(APP_ORIGIN).unwrap();
    let server = demo_server_for(StoreChoice::Memory, origins).await;
    assert_eq!(
        preflight(&server, fsc4b::GEGEVENS, APP_ORIGIN)
            .await
            .as_deref(),
        Some(APP_ORIGIN)
    );
    assert_eq!(
        preflight(&server, fsc4b::CHALLENGE, APP_ORIGIN)
            .await
            .as_deref(),
        Some(APP_ORIGIN)
    );
    assert_eq!(
        preflight(&server, fsc4b::GEGEVENS, "https://elders.example.org").await,
        None
    );
    let response = server
        .http
        .get(format!("{}{}", server.base, fsc4b::REGELWERKEN))
        .header("origin", APP_ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        APP_ORIGIN
    );
    // An answer to a real request carries the header too, a refusal
    // included, so the app can read why.
    let response = server
        .http
        .post(format!("{}{}", server.base, fsc4b::GEGEVENS))
        .header("origin", APP_ORIGIN)
        .header("authorization", "geen-sessie")
        .body("geen-jwe")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        APP_ORIGIN
    );
    // So does a path under /fsc4b/ that is no route.
    let response = server
        .http
        .get(format!("{}/fsc4b/bestaat-niet", server.base))
        .header("origin", APP_ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        APP_ORIGIN
    );
    // Only the channel: a browser route of the server gets no CORS headers.
    let response = server
        .http
        .get(format!("{}/api/uiterlijk", server.base))
        .header("origin", APP_ORIGIN)
        .send()
        .await
        .unwrap();
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
    // Without origins configured, no app on another address is answered.
    let server = demo_server(StoreChoice::Memory).await;
    assert_eq!(preflight(&server, fsc4b::GEGEVENS, APP_ORIGIN).await, None);
}
