//! The app under its prefix, as the poc portal forwards to it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use regelrecht_poc_vakantieverlof::{api::AppState, kern::Corpus, router};
use serde_json::{json, Value};
use tower::ServiceExt;

fn app() -> axum::Router {
    let corpus = Corpus::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")).unwrap();
    router(
        AppState::new(corpus).unwrap(),
        "/vakantieverlof/",
        "/nonexistent",
    )
}

async fn send(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn post(uri: &str, body: Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn the_api_answers_under_the_prefix() {
    let app = app();
    let (status, vormen) = send(
        &app,
        Request::get("/vakantieverlof/api/vormen?datum=2026-10-14")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        vormen["aanvraag"]["grondslag"],
        json!("leerplichtwet_1969#13a")
    );

    let response = app
        .clone()
        .oneshot(
            Request::get("/vakantieverlof/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
}

#[tokio::test]
async fn a_refusal_is_a_422_with_the_reason_and_reset_empties_the_chronicle() {
    let app = app();
    let (status, body) = send(
        &app,
        post(
            "/vakantieverlof/api/aanvragen",
            json!({ "velden": { "bestemming": "Lissabon" }, "datum": "2026-10-13" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["fout"].as_str().unwrap().contains("bestemming"),
        "{body}"
    );

    let (status, _) = send(
        &app,
        post(
            "/vakantieverlof/api/aanvragen",
            json!({ "velden": { "bsn_jongere": "999990019" }, "datum": "2026-10-13" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let kroniek = |app: axum::Router| async move {
        send(
            &app,
            Request::get("/vakantieverlof/api/kroniek")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .1
    };
    assert_eq!(
        kroniek(app.clone()).await["grammen"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    send(&app, post("/vakantieverlof/api/reset", json!({}))).await;
    assert!(kroniek(app.clone()).await["grammen"]
        .as_array()
        .unwrap()
        .is_empty());
}
