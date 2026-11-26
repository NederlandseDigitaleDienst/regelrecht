//! The simulated login of a case that identifies an applicant by its KvK
//! number, through the login step of the case: the test source of the
//! chronolexography PoC (`tests/bron-inlogstap`), whose accredited service
//! knows the users bakker, kapper and bloemist.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{anonymous, config, digid};
use regelrecht_regelhet::app;
use regelrecht_regelhet::config::Mode;
use regelrecht_regelhet::login::Login;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn server() -> Router {
    let mut c = config(Mode::Test);
    c.manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../poc-chronolexografie/tests/bron-inlogstap/bron.yaml");
    app::build_with(&c, Login::TestLogin).await.unwrap()
}

/// Log in through the login step; the status, and the session cookie on
/// success.
async fn step_login(
    app: &Router,
    user: &str,
    password: &str,
) -> (StatusCode, Option<String>, Value) {
    let body = json!({ "gebruikersnaam": user, "wachtwoord": password });
    let request = Request::post("/auth/testlogin")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|c| c.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    (
        status,
        cookie,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn the_screen_asks_for_a_kvk_number_and_offers_the_users_of_the_step() {
    let app = server().await;
    let (status, simulation) = anonymous(&app).get("/auth/simulatie").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        simulation["identiteit"],
        json!({ "claim": "kvk_nummer", "label": "KvK-nummer", "soort": "kvk", "cijfers": 8 })
    );
    let users = simulation["inlogstap"]["gebruikersnamen"]
        .as_array()
        .unwrap();
    assert!(users.contains(&json!("bakker")), "{simulation}");
    assert_eq!(simulation["inlogstap"]["wachtwoord"], "welkom");

    // There one logs in with a user name, so no number is offered.
    let (_, personas) = anonymous(&app).get("/auth/testpersonen").await;
    assert!(
        personas
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["soort"] != "burger"),
        "{personas}"
    );
}

#[tokio::test]
async fn the_login_step_logs_the_business_in_by_its_kvk_number() {
    let app = server().await;
    let (status, cookie, _) = step_login(&app, "bakker", "welkom").await;
    assert_eq!(status, StatusCode::OK);

    let me = common::As { app: &app, cookie };
    let (status, ik) = me.get("/api/ik").await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    assert_eq!(ik["sub"], "bedrijf-99970011", "{ik}");
    assert_eq!(ik["identiteit"], "99970011", "{ik}");
}

#[tokio::test]
async fn a_refused_login_says_why() {
    let app = server().await;
    let (status, _, body) = step_login(&app, "bakker", "verkeerd").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.to_string().contains("wachtwoord"), "{body}");
    // Authorized for another service only.
    let (status, _, body) = step_login(&app, "kapper", "welkom").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.to_string().contains("geen toegang"), "{body}");
}

#[tokio::test]
async fn a_case_with_a_login_step_takes_no_typed_number() {
    let app = server().await;
    let (status, body) = anonymous(&app)
        .post(
            "/auth/testlogin",
            json!({ "nummer": "99970011", "naam": "Bakkerij" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.to_string().contains("inlogstap"), "{body}");
}

#[tokio::test]
async fn a_case_on_bsn_keeps_the_simulated_digid() {
    let app = common::server().await;
    let (_, simulation) = anonymous(&app).get("/auth/simulatie").await;
    assert_eq!(simulation["identiteit"]["label"], "BSN");
    assert_eq!(simulation["inlogstap"], Value::Null);
    // `nummer` and, as before, `bsn` both name the number.
    digid(&app, common::ANNA, "Anna Voorbeeld").await;
    let (status, _) = anonymous(&app)
        .post(
            "/auth/testlogin",
            json!({ "nummer": common::BRAM, "naam": "Bram Proef" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_claim_setting_other_than_the_case_key_does_not_start() {
    let mut c = config(Mode::Test);
    c.manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../poc-chronolexografie/tests/bron-inlogstap/bron.yaml");
    c.subject_claim = Some("bsn".into());
    let e = app::build_with(&c, Login::TestLogin).await.unwrap_err();
    assert!(e.to_string().contains("kvk_nummer"), "{e}");
}

#[tokio::test]
async fn the_form_fills_the_kvk_number_from_the_login_and_does_not_ask_it() {
    let app = server().await;
    let (status, cookie, _) = step_login(&app, "bakker", "welkom").await;
    assert_eq!(status, StatusCode::OK);
    let me = common::As { app: &app, cookie };
    let (status, forms) = me.get("/api/formulier").await;
    assert_eq!(status, StatusCode::OK, "{forms}");
    let application = &forms["indieningen"]["voorbeeld_vergunning_inlogstap#1"];
    assert!(
        application["velden"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["naam"] == "kvk_nummer"),
        "{application}"
    );
    assert_eq!(
        application["systeem"]["kvk_nummer"]["soort"], "inloggen",
        "{application}"
    );
}
