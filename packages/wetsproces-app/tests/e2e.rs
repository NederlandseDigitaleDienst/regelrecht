//! The app's core against a real wetsproces server in test mode, on the
//! demo Stelsel, over HTTP: activate, start a session, read the forms,
//! submit, read the dossier and ask why. No window; these are the commands
//! the webview calls.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_fsc4b::jose::SigningKey;
use regelrecht_wetsproces::app;
use regelrecht_wetsproces::config::{
    Config, DEFAULT_MANIFEST, DEFAULT_PORT, DEFAULT_SUBJECT_CLAIM, Fsc4bChoice, LoginChoice, Mode,
    StoreChoice,
};
use regelrecht_wetsproces::fsc4b::testing::decide_as_municipality;
use regelrecht_wetsproces::login::Login;
use regelrecht_wetsproces::surface::Surface;
use regelrecht_wetsproces_app::keystore::KeyStore;
use regelrecht_wetsproces_app::{AppConfig, AppCore, CommandError, SoftwareKeyStore};
use serde_json::{Value, json};
use tokio::net::TcpListener;

const BSN: &str = "999990032";

/// A server in test mode on `127.0.0.1`, with or without the demo Stelsel;
/// its address.
async fn server(demo: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    serve(listener, &base, demo).await;
    base
}

/// The server on `listener`, reached at `base`, until the task is aborted.
/// Every answer closes its connection, so no connection outlives a restart.
async fn serve(listener: TcpListener, base: &str, demo: bool) -> tokio::task::JoinHandle<()> {
    let base = base.to_string();
    let fsc4b = if demo {
        Fsc4bChoice::Demo {
            public_url: base.clone(),
        }
    } else {
        Fsc4bChoice::Off
    };
    let config = Config {
        mode: Mode::Test,
        surface: Surface::All,
        store: StoreChoice::Memory,
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        frontend: PathBuf::from("/nonexistent/frontend"),
        port: DEFAULT_PORT,
        login: LoginChoice::TestLogin,
        subject_claim: DEFAULT_SUBJECT_CLAIM.into(),
        fsc4b,
        accent: None,
        link: Default::default(),
    };
    let router = app::build_with(&config, Login::TestLogin)
        .await
        .unwrap()
        .layer(axum::middleware::map_response(
            |mut response: axum::response::Response| async move {
                response.headers_mut().insert(
                    axum::http::header::CONNECTION,
                    axum::http::HeaderValue::from_static("close"),
                );
                response
            },
        ));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    })
}

fn core(base: &str, store: Arc<SoftwareKeyStore>) -> AppCore {
    regelrecht_wetsproces_app::install_crypto_provider();
    AppCore::build(AppConfig::build(base, None), store).unwrap()
}

fn application() -> Value {
    json!({ "velden": {
        "kaart_als_bestuurder": true,
        "kaart_als_passagier": false,
        "adres_aanvrager": "Voorbeeldlaan 1",
    }})
}

fn ids(dossier: &Value) -> Vec<&str> {
    dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        // Not the facts recorded beside them, such as the request to the GGD
        // an application brings about.
        .filter(|g| g["kind"] != "EXECUTOGRAM")
        .map(|g| g["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn the_core_runs_the_whole_path_against_a_demo_server() {
    let base = server(true).await;
    let app = core(&base, Arc::new(SoftwareKeyStore::in_memory()));

    let before = app.status().await;
    assert_eq!(before.holder, None);
    assert_eq!(before.server.as_deref(), Some(base.as_str()));
    assert!(!before.hardware_backed);
    assert_eq!(app.forms().await, Err(CommandError::NotActivated));

    let status = app.activate(BSN).await.unwrap();
    assert_eq!(status.holder.as_ref().map(|h| h.bsn.as_str()), Some(BSN));
    // Nothing the screens receive carries the credential or a key.
    let shown = serde_json::to_string(&status).unwrap();
    assert!(!shown.contains("eyJ"), "{shown}");

    let case = app.case().await.unwrap();
    assert!(case["vandaag"].is_string(), "{case}");
    let forms = app.forms().await.unwrap();
    assert!(forms.is_object(), "{forms}");

    let submitted = app.submit(&application(), "aanvraag-1").await.unwrap();
    let id = submitted["id"].as_str().unwrap().to_string();
    assert_eq!(submitted["fields"]["bsn_aanvrager"], json!(BSN));
    // The same key again: answered from the receipt, recorded once.
    assert_eq!(
        app.submit(&application(), "aanvraag-1").await.unwrap(),
        submitted
    );

    let dossier = app.dossier().await.unwrap();
    assert!(ids(&dossier).contains(&id.as_str()), "{dossier}");

    // A decider of the municipality decides; the app asks why.
    let decision = decide_as_municipality(&reqwest::Client::new(), &base, &id).await;
    let why = app.explain(&decision).await.unwrap();
    assert!(!why["ingevuld"].as_array().unwrap().is_empty(), "{why}");
    assert!(ids(&app.dossier().await.unwrap()).contains(&decision.as_str()));
    // A decision of someone else's dossier is not explained.
    assert!(matches!(
        app.explain("not-mine").await,
        Err(CommandError::Service { status: 404, .. })
    ));

    // The service's refusal reaches the screens with its status and body.
    let mut forged = application();
    forged["velden"]["bsn_aanvrager"] = json!("999990020");
    match app.submit(&forged, "aanvraag-2").await {
        Err(CommandError::Service { status: 422, body }) => {
            assert!(body["fout"].is_string(), "{body}");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_lapsed_session_is_renewed_without_the_request_being_lost() {
    let base = server(true).await;
    let app = core(&base, Arc::new(SoftwareKeyStore::in_memory()));
    app.activate(BSN).await.unwrap();
    app.spoil_session().await;
    // The first request under the spoiled token is refused, a new session
    // starts, and the submission goes out as it was: recorded once.
    let first = app.submit(&application(), "aanvraag-1").await.unwrap();
    assert_eq!(app.sessions_created(), 2);
    assert_eq!(
        ids(&app.dossier().await.unwrap()),
        [first["id"].as_str().unwrap()]
    );
}

#[tokio::test]
async fn concurrent_commands_share_one_session() {
    let base = server(true).await;
    let app = Arc::new(core(&base, Arc::new(SoftwareKeyStore::in_memory())));
    app.activate(BSN).await.unwrap();
    app.spoil_session().await;
    let (a, b, c) = tokio::join!(app.case(), app.forms(), app.dossier());
    assert!(a.is_ok() && b.is_ok() && c.is_ok(), "{a:?} {b:?} {c:?}");
    // One at activation, one shared by the three after the spoiled one.
    assert_eq!(app.sessions_created(), 2);
}

#[tokio::test]
async fn a_restarted_demo_server_sends_the_citizen_back_to_activation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let base = format!("http://{address}");
    let first = serve(listener, &base, true).await;
    let app = core(&base, Arc::new(SoftwareKeyStore::in_memory()));
    app.activate(BSN).await.unwrap();
    // A new start makes new keys: the old credential is from an
    // Activatiedienst the new server does not know, and the core read the
    // organization's old key. It reads the Stelsel anew, and then the
    // credential is refused, which sends the citizen to activation.
    first.abort();
    let _ = first.await;
    serve(TcpListener::bind(address).await.unwrap(), &base, true).await;
    app.spoil_session().await;
    assert_eq!(app.dossier().await, Err(CommandError::CredentialRefused));
    app.activate(BSN).await.unwrap();
    assert!(app.dossier().await.is_ok());
}

#[tokio::test]
async fn a_lapsed_credential_sends_the_citizen_back_to_activation() {
    let base = server(true).await;
    let store = Arc::new(SoftwareKeyStore::in_memory());
    let app = core(&base, Arc::clone(&store));
    app.activate(BSN).await.unwrap();
    // A credential for the same key that has lapsed: the core says so before
    // the server is asked.
    let signer = store.signer().unwrap();
    let lapsed = SigningKey::generate()
        .unwrap()
        .sign(
            &json!({ "cnf": { "jwk": signer.public_jwk() }, "bsn": BSN, "exp": 1 }),
            &[],
        )
        .unwrap();
    store.store_credential(&lapsed).unwrap();
    app.spoil_session().await;
    assert_eq!(app.dossier().await, Err(CommandError::CredentialExpired));
    let status = app.status().await;
    assert!(status.credential_expired);
    assert_eq!(status.holder, None);
}

#[tokio::test]
async fn a_server_without_the_demo_is_named_as_such() {
    let base = server(false).await;
    let app = core(&base, Arc::new(SoftwareKeyStore::in_memory()));
    assert!(matches!(
        app.activate(BSN).await,
        Err(CommandError::NoDemo { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_server_is_a_network_error() {
    // A port that was free a moment ago and has no listener now.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let app = core(&base, Arc::new(SoftwareKeyStore::in_memory()));
    assert!(matches!(
        app.activate(BSN).await,
        Err(CommandError::Network { .. })
    ));
}

#[tokio::test]
async fn only_the_offered_test_numbers_activate() {
    let base = server(true).await;
    let app = core(&base, Arc::new(SoftwareKeyStore::in_memory()));
    for bsn in ["999990018", "123456782", ""] {
        assert!(
            matches!(app.activate(bsn).await, Err(CommandError::Input { .. })),
            "{bsn}"
        );
    }
}

#[tokio::test]
async fn without_a_server_address_the_app_says_it_is_not_configured() {
    let app = AppCore::build(
        AppConfig::from_lookup(|_| None),
        Arc::new(SoftwareKeyStore::in_memory()),
    )
    .unwrap();
    assert!(app.status().await.config_problem.is_some());
    assert!(matches!(
        app.activate(BSN).await,
        Err(CommandError::NotConfigured { .. })
    ));
}
