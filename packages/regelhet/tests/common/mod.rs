//! What the integration tests of the HTTP API share: a server in test mode
//! with the test login, an account's session on it, and the requests of the
//! parking card process.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use regelrecht_auth::OidcConfig;
use regelrecht_regelhet::app;
use regelrecht_regelhet::config::{
    Config, DEFAULT_MANIFEST, DEFAULT_PORT, DEFAULT_SUBJECT_CLAIM, Fsc4bChoice, LoginChoice, Mode,
    StoreChoice,
};
use regelrecht_regelhet::login::{Login, OidcLogin};
use regelrecht_regelhet::surface::Surface;
use serde_json::{Value, json};
use tower::ServiceExt;

pub const ADVICE: &str =
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";
pub const GEMEENTE: &str = "gemeente_wilgendaal";
pub const GGD: &str = "ggd_wilgendaal";
pub const RECHTBANK: &str = "rechtbank_wilgendaal";

/// Made-up citizen numbers of the BSN test range.
pub const ANNA: &str = "999990032";
pub const BRAM: &str = "999990020";

/// The configuration of `mode` on the store in memory with the test login:
/// in test mode on both surfaces, as `just regelhet` runs; in stable mode,
/// which serves one side, on the citizens'.
pub fn config(mode: Mode) -> Config {
    let surface = match mode {
        Mode::Test => Surface::All,
        Mode::Stable => Surface::Citizen,
    };
    config_on(mode, surface)
}

/// The configuration of `mode` on `surface`, on the store in memory with
/// the test login.
pub fn config_on(mode: Mode, surface: Surface) -> Config {
    Config {
        mode,
        surface,
        store: StoreChoice::Memory,
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        case: None,
        frontend: PathBuf::from("/nonexistent/frontend"),
        port: DEFAULT_PORT,
        login: LoginChoice::TestLogin,
        subject_claim: None,
        fsc4b: Fsc4bChoice::Off,
        fsc4b_origins: Default::default(),
        accent: None,
        vlam: None,
        title: None,
        link: Default::default(),
        role: regelrecht_regelhet::config::Role::Process(None),
    }
}

pub async fn server() -> Router {
    app::build_with(&config(Mode::Test), Login::TestLogin)
        .await
        .unwrap()
}

/// A login through Keycloak whose client was made without discovery; the
/// hosts are made up.
pub fn offline_login() -> Login {
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

/// One account's session on the server.
pub struct As<'a> {
    pub app: &'a Router,
    pub cookie: Option<String>,
}

impl As<'_> {
    /// `method` on `path` with `body` and, if given, an idempotency key: the
    /// status and the JSON answer.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(cookie) = &self.cookie {
            request = request.header("cookie", cookie);
        }
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
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.call(Method::GET, path, None, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, path, Some(body), None).await
    }
}

pub fn anonymous(app: &Router) -> As<'_> {
    As { app, cookie: None }
}

/// Log in with the test login as `sub`, with `roles` and `claims`.
pub async fn login<'a>(app: &'a Router, sub: &str, roles: &[&str], claims: Value) -> As<'a> {
    login_with(
        app,
        json!({ "sub": sub, "rollen": roles, "claims": claims }),
    )
    .await
}

/// Log in through the simulated DigiD as the citizen `naam` with `bsn`.
pub async fn digid<'a>(app: &'a Router, bsn: &str, naam: &str) -> As<'a> {
    login_with(app, json!({ "bsn": bsn, "naam": naam })).await
}

/// Log in with the test login with `body`, which must succeed.
async fn login_with(app: &Router, body: Value) -> As<'_> {
    let request = Request::post("/auth/testlogin")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get("set-cookie")
        .expect("a test login sets the session cookie")
        .to_str()
        .unwrap();
    assert!(
        cookie.starts_with("regelhet_") && cookie.contains("_session="),
        "{cookie}"
    );
    let cookie = cookie.split(';').next().unwrap().to_string();
    As {
        app,
        cookie: Some(cookie),
    }
}

pub async fn citizen<'a>(app: &'a Router, sub: &str, bsn: &str) -> As<'a> {
    login(app, sub, &["regelhet-burger"], json!({ "bsn": bsn })).await
}

/// An employee of the executor at `level`: a step of the ladder is all
/// the roles an employee holds.
pub async fn employee<'a>(app: &'a Router, sub: &str, level: &str) -> As<'a> {
    login(app, sub, &[level], json!({})).await
}

pub async fn admin(app: &Router) -> As<'_> {
    login(app, "admin-1", &["regelhet-admin"], json!({})).await
}

pub async fn set_clock(app: &Router, day: &str) {
    let (status, klok) = admin(app)
        .await
        .call(
            Method::PUT,
            "/api/backoffice/klok",
            Some(json!({ "datum": day })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{klok}");
    assert_eq!(klok["datum"], json!(day));
}

/// The head of the executor's chronicle, as a reader sees it.
pub async fn head(app: &Router) -> u64 {
    let reader = employee(app, "lezer-1", "regelhet-lezer").await;
    let (status, kroniek) = reader
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    kroniek["kop"].as_u64().unwrap()
}

/// An application as a citizen sends it: only what the citizen fills in.
/// The UI spec has the system fill the rest (the citizen number, the name,
/// the date, the requested decision and the means of signing), and the
/// server refuses those from a request.
pub fn application() -> Value {
    json!({
        "velden": {
            "kaart_als_bestuurder": true,
            "kaart_als_passagier": false,
            "adres_aanvrager": "Voorbeeldlaan 2",
        }
    })
}

pub fn findings() -> Value {
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

pub fn advice(bsn: &str) -> Value {
    json!({
        "cel": GGD,
        "feit": ADVICE,
        "velden": {
            "bsn_aanvrager": bsn,
            "datum_onderzoek": "2026-10-16",
            "langdurige_loopbeperking": true,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        },
    })
}

/// The open request of the municipality to the GGD about `bsn`, as the
/// admin panel lists it: its receipt in the GGD's chronicle.
pub async fn open_request(app: &Router, bsn: &str) -> Option<Value> {
    let (status, panel) = admin(app)
        .await
        .get("/api/backoffice/beheer/partijen")
        .await;
    assert_eq!(status, StatusCode::OK, "{panel}");
    panel["partijen"]
        .as_array()?
        .iter()
        .find(|p| p["cel"] == json!(GGD))?["verzoeken"]
        .as_array()?
        .iter()
        .map(|r| r["verzoek"].clone())
        .find(|v| v["fields"]["bsn_aanvrager"] == json!(bsn))
}

/// The GGD's advice on `bsn`, entered by an admin in the admin panel from
/// the open request about `bsn`: the request says for whom it is, so the
/// form leaves the citizen number out. The answer is the record in the
/// GGD's chronicle.
pub async fn ggd_advises(app: &Router, bsn: &str) -> Value {
    let verzoek = open_request(app, bsn)
        .await
        .expect("an open request about the applicant");
    let mut body = advice(bsn);
    body["verzoek"] = verzoek["id"].clone();
    if let Some(velden) = body["velden"].as_object_mut() {
        velden.remove("bsn_aanvrager");
    }
    let (status, advies) = admin(app)
        .await
        .post("/api/backoffice/beheer/vastleggingen", body)
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    advies
}

/// Anna applies; the answer is her application.
pub async fn anna_applies(app: &Router) -> Value {
    let (status, aanvraag) = citizen(app, "anna", ANNA)
        .await
        .post("/api/indieningen", application())
        .await;
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    aanvraag
}

/// Every record id anywhere in `v`, also inside a longer string: the
/// chronicles give out ids as UUIDs.
pub fn record_ids(v: &Value, into: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::String(s) => {
            let chars: Vec<char> = s.chars().collect();
            for window in chars.windows(36) {
                let uuid = window.iter().enumerate().all(|(i, c)| {
                    if [8, 13, 18, 23].contains(&i) {
                        *c == '-'
                    } else {
                        c.is_ascii_hexdigit()
                    }
                });
                if uuid {
                    into.insert(window.iter().collect());
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|x| record_ids(x, into)),
        Value::Object(map) => map.iter().for_each(|(k, x)| {
            record_ids(&Value::String(k.clone()), into);
            record_ids(x, into);
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub fn ids_in(v: &Value) -> std::collections::BTreeSet<String> {
    let mut ids = std::collections::BTreeSet::new();
    record_ids(v, &mut ids);
    ids
}

/// An objection or an appeal as Anna fills it in, on `grounds`.
pub fn notice_of(grounds: &str) -> Value {
    json!({
        "bezwaar_of_beroepschrift_ondertekend": true,
        "naam_indiener": "A. de Vries",
        "adres_indiener": "Voorbeeldlaan 2",
        "dagtekening_bezwaar_of_beroepschrift": "2026-10-21",
        "omschrijving_bestreden_besluit": "het besluit op de aanvraag",
        "gronden_bezwaar_of_beroep": grounds,
    })
}
