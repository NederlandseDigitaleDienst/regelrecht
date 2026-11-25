//! The draft explanation of a decision by Vlam, against a stand-in for Vlam's
//! OpenAI-compatible API on a local port: who may ask for it, what reaches
//! Vlam, and that the feature is off without a key.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use common::*;
use regelrecht_regelhet::app;
use regelrecht_regelhet::config::Mode;
use regelrecht_regelhet::login::Login;
use regelrecht_regelhet::vlam::VlamConfig;
use serde_json::{Value, json};

const KEY: &str = "test-key-not-real";
const DRAFT: &str = "U krijgt een gehandicaptenparkeerkaart.";

/// What the stand-in received: the authorization header and the body.
type Seen = Arc<Mutex<Vec<(String, Value)>>>;

/// A stand-in for Vlam that answers every chat completion with `answer` and
/// keeps what it received; its base URL, up to and including `/v1`.
async fn stand_in(status: StatusCode, answer: Value) -> (String, Seen) {
    let seen: Seen = Arc::default();
    let keep = Arc::clone(&seen);
    let vlam = Router::new().route(
        "/v1/chat/completions",
        post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let keep = Arc::clone(&keep);
            let answer = answer.clone();
            async move {
                let auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                keep.lock().unwrap().push((auth, body));
                (status, Json(answer))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, vlam).await.unwrap() });
    (format!("http://{addr}/v1"), seen)
}

/// A stand-in that answers every chat completion with a redirect to
/// `target`, as a 307 does: the same request again, to another address.
async fn redirect_to(target: String) -> (String, ()) {
    let vlam = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let target = target.clone();
            async move { (StatusCode::TEMPORARY_REDIRECT, [("location", target)]) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, vlam).await.unwrap() });
    (format!("http://{addr}/v1"), ())
}

fn completion(text: &str) -> Value {
    json!({ "choices": [{ "message": { "role": "assistant", "content": text } }] })
}

async fn server_with_vlam(base_url: String) -> Router {
    let mut config = config(Mode::Test);
    config.vlam = Some(VlamConfig {
        base_url,
        api_key: KEY.into(),
        model: "test-model".into(),
    });
    app::build_with(&config, Login::TestLogin).await.unwrap()
}

/// Anna applies and the decider decides; the id of the decision.
async fn a_decision(app: &Router) -> String {
    let aanvraag = anna_applies(app).await;
    let (status, besluit) = employee(app, "gemeente-1", "regelhet-beslisser")
        .await
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    besluit["besluit"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn a_decider_gets_a_draft_and_vlam_sees_no_personal_data() {
    let (base, seen) = stand_in(StatusCode::OK, completion(DRAFT)).await;
    let app = server_with_vlam(base).await;
    let id = a_decision(&app).await;

    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (status, ik) = beslisser.get("/api/ik").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ik["concepttoelichting"], true);

    let (status, antwoord) = beslisser
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{antwoord}");
    assert_eq!(antwoord["concept"], DRAFT);
    assert_eq!(antwoord["model"], "test-model");

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let (auth, body) = &seen[0];
    assert_eq!(auth, &format!("Bearer {KEY}"));
    assert_eq!(body["model"], "test-model");
    let sent = body["messages"][1]["content"].as_str().unwrap();
    assert!(
        sent.contains("uitkomst") && sent.contains("waarom"),
        "{sent}"
    );
    assert!(
        !sent.contains(ANNA),
        "the citizen number reached Vlam: {sent}"
    );
    assert!(!sent.contains("Voorbeeld"), "the name reached Vlam: {sent}");
    // Vlam reads the findings with their values, and each step of the chain
    // with the term it is about.
    let input: Value = serde_json::from_str(sent).unwrap();
    let bevindingen = input["waarom"]["bevindingen"].as_object().unwrap();
    assert!(!bevindingen.is_empty(), "{sent}");
    assert!(bevindingen.values().all(Value::is_boolean), "{sent}");
    assert!(
        sent.contains(r#""soort":"begrip""#) && sent.contains(r#""naam":""#),
        "a step lost its term: {sent}"
    );
}

#[tokio::test]
async fn a_reader_and_a_citizen_get_no_draft() {
    let (base, seen) = stand_in(StatusCode::OK, completion(DRAFT)).await;
    let app = server_with_vlam(base).await;
    let id = a_decision(&app).await;

    let lezer = employee(&app, "gemeente-2", "regelhet-lezer").await;
    let (_, ik) = lezer.get("/api/ik").await;
    assert_eq!(ik["concepttoelichting"], false);
    let (status, _) = lezer
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = citizen(&app, "anna", ANNA)
        .await
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert!(status.is_client_error(), "{status}");
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_unknown_decision_and_a_failing_vlam_answer_with_an_error() {
    let (base, _) = stand_in(StatusCode::OK, completion(DRAFT)).await;
    let app = server_with_vlam(base).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (status, _) = beslisser
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": "00000000-0000-0000-0000-000000000000" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (base, _) = stand_in(StatusCode::TOO_MANY_REQUESTS, json!({})).await;
    let app = server_with_vlam(base).await;
    let id = a_decision(&app).await;
    let (status, fout) = employee(&app, "gemeente-1", "regelhet-beslisser")
        .await
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{fout}");
    assert!(!fout.to_string().contains(KEY), "{fout}");
}

#[tokio::test]
async fn a_redirect_is_not_followed_and_an_oversized_answer_is_not_read() {
    // A redirect would carry the decision and the key to another host.
    let (elsewhere, seen_elsewhere) = stand_in(StatusCode::OK, completion(DRAFT)).await;
    let (base, ()) = redirect_to(format!("{elsewhere}/chat/completions")).await;
    let app = server_with_vlam(base).await;
    let id = a_decision(&app).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (status, fout) = beslisser
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{fout}");
    assert!(seen_elsewhere.lock().unwrap().is_empty());

    let (base, _) = stand_in(StatusCode::OK, completion(&"x".repeat(300 * 1024))).await;
    let app = server_with_vlam(base).await;
    let id = a_decision(&app).await;
    let (status, fout) = employee(&app, "gemeente-1", "regelhet-beslisser")
        .await
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{fout}");
    assert!(fout.to_string().contains("larger than"), "{fout}");
}

#[tokio::test]
async fn without_a_key_there_is_no_draft_and_the_process_runs_on() {
    let app = server().await;
    let id = a_decision(&app).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (_, ik) = beslisser.get("/api/ik").await;
    assert_eq!(ik["concepttoelichting"], false);
    let (status, _) = beslisser
        .post(
            "/api/backoffice/concepttoelichting",
            json!({ "besluit": id }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
