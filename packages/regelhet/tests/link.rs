//! Two servers of one case, the municipality's and the GGD's, with a link
//! between them: the request for an examination crosses to the GGD's own
//! server, its staff answer there, and the advice crosses back.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use regelrecht_fsc4b::jose::SigningKey;
use regelrecht_regelhet::app;
use regelrecht_regelhet::config::{LinkSettings, Mode, PeerSetting};
use regelrecht_regelhet::link::{KEY_PATH, MESSAGES_PATH, PARTIES_PATH};
use regelrecht_regelhet::login::Login;
use regelrecht_regelhet::surface::Surface;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// The two servers, each listening on an address of its own and linked to
/// the other's.
struct Pair {
    gemeente: Router,
    ggd: Router,
    ggd_url: String,
}

async fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (listener, url)
}

fn peer(cell: &str, url: &str) -> Vec<PeerSetting> {
    vec![PeerSetting {
        cell: cell.into(),
        url: url.into(),
        key: None,
    }]
}

async fn pair() -> Pair {
    regelrecht_auth::install_crypto_provider();
    let (at_gemeente, gemeente_url) = listener().await;
    let (at_ggd, ggd_url) = listener().await;
    let mut config = config_on(Mode::Test, Surface::All);
    config.link = LinkSettings {
        party: None,
        peers: peer(GGD, &ggd_url),
        key: None,
    };
    let gemeente = app::build_with(&config, Login::TestLogin).await.unwrap();
    let mut config = config_on(Mode::Test, Surface::Backoffice);
    config.link = LinkSettings {
        party: Some(GGD.into()),
        peers: peer(GEMEENTE, &gemeente_url),
        key: None,
    };
    let ggd = app::build_with(&config, Login::TestLogin).await.unwrap();
    for (listener, app) in [(at_gemeente, gemeente.clone()), (at_ggd, ggd.clone())] {
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    }
    Pair {
        gemeente,
        ggd,
        ggd_url,
    }
}

/// Ask `look` until it gives something, for as long as a link may take.
async fn eventually<T, F: AsyncFn() -> Option<T>>(what: &str, look: F) -> T {
    for _ in 0..100 {
        if let Some(found) = look().await {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{what} did not happen");
}

/// The panel of the GGD's own server, as one of its doctors sees it.
async fn ggd_panel(ggd: &Router) -> Value {
    let (status, panel) = employee(ggd, "arts-1", "regelhet-beslisser")
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    assert_eq!(status, StatusCode::OK, "{panel}");
    panel
}

/// The municipality's one request, as its back office lists it.
async fn request_at(gemeente: &Router) -> Value {
    let (status, body) = employee(gemeente, "lezer-1", "regelhet-lezer")
        .await
        .get("/api/backoffice/verzoeken")
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["verzoeken"][0].clone()
}

#[tokio::test]
async fn the_request_reaches_the_ggds_server_and_its_advice_comes_back() {
    let Pair { gemeente, ggd, .. } = pair().await;
    let aanvraag = anna_applies(&gemeente).await;
    assert_eq!(request_at(&gemeente).await["open"], json!(true));

    // The request arrives at the GGD's own server, where its staff see it.
    let verzoek = eventually("the request arriving at the GGD", async || {
        let panel = ggd_panel(&ggd).await;
        assert_eq!(panel["partijen"].as_array().unwrap().len(), 1);
        assert_eq!(panel["partijen"][0]["cel"], json!(GGD));
        panel["partijen"][0]["verzoeken"][0]["verzoek"]
            .as_object()
            .map(|v| Value::Object(v.clone()))
    })
    .await;
    assert_eq!(verzoek["fields"]["bsn_aanvrager"], json!(ANNA));
    assert_eq!(verzoek["fields"]["aanvraag"], aanvraag["id"]);

    // A doctor of the GGD answers it there.
    let mut body = advice(ANNA);
    body["verzoek"] = verzoek["id"].clone();
    body["velden"]
        .as_object_mut()
        .unwrap()
        .remove("bsn_aanvrager");
    let (status, advies) = employee(&ggd, "arts-1", "regelhet-beslisser")
        .await
        .post("/api/backoffice/beheer/vastleggingen", body)
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");

    // The advice crosses back and closes the request at the municipality.
    let closed = eventually("the advice arriving at the municipality", async || {
        let request = request_at(&gemeente).await;
        (request["open"] == json!(false)).then_some(request)
    })
    .await;
    assert_eq!(
        closed["antwoord_ontvangen"]["fields"]["langdurige_loopbeperking"],
        json!(true)
    );
    // And the GGD has nothing left to answer.
    eventually("the request leaving the GGD's list", async || {
        ggd_panel(&ggd).await["partijen"][0]["verzoeken"]
            .as_array()
            .is_some_and(Vec::is_empty)
            .then_some(())
    })
    .await;
    // Its own server keeps what it answered, with the answer beside it.
    let answered = ggd_panel(&ggd).await["partijen"][0]["beantwoord"].clone();
    assert_eq!(answered.as_array().map(Vec::len), Some(1), "{answered}");
    assert_eq!(answered[0]["verzoek"]["id"], verzoek["id"]);
    assert_eq!(answered[0]["antwoord"]["id"], advies["id"]);
    // The municipality's admin panel never lists a party's answered requests.
    let (_, at_gemeente) = admin(&gemeente)
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    for party in at_gemeente["partijen"].as_array().unwrap() {
        assert_eq!(party["beantwoord"], json!([]), "{party}");
    }
}

#[tokio::test]
async fn each_server_serves_its_own_side_only() {
    let Pair { gemeente, ggd, .. } = pair().await;
    // The municipality's admin enters nothing for the GGD any more: the
    // panel names it as linked, and the route refuses it.
    let (_, panel) = admin(&gemeente)
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    let cells: Vec<&str> = panel["partijen"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["cel"].as_str())
        .collect();
    assert_eq!(cells, [RECHTBANK]);
    assert_eq!(panel["gekoppeld"][0]["cel"], json!(GGD));
    let (status, _) = admin(&gemeente)
        .await
        .post("/api/backoffice/beheer/vastleggingen", advice(ANNA))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The GGD's server says whose it is, has no back office of the
    // municipality, and enters nothing for the court.
    let arts = employee(&ggd, "arts-1", "regelhet-beslisser").await;
    let (_, ik) = arts.get("/api/ik").await;
    assert_eq!(ik["partij"]["cel"], json!(GGD));
    assert_eq!(ik["partijen_invoeren"], json!(true));
    let (status, _) = arts.get("/api/backoffice/werkvoorraad").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut for_the_court = advice(ANNA);
    for_the_court["cel"] = json!(RECHTBANK);
    let (status, _) = arts
        .post("/api/backoffice/beheer/vastleggingen", for_the_court)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The test login offers the GGD's own staff, not the municipality's.
    let (_, personas) = anonymous(&ggd).get("/auth/testpersonen").await;
    let personas = personas.as_array().unwrap().clone();
    assert_eq!(personas.len(), 2, "{personas:?}");
    for persona in &personas {
        assert_eq!(persona["partij"], json!(true));
        assert_eq!(persona["cel"], json!(GGD));
        assert_eq!(persona["cel_naam"], json!("GGD Wilgendaal"));
    }
    let (_, at_gemeente) = anonymous(&gemeente).get("/auth/testpersonen").await;
    assert!(
        at_gemeente
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["partij"] != json!(true))
    );
    // A reader of the GGD enters nothing.
    let (status, _) = employee(&ggd, "lezer-1", "regelhet-lezer")
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_server_names_the_parties_its_policy_asks_something_of() {
    // Without a link, and without a login: the municipality asks the GGD
    // for an examination, and the court nothing.
    let alone = server().await;
    let (status, parties) = anonymous(&alone).get(PARTIES_PATH).await;
    assert_eq!(status, StatusCode::OK, "{parties}");
    assert_eq!(
        parties,
        json!({
            "uitvoerder": { "cel": GEMEENTE, "cel_naam": "gemeente Wilgendaal" },
            "eigen_cel": GEMEENTE,
            "partijen": [{ "cel": GGD, "cel_naam": "GGD Wilgendaal", "gekoppeld": false }],
        })
    );
    // An instance for citizens does not have the route.
    let citizens = app::build_with(&config_on(Mode::Test, Surface::Citizen), Login::TestLogin)
        .await
        .unwrap();
    let (status, _) = anonymous(&citizens).get(PARTIES_PATH).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Linked, both servers say so.
    let Pair { gemeente, ggd, .. } = pair().await;
    let (_, at_gemeente) = anonymous(&gemeente).get(PARTIES_PATH).await;
    assert_eq!(at_gemeente["partijen"][0]["gekoppeld"], json!(true));
    let (_, at_ggd) = anonymous(&ggd).get(PARTIES_PATH).await;
    assert_eq!(at_ggd["eigen_cel"], json!(GGD));
    assert_eq!(at_ggd["partijen"][0]["gekoppeld"], json!(true));
}

#[tokio::test]
async fn a_message_under_another_key_is_refused() {
    let Pair { ggd, ggd_url, .. } = pair().await;
    let http = reqwest::Client::new();
    let (_, key) = anonymous(&ggd).get(KEY_PATH).await;
    assert_eq!(key["cel"], json!(GGD));
    assert_eq!(key["sleutel"]["crv"], json!("P-256"));

    // Signed by someone who is not the municipality's server.
    let stranger = SigningKey::generate().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = json!({
        "iss": GEMEENTE,
        "aud": GGD,
        "jti": "vervalst-1",
        "iat": now,
        "feit": "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#9#geneeskundig_onderzoek_aangevraagd",
        "velden": { "bsn_aanvrager": ANNA },
        "datum": "2026-10-13",
    });
    for body in [stranger.sign(&claims, &[]).unwrap(), "geen.jws.hier".into()] {
        let response = http
            .post(format!("{ggd_url}{MESSAGES_PATH}"))
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    }
    assert!(
        ggd_panel(&ggd).await["partijen"][0]["verzoeken"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
