//! The parking card process through the HTTP API, in test mode: an
//! application, the GGD's advice, the decision, and what the chronicles
//! hold after, with the clock set between the steps.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use regelrecht_wetsproces::app;
use regelrecht_wetsproces::config::{Config, DEFAULT_MANIFEST, DEFAULT_PORT, Mode};
use serde_json::{Value, json};
use tower::ServiceExt;

const ADVICE: &str =
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";

async fn server() -> Router {
    app::build(&Config {
        mode: Mode::Test,
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        port: DEFAULT_PORT,
    })
    .await
    .unwrap()
}

/// `method` on `path` with `body` and, if given, an idempotency key: the
/// status and the JSON answer.
async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
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
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn set_clock(app: &Router, day: &str) {
    let (status, klok) = call(
        app,
        Method::PUT,
        "/api/klok",
        Some(json!({ "datum": day })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{klok}");
    assert_eq!(klok["datum"], json!(day));
}

async fn head(app: &Router, cell: &str) -> u64 {
    let (status, kroniek) = call(
        app,
        Method::GET,
        &format!("/api/cellen/{cell}/kroniek"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    kroniek["kop"].as_u64().unwrap()
}

fn application(bsn: &str) -> Value {
    json!({
        "velden": {
            "bsn_aanvrager": bsn,
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

#[tokio::test]
async fn health_case_and_forms() {
    let app = server().await;
    let request = Request::get("/health").body(Body::empty()).unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
    let (status, casus) = call(&app, Method::GET, "/api/casus", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(casus["id"], json!("gehandicaptenparkeerkaart"));
    assert_eq!(casus["vandaag"], json!("2026-10-13"));
    let (status, vormen) = call(&app, Method::GET, "/api/formulier", None, None).await;
    assert_eq!(status, StatusCode::OK, "{vormen}");
    let aanvraag =
        &vormen["indieningen"]["besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49"];
    assert_eq!(aanvraag["soort"], json!("AANVRAAG"), "{vormen}");
    // What the UI spec says of the fields is there, empty for a case
    // without one.
    assert!(aanvraag["prefill"].is_object(), "{aanvraag}");
    let (status, klok) = call(&app, Method::GET, "/api/klok", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        klok,
        json!({ "datum": "2026-10-13", "modus": "test", "bestuurbaar": true })
    );
}

#[tokio::test]
async fn an_application_the_ggd_advises_on_is_granted() {
    let app = server().await;
    let (status, aanvraag) = call(
        &app,
        Method::POST,
        "/api/indieningen",
        Some(application("999990032")),
        Some("aanvraag-1"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    assert_eq!(aanvraag["kind"], json!("AANVRAAG"));
    assert_eq!(aanvraag["recorded_on"], json!("2026-10-13"));
    let op = aanvraag["id"].as_str().unwrap().to_string();

    set_clock(&app, "2026-10-16").await;
    let (status, advies) = call(
        &app,
        Method::POST,
        "/api/vastleggingen",
        Some(json!({
            "cel": "ggd_wilgendaal",
            "feit": ADVICE,
            "velden": {
                "bsn_aanvrager": "999990032",
                "datum_onderzoek": "2026-10-16",
                "langdurige_loopbeperking": true,
                "continu_afhankelijk_van_hulp_bestuurder": false,
                "permanent_rolstoelgebonden": false,
                "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
            },
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    assert_eq!(advies["recording_actor"], json!("ggd_wilgendaal"));
    // The advice reaches the municipality's chronicle too: one step, two
    // cells.
    assert_eq!(head(&app, "ggd_wilgendaal").await, 1);
    assert_eq!(head(&app, "gemeente_wilgendaal").await, 2);

    set_clock(&app, "2026-10-20").await;
    let (status, besluit) = call(
        &app,
        Method::POST,
        "/api/besluiten",
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
    assert_eq!(head(&app, "gemeente_wilgendaal").await, 3);
}

#[tokio::test]
async fn a_retry_under_the_same_key_answers_the_same_and_records_nothing() {
    let app = server().await;
    let first = call(
        &app,
        Method::POST,
        "/api/indieningen",
        Some(application("999990020")),
        Some("k-1"),
    )
    .await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    let again = call(
        &app,
        Method::POST,
        "/api/indieningen",
        Some(application("999990020")),
        Some("k-1"),
    )
    .await;
    assert_eq!(again, first);
    assert_eq!(head(&app, "gemeente_wilgendaal").await, 1);
    // The same key with another request is refused.
    let (status, fout) = call(
        &app,
        Method::POST,
        "/api/indieningen",
        Some(application("999990033")),
        Some("k-1"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{fout}");
    assert!(fout["fout"].as_str().unwrap().contains("k-1"), "{fout}");
    // Without a key, every request counts.
    for _ in 0..2 {
        let (status, _) = call(
            &app,
            Method::POST,
            "/api/indieningen",
            Some(application("999990020")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    assert_eq!(head(&app, "gemeente_wilgendaal").await, 3);
    let (status, _) = call(
        &app,
        Method::POST,
        "/api/indieningen",
        Some(application("999990020")),
        Some("met spatie"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn what_the_cell_refuses_and_an_unknown_cell() {
    let app = server().await;
    let (status, fout) = call(
        &app,
        Method::POST,
        "/api/besluiten",
        Some(json!({ "op": "niet-bestaand", "bevindingen": {} })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert!(fout["fout"].is_string(), "{fout}");
    let (status, _) = call(&app, Method::GET, "/api/cellen/elders/kroniek", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(head(&app, "gemeente_wilgendaal").await, 0);
}
