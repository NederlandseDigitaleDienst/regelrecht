//! Citizens and employees each on an instance of their own: which routes
//! each surface registers, which roles and sessions it admits, which test
//! accounts it offers, and that two instances on one database run one
//! process, with one chronicle and one clock.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use axum::Router;
use axum::http::{Method, StatusCode};
use common::*;
use regelrecht_shared::postgres::test_utils::TestDatabase;
use regelrecht_wetsproces::app;
use regelrecht_wetsproces::config::{Config, Fsc4bChoice, Mode, StoreChoice};
use regelrecht_wetsproces::login::Login;
use regelrecht_wetsproces::surface::Surface;
use serde_json::{Value, json};

/// Which group a route belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Group {
    /// Both surfaces: health, the login, the case, the forms, the clock, `ik`.
    Shared,
    /// The citizen's own routes, and FSC voor Burgers.
    Citizen,
    /// The back office.
    Backoffice,
}

/// Every route of the server, with its group, by hand: a route added to the
/// API belongs here too. `every_back_office_route_of_the_api_is_in_the_table`
/// holds that for the back office, the group that must never reach the
/// instance for citizens.
const ROUTES: &[(Group, &str, &str)] = &[
    (Group::Shared, "GET", "/health"),
    (Group::Shared, "GET", "/auth/status"),
    (Group::Shared, "GET", "/auth/testpersonen"),
    (Group::Shared, "POST", "/auth/testlogin"),
    (Group::Shared, "GET", "/auth/logout"),
    (Group::Shared, "GET", "/api/casus"),
    (Group::Shared, "GET", "/api/formulier"),
    (Group::Shared, "GET", "/api/klok"),
    (Group::Shared, "GET", "/api/ik"),
    (Group::Citizen, "GET", "/api/dossier"),
    (Group::Citizen, "GET", "/api/dossier/waarom/x"),
    (Group::Citizen, "POST", "/api/indieningen"),
    (Group::Citizen, "GET", "/fsc4b/discovery"),
    (Group::Citizen, "POST", "/fsc4b/session/v1/challenge"),
    (Group::Citizen, "POST", "/fsc4b/burger/v1/indieningen"),
    (
        Group::Backoffice,
        "GET",
        "/api/backoffice/cellen/gemeente_wilgendaal/kroniek",
    ),
    (Group::Backoffice, "GET", "/api/backoffice/waarom/x"),
    (Group::Backoffice, "POST", "/api/backoffice/voorvertoning"),
    (Group::Backoffice, "POST", "/api/backoffice/vastleggingen"),
    (Group::Backoffice, "POST", "/api/backoffice/toezendingen"),
    (Group::Backoffice, "POST", "/api/backoffice/besluiten"),
    (Group::Backoffice, "PUT", "/api/backoffice/klok"),
];

/// Whether `surface` registers the routes of `group`.
fn registers(surface: Surface, group: Group) -> bool {
    match group {
        Group::Shared => true,
        Group::Citizen => surface.serves_citizens(),
        Group::Backoffice => surface.serves_employees(),
    }
}

/// A server in test mode on `surface`, with FSC voor Burgers on the demo
/// Stelsel where the surface serves citizens, so its routes are in the
/// table too.
async fn on(surface: Surface) -> Router {
    let mut c = config_on(Mode::Test, surface);
    if surface.serves_citizens() {
        c.fsc4b = Fsc4bChoice::Demo {
            public_url: "http://localhost:7199".into(),
        };
    }
    app::build_with(&c, Login::TestLogin).await.unwrap()
}

#[tokio::test]
async fn each_surface_registers_its_own_routes_and_no_route_of_the_other_side() {
    for surface in Surface::ALL {
        let app = on(surface).await;
        let nobody = anonymous(&app);
        for (group, method, path) in ROUTES {
            let method: Method = method.parse().unwrap();
            let (status, _) = nobody.call(method, path, None, None).await;
            if registers(surface, *group) {
                assert_ne!(status, StatusCode::NOT_FOUND, "{surface}: {path}");
            } else {
                assert_eq!(status, StatusCode::NOT_FOUND, "{surface}: {path}");
            }
        }
    }
}

#[test]
fn every_back_office_route_of_the_api_is_in_the_table() {
    // The router in api.rs names each route once, as a string literal, on
    // the line of `.route(` or, once rustfmt wraps the call, on the next.
    let api = include_str!("../src/api.rs");
    let declared: BTreeSet<&str> = api
        .split(".route(")
        .skip(1)
        .filter_map(|rest| rest.trim_start().strip_prefix('"'))
        .filter_map(|rest| rest.split('"').next())
        .filter(|path| path.starts_with("/api/backoffice/"))
        .collect();
    assert!(declared.len() >= 7, "{declared:?}");
    // The table names a path with its parameters filled in.
    let fill = |path: &str| {
        path.replace("{cel}", "gemeente_wilgendaal")
            .replace("{id}", "x")
    };
    let table: BTreeSet<String> = ROUTES
        .iter()
        .filter(|(group, _, _)| *group == Group::Backoffice)
        .map(|(_, _, path)| (*path).to_string())
        .collect();
    for path in declared {
        assert!(table.contains(&fill(path)), "{path} is not in ROUTES");
    }
}

#[tokio::test]
async fn a_route_of_the_other_side_is_a_404_also_with_a_session() {
    // On the instance for citizens even an account that carries every
    // role finds no route of the back office: not refused, not there.
    let citizens = on(Surface::Citizen).await;
    let everything = login(
        &citizens,
        "alles-1",
        &[
            "wetsproces-burger",
            "wetsproces-admin",
            "wetsproces-cel-gemeente_wilgendaal",
        ],
        json!({ "bsn": ANNA }),
    )
    .await;
    let (status, _) = everything
        .call(
            Method::PUT,
            "/api/backoffice/klok",
            Some(json!({ "datum": "2026-11-01" })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = everything
        .get("/api/backoffice/cellen/gemeente_wilgendaal/kroniek")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let backoffice = on(Surface::Backoffice).await;
    let admin = admin(&backoffice).await;
    let (status, _) = admin.post("/api/indieningen", application()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(admin.get("/api/dossier").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_citizens_instance_never_gives_a_session_an_employees_role() {
    let citizens = on(Surface::Citizen).await;
    let both = login(
        &citizens,
        "beide-1",
        &[
            "wetsproces-burger",
            "wetsproces-beslisser",
            "wetsproces-cel-gemeente_wilgendaal",
        ],
        json!({ "bsn": ANNA }),
    )
    .await;
    let (status, ik) = both.get("/api/ik").await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    assert_eq!(ik["burger"], json!(true));
    assert_eq!(ik["niveau"], Value::Null);
    assert_eq!(ik["cellen"], json!([]));
    assert_eq!(ik["klok_zetten"], json!(false));
    // An employee's account is nobody here.
    let employee = employee(&citizens, "lezer-1", "wetsproces-lezer", GEMEENTE).await;
    assert_eq!(employee.get("/api/ik").await.0, StatusCode::FORBIDDEN);
    assert_eq!(employee.get("/api/casus").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_back_office_refuses_a_session_with_only_the_citizens_role() {
    let backoffice = on(Surface::Backoffice).await;
    let anna = citizen(&backoffice, "anna", ANNA).await;
    for path in ["/api/ik", "/api/casus", "/api/formulier", "/api/klok"] {
        assert_eq!(anna.get(path).await.0, StatusCode::FORBIDDEN, "{path}");
    }
    // An account with both is an employee here, and no citizen.
    let both = login(
        &backoffice,
        "beide-1",
        &[
            "wetsproces-burger",
            "wetsproces-lezer",
            "wetsproces-cel-gemeente_wilgendaal",
        ],
        json!({ "bsn": ANNA }),
    )
    .await;
    let (status, ik) = both.get("/api/ik").await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    assert_eq!(ik["burger"], json!(false));
    assert_eq!(ik["identiteit"], Value::Null);
    assert_eq!(ik["niveau"], json!("lezer"));
}

#[tokio::test]
async fn the_test_login_offers_each_side_its_own_accounts() {
    let kinds = |app: Router| async move {
        let (status, personas) = anonymous(&app).get("/auth/testpersonen").await;
        assert_eq!(status, StatusCode::OK, "{personas}");
        personas
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["soort"].as_str().unwrap().to_string())
            .collect::<BTreeSet<String>>()
    };
    let set = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    assert_eq!(kinds(on(Surface::Citizen).await).await, set(&["burger"]));
    assert_eq!(
        kinds(on(Surface::Backoffice).await).await,
        set(&["medewerker", "beheerder"])
    );
    assert_eq!(
        kinds(on(Surface::All).await).await,
        set(&["burger", "medewerker", "beheerder"])
    );
}

/// Two configurations in test mode on the database at `url`: the instance
/// for citizens and the one for the back office, as two deployments run.
fn pair(url: &str) -> (Config, Config) {
    let store = StoreChoice::Postgres { url: url.into() };
    let mut citizens = config_on(Mode::Test, Surface::Citizen);
    citizens.store = store.clone();
    let mut backoffice = config_on(Mode::Test, Surface::Backoffice);
    backoffice.store = store;
    (citizens, backoffice)
}

/// The session cookie of `who`, under the name `name`.
fn cookie_as(who: &As<'_>, name: &str) -> String {
    let cookie = who.cookie.as_deref().unwrap();
    let (_, value) = cookie.split_once('=').unwrap();
    format!("{name}={value}")
}

#[tokio::test]
async fn a_session_of_one_side_is_no_session_on_the_other() {
    let database = TestDatabase::create().await;
    let (c, b) = pair(database.url());
    let citizens = app::build_with(&c, Login::TestLogin).await.unwrap();
    let backoffice = app::build_with(&b, Login::TestLogin).await.unwrap();

    let admin = admin(&backoffice).await;
    assert!(
        admin
            .cookie
            .as_deref()
            .unwrap()
            .starts_with("wetsproces_backoffice_session=")
    );
    let anna = citizen(&citizens, "anna", ANNA).await;
    assert!(
        anna.cookie
            .as_deref()
            .unwrap()
            .starts_with("wetsproces_burger_session=")
    );
    // Both sessions are in `wetsproces.sessions`; each is valid on its own
    // instance only, also under the other's cookie name.
    let stolen = As {
        app: &citizens,
        cookie: Some(cookie_as(&admin, "wetsproces_burger_session")),
    };
    assert_eq!(stolen.get("/api/klok").await.0, StatusCode::UNAUTHORIZED);
    let stolen = As {
        app: &backoffice,
        cookie: Some(cookie_as(&anna, "wetsproces_backoffice_session")),
    };
    assert_eq!(stolen.get("/api/klok").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(admin.get("/api/klok").await.0, StatusCode::OK);
    assert_eq!(anna.get("/api/klok").await.0, StatusCode::OK);
}

#[tokio::test]
async fn a_submission_on_the_citizens_instance_is_decided_on_the_back_office() {
    let database = TestDatabase::create().await;
    let (c, b) = pair(database.url());
    let citizens = app::build_with(&c, Login::TestLogin).await.unwrap();
    let backoffice = app::build_with(&b, Login::TestLogin).await.unwrap();
    let anna = citizen(&citizens, "anna", ANNA).await;
    let admin = admin(&backoffice).await;
    let set_clock = |day: &'static str| {
        let admin = &admin;
        async move {
            let (status, klok) = admin
                .call(
                    Method::PUT,
                    "/api/backoffice/klok",
                    Some(json!({ "datum": day })),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{klok}");
        }
    };

    // Anna applies on her own instance.
    let (status, aanvraag) = anna.post("/api/indieningen", application()).await;
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    assert_eq!(aanvraag["recorded_on"], json!("2026-10-13"));
    let op = aanvraag["id"].as_str().unwrap().to_string();

    // The back office sees it in the municipality's chronicle.
    let reader = employee(&backoffice, "lezer-1", "wetsproces-lezer", GEMEENTE).await;
    let (status, kroniek) = reader
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    assert!(
        kroniek["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == json!(op)),
        "{kroniek}"
    );

    // The admin moves the clock on the back office; the citizens' instance
    // tells the same day, because the day lives in the shared store.
    set_clock("2026-10-16").await;
    assert_eq!(anna.get("/api/klok").await.1["datum"], json!("2026-10-16"));
    let ggd = employee(&backoffice, "ggd-1", "wetsproces-behandelaar", GGD).await;
    let (status, advies) = ggd
        .post("/api/backoffice/vastleggingen", advice(ANNA))
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    assert_eq!(advies["recorded_on"], json!("2026-10-16"));

    set_clock("2026-10-20").await;
    let decider = employee(&backoffice, "gemeente-1", "wetsproces-beslisser", GEMEENTE).await;
    let (status, besluit) = decider
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": op, "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    let besluit_id = besluit["besluit"]["id"].as_str().unwrap().to_string();
    assert_eq!(besluit["besluit"]["recorded_on"], json!("2026-10-20"));

    // Anna sees the decision in her dossier on her own instance, and why.
    let (status, dossier) = anna.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    assert!(
        dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == json!(besluit_id)),
        "{dossier}"
    );
    let (status, waarom) = anna.get(&format!("/api/dossier/waarom/{besluit_id}")).await;
    assert_eq!(status, StatusCode::OK, "{waarom}");
    assert!(waarom.is_object(), "{waarom}");
}
