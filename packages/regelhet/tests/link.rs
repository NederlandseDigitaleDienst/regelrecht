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
        grant_hash: None,
        front: None,
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
async fn an_admin_answers_for_the_linked_ggd_when_its_server_does_not() {
    let Pair { gemeente, ggd, .. } = pair().await;
    anna_applies(&gemeente).await;
    // The request waits in the municipality's own copy of the GGD's cell,
    // where the admin answers it as for a party without a server.
    let (_, panel) = admin(&gemeente)
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    let verzoek = panel["partijen"][0]["verzoeken"][0]["verzoek"].clone();
    assert_eq!(verzoek["fields"]["bsn_aanvrager"], json!(ANNA), "{panel}");
    let mut body = advice(ANNA);
    body["verzoek"] = verzoek["id"].clone();
    body["velden"]
        .as_object_mut()
        .unwrap()
        .remove("bsn_aanvrager");
    let (status, advies) = admin(&gemeente)
        .await
        .post("/api/backoffice/beheer/vastleggingen", body)
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    assert_eq!(request_at(&gemeente).await["open"], json!(false));

    // The receipt of that advice crosses the link as any record of the
    // municipality, so the GGD's own server holds the request as answered
    // once it is reachable, and refuses a second answer.
    let answered = eventually("the answer arriving at the GGD", async || {
        ggd_panel(&ggd).await["partijen"][0]["beantwoord"][0]
            .as_object()
            .map(|a| Value::Object(a.clone()))
    })
    .await;
    assert_eq!(
        answered["antwoord"]["fields"]["langdurige_loopbeperking"],
        json!(true)
    );
    let mut body = advice(ANNA);
    body["verzoek"] = answered["verzoek"]["id"].clone();
    body["velden"]
        .as_object_mut()
        .unwrap()
        .remove("bsn_aanvrager");
    let (status, late) = employee(&ggd, "arts-1", "regelhet-beslisser")
        .await
        .post("/api/backoffice/beheer/vastleggingen", body)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{late}");
}

#[tokio::test]
async fn each_server_serves_its_own_side_only() {
    let Pair {
        gemeente,
        ggd,
        ggd_url,
    } = pair().await;
    // The municipality's panel names the GGD as linked, with where its own
    // screens are. In test mode it lists the GGD still, marked as a
    // fallback, beside the court it enters for as before.
    let (_, panel) = admin(&gemeente)
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    assert_eq!(
        panel["gekoppeld"],
        json!([{ "cel": GGD, "cel_naam": "GGD Wilgendaal", "voorkant": ggd_url }])
    );
    let fallback: Vec<(&str, bool)> = panel["partijen"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| Some((p["cel"].as_str()?, p["nooduitweg"].as_bool()?)))
        .collect();
    assert_eq!(fallback, [(GGD, true), (RECHTBANK, false)]);

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

/// A stand-in for an FSC outway: it takes a request only under the grant
/// hash `grant`, as OpenFSC's outway does, and hands it to the server at
/// `target`, as that peer's inway would. It counts what it let through.
async fn outway(
    grant: &'static str,
    target: String,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::body::Bytes;
    use axum::extract::Request;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let passed = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&passed);
    let http = reqwest::Client::new();
    let app = Router::new().fallback(move |request: Request| {
        let (http, target, count) = (http.clone(), target.clone(), Arc::clone(&count));
        async move {
            let hash = request
                .headers()
                .get("Fsc-Grant-Hash")
                .and_then(|h| h.to_str().ok())
                .map(str::to_string);
            if hash.as_deref() != Some(grant) {
                return (
                    StatusCode::BAD_REQUEST,
                    Bytes::from("no valid Fsc-Grant-Hash"),
                );
            }
            let method = reqwest::Method::from_bytes(request.method().as_str().as_bytes()).unwrap();
            let path = request.uri().path().to_string();
            let body = axum::body::to_bytes(request.into_body(), 1 << 20)
                .await
                .unwrap();
            let answer = http
                .request(method, format!("{target}{path}"))
                .body(body)
                .send()
                .await
                .unwrap();
            count.fetch_add(1, Ordering::SeqCst);
            let status = StatusCode::from_u16(answer.status().as_u16()).unwrap();
            (status, answer.bytes().await.unwrap())
        }
    });
    let (at, url) = listener().await;
    tokio::spawn(async move { axum::serve(at, app).await.unwrap() });
    (url, passed)
}

#[tokio::test]
async fn with_a_grant_every_request_to_a_peer_goes_through_the_outway() {
    regelrecht_auth::install_crypto_provider();
    let (at_gemeente, gemeente_url) = listener().await;
    let (at_ggd, ggd_url) = listener().await;
    // Each server knows its peer only by its own outway and a grant.
    let (to_ggd, through_to_ggd) = outway("$1$4$naar-de-ggd", ggd_url).await;
    let (to_gemeente, through_to_gemeente) = outway("$1$4$naar-de-gemeente", gemeente_url).await;
    let via = |cell: &str, url: &str, grant: &str| {
        vec![PeerSetting {
            cell: cell.into(),
            url: url.into(),
            grant_hash: Some(grant.into()),
            front: None,
            key: None,
        }]
    };
    let mut config = config_on(Mode::Test, Surface::All);
    config.link = LinkSettings {
        party: None,
        peers: via(GGD, &to_ggd, "$1$4$naar-de-ggd"),
        key: None,
    };
    let gemeente = app::build_with(&config, Login::TestLogin).await.unwrap();
    let mut config = config_on(Mode::Test, Surface::Backoffice);
    config.link = LinkSettings {
        party: Some(GGD.into()),
        peers: via(GEMEENTE, &to_gemeente, "$1$4$naar-de-gemeente"),
        key: None,
    };
    let ggd = app::build_with(&config, Login::TestLogin).await.unwrap();
    for (listener, app) in [(at_gemeente, gemeente.clone()), (at_ggd, ggd.clone())] {
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    }

    anna_applies(&gemeente).await;
    let verzoek = eventually(
        "the request arriving at the GGD through the outway",
        async || {
            ggd_panel(&ggd).await["partijen"][0]["verzoeken"][0]["verzoek"]
                .as_object()
                .map(|v| Value::Object(v.clone()))
        },
    )
    .await;
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
    eventually(
        "the advice arriving at the municipality through the outway",
        async || (request_at(&gemeente).await["open"] == json!(false)).then_some(()),
    )
    .await;
    // Both the messages and the keys they verify under went that way: the
    // message itself, and the GGD reading the municipality's key back
    // through its own outway.
    use std::sync::atomic::Ordering;
    assert!(through_to_ggd.load(Ordering::SeqCst) >= 1);
    assert!(through_to_gemeente.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn under_a_grant_the_outway_does_not_know_nothing_arrives_and_the_record_waits() {
    regelrecht_auth::install_crypto_provider();
    let (at_ggd, ggd_url) = listener().await;
    let (to_ggd, through) = outway("$1$4$het-echte-grant", ggd_url.clone()).await;
    let mut config = config_on(Mode::Test, Surface::All);
    config.link = LinkSettings {
        party: None,
        peers: vec![PeerSetting {
            cell: GGD.into(),
            url: to_ggd,
            grant_hash: Some("$1$4$een-ander-grant".into()),
            front: None,
            key: None,
        }],
        key: None,
    };
    let gemeente = app::build_with(&config, Login::TestLogin).await.unwrap();
    let mut config = config_on(Mode::Test, Surface::Backoffice);
    config.link = LinkSettings {
        party: Some(GGD.into()),
        peers: peer(GEMEENTE, "http://127.0.0.1:9"),
        key: None,
    };
    let ggd = app::build_with(&config, Login::TestLogin).await.unwrap();
    tokio::spawn(async move { axum::serve(at_ggd, ggd.clone()).await.unwrap() });
    anna_applies(&gemeente).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(through.load(std::sync::atomic::Ordering::SeqCst), 0);
    // The request stays open at the municipality, to be sent again.
    assert_eq!(request_at(&gemeente).await["open"], json!(true));
}
