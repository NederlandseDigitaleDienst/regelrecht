//! An organisation in front of its processes: the statement a front signs,
//! how a process takes it (a hand-off of a browser, a call with it in the
//! header), the role a person keeps on the way, and the front's own routes,
//! up to a hand-off from a tile into a process that runs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::*;
use regelrecht_fsc4b::jose::SigningKey;
use regelrecht_regelhet::app;
use regelrecht_regelhet::config::{
    FrontSettings, Membership, Mode, OrganisationKey, Role, StoreChoice,
};
use regelrecht_regelhet::login::Login;
use regelrecht_regelhet::organisatie::statement::{Signer, Statement};
use regelrecht_regelhet::store::Channel;
use regelrecht_regelhet::surface::Surface;
use regelrecht_shared::postgres::test_utils::TestDatabase;
use serde_json::{Value, json};
use tower::ServiceExt;

const PROCESS: &str = "tnt-p/parkeerkaart";
const ORGANISATION: &str = "tnt-o/wilgendaal";

fn now() -> i64 {
    regelrecht_regelhet::fsc4b::now_unix()
}

/// A fresh key of the organisation, and a copy of it for the signer.
fn organisation_key() -> (SigningKey, Signer) {
    let key = SigningKey::generate().unwrap();
    let copy = SigningKey::from_pkcs8_pem(&key.to_pkcs8_pem().unwrap()).unwrap();
    (key, Signer::new(copy, ORGANISATION))
}

fn membership(key: &SigningKey) -> Membership {
    Membership {
        process_id: PROCESS.into(),
        organisation_id: ORGANISATION.into(),
        name: "Gemeente Wilgendaal".into(),
        url: Some("https://wilgendaal-medewerkers.example.org".into()),
        key: OrganisationKey::Jwk(serde_json::to_string(key.public_jwk()).unwrap()),
    }
}

/// The parking card process on `surface`, belonging to the organisation of
/// `key`.
async fn process(key: &SigningKey, surface: Surface) -> Router {
    let mut config = config_on(Mode::Test, surface);
    config.role = Role::Process(Some(membership(key)));
    app::build_with(&config, Login::TestLogin).await.unwrap()
}

fn decider() -> Statement {
    Statement {
        sub: "beslisser-1".into(),
        roles: BTreeSet::from(["regelhet-beslisser".to_string()]),
        claims: BTreeMap::new(),
        name: Some("Bea Beslisser".into()),
        login: Channel::TestLogin,
        mode: Mode::Test,
    }
}

fn anna() -> Statement {
    Statement {
        sub: format!("burger-{ANNA}"),
        roles: BTreeSet::from(["regelhet-burger".to_string()]),
        claims: BTreeMap::from([("bsn".to_string(), json!(ANNA))]),
        name: Some("Anna Voorbeeld".into()),
        login: Channel::TestLogin,
        mode: Mode::Test,
    }
}

fn backoffice() -> String {
    format!("{PROCESS}#backoffice")
}

/// `GET path` on `app` with `headers`: the status, the headers and the body.
async fn get(
    app: &Router,
    path: &str,
    headers: &[(&str, String)],
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder().uri(path);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The hand-off of `token` to `naar`: the status, and the session's cookie
/// with where it goes when it succeeded.
async fn hand_off(app: &Router, token: &str, naar: &str) -> (StatusCode, Option<(String, String)>) {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("token", token)
        .append_pair("naar", naar)
        .finish();
    let (status, headers, _) = get(app, &format!("/auth/organisatie?{query}"), &[]).await;
    let session = headers.get(header::SET_COOKIE).map(|c| {
        let cookie = c.to_str().unwrap().split(';').next().unwrap().to_string();
        let location = headers[header::LOCATION].to_str().unwrap().to_string();
        (cookie, location)
    });
    (status, session.filter(|_| status == StatusCode::SEE_OTHER))
}

#[tokio::test]
async fn a_hand_off_makes_a_session_with_the_role_the_organisation_gave() {
    let (key, signer) = organisation_key();
    let app = process(&key, Surface::Backoffice).await;
    let token = signer.sign(&decider(), &backoffice(), now()).unwrap();
    let (status, session) = hand_off(&app, &token, "/backoffice/kroniek").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (cookie, location) = session.unwrap();
    assert_eq!(location, "/backoffice/kroniek");
    let me = As {
        app: &app,
        cookie: Some(cookie),
    };
    let (status, ik) = me.get("/api/ik").await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    // One ladder for the organisation: a decider at the front decides here.
    assert_eq!(ik["niveau"], json!("beslisser"));
    assert_eq!(ik["sub"], json!(format!("{ORGANISATION}:beslisser-1")));
    assert_eq!(ik["naam"], json!("Bea Beslisser"));
    assert_eq!(ik["cellen"][0]["beslissen"], json!(true));
}

#[tokio::test]
async fn a_hand_off_is_refused_when_expired_signed_with_another_key_or_for_another_process() {
    let (key, signer) = organisation_key();
    let app = process(&key, Surface::Backoffice).await;
    let expired = signer.sign(&decider(), &backoffice(), now() - 600).unwrap();
    let (_, other) = organisation_key();
    let foreign = other.sign(&decider(), &backoffice(), now()).unwrap();
    let elsewhere = signer
        .sign(&decider(), "tnt-p/andere-regeling#backoffice", now())
        .unwrap();
    let other_side = signer
        .sign(&decider(), &format!("{PROCESS}#burger"), now())
        .unwrap();
    for (what, token) in [
        ("expired", expired),
        ("another key", foreign),
        ("another process", elsewhere),
        ("a side the instance does not serve", other_side),
        ("no statement", "abc.def.ghi".to_string()),
    ] {
        let (status, session) = hand_off(&app, &token, "/backoffice").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{what}");
        assert!(session.is_none(), "{what}");
    }
}

#[tokio::test]
async fn a_statement_is_taken_once_and_leads_only_to_a_path_here() {
    let (key, signer) = organisation_key();
    let app = process(&key, Surface::Backoffice).await;
    let token = signer.sign(&decider(), &backoffice(), now()).unwrap();
    for naar in [
        "//evil.example.org/",
        "/\\evil.example.org",
        "https://evil.example.org",
    ] {
        let (status, _) = hand_off(&app, &token, naar).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{naar}");
    }
    let (status, _) = hand_off(&app, &token, "/backoffice").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (status, session) = hand_off(&app, &token, "/backoffice").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(session.is_none());
}

#[tokio::test]
async fn the_citizens_role_does_not_reach_the_back_office() {
    let (key, signer) = organisation_key();
    let app = process(&key, Surface::Backoffice).await;
    // Signed for the back office, but carrying only the citizen's role.
    let token = signer.sign(&anna(), &backoffice(), now()).unwrap();
    let (status, session) = hand_off(&app, &token, "/backoffice").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(session.is_none());
    // A decider's role signed for the citizens' side stays behind there.
    let all = process(&key, Surface::All).await;
    let token = signer
        .sign(&decider(), &format!("{PROCESS}#burger"), now())
        .unwrap();
    let (status, session) = hand_off(&all, &token, "/burger").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no citizen, no employee");
    assert!(session.is_none());
    // A citizen who is also a decider arrives on the citizens' side as a
    // citizen only.
    let mut both = anna();
    both.roles.insert("regelhet-beslisser".into());
    let token = signer
        .sign(&both, &format!("{PROCESS}#burger"), now())
        .unwrap();
    let (status, session) = hand_off(&all, &token, "/burger").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let me = As {
        app: &all,
        cookie: Some(session.unwrap().0),
    };
    let (_, ik) = me.get("/api/ik").await;
    assert_eq!(ik["burger"], json!(true));
    assert_eq!(ik["niveau"], Value::Null);
    let (status, _) = me.get("/api/backoffice/werkvoorraad").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_statement_in_the_header_reads_but_does_not_act() {
    let (key, signer) = organisation_key();
    let app = process(&key, Surface::Backoffice).await;
    let header = |token: String| vec![("authorization", format!("Organisatie {token}"))];
    let token = signer.sign(&decider(), &backoffice(), now()).unwrap();
    let (status, _, queue) = get(&app, "/api/backoffice/werkvoorraad", &header(token)).await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    // Without a statement, or with one taken before, nobody is in.
    let (status, _, _) = get(&app, "/api/backoffice/werkvoorraad", &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let token = signer.sign(&decider(), &backoffice(), now()).unwrap();
    let (status, _, _) = get(&app, "/api/ik", &header(token.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = get(&app, "/api/ik", &header(token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // A POST with a statement is refused before anything is recorded.
    let token = signer.sign(&decider(), &backoffice(), now()).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/backoffice/vastleggingen")
                .header("authorization", format!("Organisatie {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_process_without_an_organisation_has_no_hand_off() {
    let app = server().await;
    let (status, _, _) = get(&app, "/auth/organisatie?token=x&naar=/", &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, uiterlijk) = get(&app, "/api/uiterlijk", &[]).await;
    assert_eq!(uiterlijk["organisatie"], Value::Null);
}

#[tokio::test]
async fn the_process_names_its_organisation_for_the_link_back() {
    let (key, _) = organisation_key();
    let app = process(&key, Surface::Backoffice).await;
    let (_, _, uiterlijk) = get(&app, "/api/uiterlijk", &[]).await;
    assert_eq!(
        uiterlijk["organisatie"],
        json!({
            "naam": "Gemeente Wilgendaal",
            "url": "https://wilgendaal-medewerkers.example.org",
        })
    );
}

#[tokio::test]
async fn what_a_citizen_handed_over_submits_is_kept_as_through_the_organisation() {
    let database = TestDatabase::create().await;
    let (key, signer) = organisation_key();
    let mut config = config_on(Mode::Test, Surface::Citizen);
    config.store = StoreChoice::postgres(database.url().to_string());
    config.role = Role::Process(Some(membership(&key)));
    let app = app::build_with(&config, Login::TestLogin).await.unwrap();
    let token = signer
        .sign(&anna(), &format!("{PROCESS}#burger"), now())
        .unwrap();
    let (status, session) = hand_off(&app, &token, "/burger#nieuwe-aanvraag").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (cookie, location) = session.unwrap();
    assert_eq!(location, "/burger#nieuwe-aanvraag");
    let anna = As {
        app: &app,
        cookie: Some(cookie),
    };
    let (status, aanvraag) = anna.post("/api/indieningen", application()).await;
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    let pool = sqlx::PgPool::connect(database.url()).await.unwrap();
    let kept: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT recorded_via, recorded_by FROM wetsproces.grams WHERE recorded_via <> 'beleid'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!kept.is_empty());
    for (via, by) in kept {
        assert_eq!(via.as_deref(), Some("organisatie"));
        assert_eq!(by, Some(format!("{ORGANISATION}:burger-{ANNA}")));
    }
}

/// A front of the organisation on both sides, before the processes in
/// `processes`, signing with `key`.
async fn front(key: &SigningKey, processes: &str) -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("processen.yaml");
    std::fs::write(&file, processes).unwrap();
    let key_file = dir.path().join("sleutel.pem");
    std::fs::write(&key_file, key.to_pkcs8_pem().unwrap()).unwrap();
    let mut config = config_on(Mode::Test, Surface::All);
    config.role = Role::Organisation(FrontSettings {
        id: ORGANISATION.into(),
        name: "Gemeente Wilgendaal".into(),
        logo: None,
        processes: file,
        key: Some(key_file),
    });
    config.frontend = PathBuf::from("/nonexistent/frontend");
    (
        app::build_with(&config, Login::TestLogin).await.unwrap(),
        dir,
    )
}

/// The file of processes with the parking card at `intern`.
fn parking_card_at(intern: &str) -> String {
    format!(
        "processen:
  - id: {PROCESS}
    naam: parkeerkaart
    burger: {{ intern: '{intern}', publiek: 'https://parkeerkaart.example.org' }}
    backoffice: {{ intern: '{intern}', publiek: 'https://parkeerkaart-medewerkers.example.org' }}
"
    )
}

/// `app` served on a free port of the loopback, for the front to call.
async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("http://{address}")
}

#[tokio::test]
async fn a_tile_opens_the_process_without_a_second_login() {
    let (key, _) = organisation_key();
    let process_app = process(&key, Surface::All).await;
    let intern = serve(process_app.clone()).await;
    let (front, _dir) = front(&key, &parking_card_at(&intern)).await;
    let bea = login(&front, "beslisser-1", &["regelhet-beslisser"], json!({})).await;

    // The tile: the process, with the title of its case.
    let (status, overview) = bea.get("/api/organisatie").await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["naam"], json!("Gemeente Wilgendaal"));
    assert_eq!(overview["kant"], json!("backoffice"));
    assert_eq!(overview["processen"][0]["id"], json!(PROCESS));
    assert_eq!(overview["processen"][0]["bereikbaar"], json!(true));

    // The front passes a listed route on, as the decider.
    let id = url::form_urlencoded::byte_serialize(PROCESS.as_bytes()).collect::<String>();
    let (status, queue) = bea
        .get(&format!("/api/processen/{id}/api/backoffice/werkvoorraad"))
        .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    let (status, _) = bea
        .get(&format!(
            "/api/processen/{id}/api/backoffice/beheer/partijen"
        ))
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a route the front does not pass on"
    );
    let (status, _) = bea.get("/api/processen/tnt-x%2Fonbekend/api/casus").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an unknown process");

    // The click: a 303 to the process, which takes the statement.
    let (status, headers, _) = get(
        &front,
        &format!("/api/processen/{id}/openen?naar=/backoffice"),
        &[("cookie", bea.cookie.clone().unwrap())],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = headers[header::LOCATION].to_str().unwrap();
    let path = location
        .strip_prefix("https://parkeerkaart-medewerkers.example.org")
        .expect("the process's own address for employees");
    let (status, headers, _) = get(&process_app, path, &[]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/backoffice");
}

#[tokio::test]
async fn the_realms_own_roles_stay_at_the_front() {
    let (key, _) = organisation_key();
    let process_app = process(&key, Surface::All).await;
    let intern = serve(process_app).await;
    let (front, _dir) = front(&key, &parking_card_at(&intern)).await;
    // A session as Keycloak fills it: the realm's own roles beside ours.
    let bea = login(
        &front,
        "beslisser-1",
        &[
            "regelhet-beslisser",
            "offline_access",
            "default-roles-voorbeeld",
        ],
        json!({}),
    )
    .await;
    let id = url::form_urlencoded::byte_serialize(PROCESS.as_bytes()).collect::<String>();
    let (status, ik) = bea.get(&format!("/api/processen/{id}/api/ik")).await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    assert_eq!(ik["niveau"], json!("beslisser"));
    // The name travels in the statement, for a call without a session too.
    assert_eq!(ik["naam"], json!("beslisser-1"));
}

#[tokio::test]
async fn a_process_that_does_not_answer_is_shown_as_such() {
    let (key, _) = organisation_key();
    // Port 1 on the loopback: nothing listens.
    let (front, _dir) = front(&key, &parking_card_at("http://127.0.0.1:1")).await;
    let anna = digid(&front, ANNA, "Anna Voorbeeld").await;
    let (status, overview) = anna.get("/api/organisatie").await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["kant"], json!("burger"));
    assert_eq!(overview["processen"][0]["bereikbaar"], json!(false));
    let id = url::form_urlencoded::byte_serialize(PROCESS.as_bytes()).collect::<String>();
    let (status, _) = anna.get(&format!("/api/processen/{id}/api/dossier")).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    // Without a login the front gives nothing of the processes.
    let (status, _) = anonymous(&front).get("/api/organisatie").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_identifier_in_the_statement_picks_the_dossier_a_citizen_gets_through_the_front() {
    let (key, _) = organisation_key();
    let process_app = process(&key, Surface::All).await;
    let aanvraag = anna_applies(&process_app).await;
    let intern = serve(process_app).await;
    let (front, _dir) = front(&key, &parking_card_at(&intern)).await;
    let id = url::form_urlencoded::byte_serialize(PROCESS.as_bytes()).collect::<String>();
    let path = format!("/api/processen/{id}/api/dossier");
    let ids = |dossier: &Value| -> Vec<Value> {
        dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["id"].clone())
            .collect()
    };

    let (status, dossier) = digid(&front, ANNA, "Anna Voorbeeld").await.get(&path).await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    assert!(ids(&dossier).contains(&aanvraag["id"]), "{dossier}");
    let (status, dossier) = digid(&front, BRAM, "Bram Proef").await.get(&path).await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    assert!(ids(&dossier).is_empty(), "{dossier}");
    // A citizen whose login carries another identifier than the process's
    // own: the process refuses, and the front passes that on as it is.
    let bedrijf = login(
        &front,
        "bedrijf-1",
        &["regelhet-burger"],
        json!({ "kvk_nummer": "12345678" }),
    )
    .await;
    let (status, _) = bedrijf.get(&path).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_front_offers_its_key_and_the_accounts_of_its_processes() {
    let (key, _) = organisation_key();
    let process_app = process(&key, Surface::All).await;
    let intern = serve(process_app).await;
    let (front, _dir) = front(&key, &parking_card_at(&intern)).await;
    let (status, _, jwk) = get(&front, "/organisatie/v1/sleutel", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(jwk["kid"], json!(key.public_jwk().thumbprint()));
    assert_eq!(jwk["x"], json!(key.public_jwk().x));
    let (_, _, personas) = get(&front, "/auth/testpersonen", &[]).await;
    let subs: Vec<&str> = personas
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["sub"].as_str())
        .collect();
    // The process's citizens and the front's own are the same, once each.
    assert_eq!(
        subs.iter().filter(|s| s.starts_with("burger-")).count(),
        2,
        "{subs:?}"
    );
    assert!(subs.contains(&"beslisser-gemeente_wilgendaal"), "{subs:?}");
    assert!(subs.contains(&"beheerder"), "{subs:?}");
}
