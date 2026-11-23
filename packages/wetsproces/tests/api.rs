//! The parking card process through the HTTP API, in test mode with the
//! test login: an application by a citizen, the GGD's advice entered in the
//! admin panel, the municipality's decision, and what each account may and
//! may not see and do on the way.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use regelrecht_wetsproces::app;
use regelrecht_wetsproces::config::{Mode, StoreChoice};
use regelrecht_wetsproces::login::Login;
use regelrecht_wetsproces::surface::Surface;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn health_is_open_and_the_rest_needs_a_login() {
    let app = server().await;
    let nobody = anonymous(&app);
    let request = Request::get("/health").body(Body::empty()).unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
    for path in [
        "/api/casus",
        "/api/formulier",
        "/api/klok",
        "/api/dossier",
        "/api/backoffice/cellen/gemeente_wilgendaal/kroniek",
    ] {
        assert_eq!(nobody.get(path).await.0, StatusCode::UNAUTHORIZED, "{path}");
    }
    assert_eq!(
        nobody.post("/api/indieningen", application()).await.0,
        StatusCode::UNAUTHORIZED
    );
    // A login without a role of this server gets nothing either.
    let stranger = login(&app, "x", &["editor-reader"], json!({})).await;
    assert_eq!(stranger.get("/api/casus").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn case_forms_and_clock_for_citizens_and_employees() {
    let app = server().await;
    for who in [
        citizen(&app, "anna", ANNA).await,
        employee(&app, "lezer-1", "wetsproces-lezer").await,
    ] {
        let (status, casus) = who.get("/api/casus").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(casus["id"], json!("gehandicaptenparkeerkaart"));
        assert_eq!(casus["vandaag"], json!("2026-10-13"));
        let (status, vormen) = who.get("/api/formulier").await;
        assert_eq!(status, StatusCode::OK, "{vormen}");
        let aanvraag = &vormen["indieningen"]["besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49"];
        assert_eq!(aanvraag["soort"], json!("AANVRAAG"), "{vormen}");
        // The UI spec has the login fill the citizen number.
        assert_eq!(
            aanvraag["systeem"]["bsn_aanvrager"]["soort"],
            json!("inloggen"),
            "{aanvraag}"
        );
        // And names what a decision says of the deadline, on every screen.
        assert_eq!(
            vormen["teksten"]["termijn_einddatum"]["label"],
            json!("Laatste dag voor bezwaar of beroep"),
            "{vormen}"
        );
        let (status, klok) = who.get("/api/klok").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            klok,
            json!({ "datum": "2026-10-13", "modus": "test", "bestuurbaar": true })
        );
    }
}

#[tokio::test]
async fn an_application_the_ggd_advises_on_is_granted_and_lands_in_the_dossier() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    assert_eq!(aanvraag["kind"], json!("AANVRAAG"));
    assert_eq!(aanvraag["recorded_on"], json!("2026-10-13"));
    // The citizen number comes from the login.
    assert_eq!(aanvraag["fields"]["bsn_aanvrager"], json!(ANNA));
    let op = aanvraag["id"].as_str().unwrap().to_string();

    set_clock(&app, "2026-10-16").await;
    let advies = ggd_advises(&app, ANNA).await;
    assert_eq!(advies["recording_actor"], json!(GGD));
    // The application brought about the request to the GGD; the advice
    // reaches the municipality's chronicle too: one step, two cells.
    assert_eq!(head(&app).await, 3);

    set_clock(&app, "2026-10-20").await;
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, besluit) = beslisser
        .call(
            Method::POST,
            "/api/backoffice/besluiten",
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
    // The grant brings the order of the card with it, by the policy.
    assert_eq!(head(&app).await, 5);

    // Anna's dossier: her application, the request for an examination in
    // both cells, the advice in both cells, the decision on her application
    // and the order of her card.
    let (status, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    let kinds: Vec<&str> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), 7, "{dossier}");
    assert!(kinds.contains(&"AANVRAAG"), "{kinds:?}");
    // Bram has applied for nothing: his dossier is empty.
    let (status, dossier) = citizen(&app, "bram", BRAM).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dossier["grammen"], json!([]));
}

#[tokio::test]
async fn a_citizen_acts_only_for_themself() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    let bram = citizen(&app, "bram", BRAM).await;

    // The citizen number is not taken from the request.
    let mut forged = application();
    forged["velden"]["bsn_aanvrager"] = json!(ANNA);
    let (status, fout) = bram.post("/api/indieningen", forged).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert!(!fout.to_string().contains(ANNA), "{fout}");

    // A reference to a record of another's dossier is as unknown as one
    // that does not exist.
    let mut objection = application();
    objection["op"] = json!(op);
    let (status, fout) = bram.post("/api/indieningen", objection.clone()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");
    objection["op"] = json!("no-such-record");
    let (status, other) = bram.post("/api/indieningen", objection).await;
    assert_eq!((status, &other), (StatusCode::NOT_FOUND, &fout));
    // Another submission than the application must refer to the dossier.
    let mut loose = application();
    loose["artikel"] = json!("algemene_wet_bestuursrecht#6:4");
    let (status, _) = bram.post("/api/indieningen", loose).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Anna's application and the request to the GGD it brought about.
    assert_eq!(head(&app).await, 2);

    // The back office is closed to a citizen.
    for path in [
        "/api/backoffice/cellen/gemeente_wilgendaal/kroniek",
        "/api/backoffice/cellen/ggd_wilgendaal/kroniek",
    ] {
        assert_eq!(bram.get(path).await.0, StatusCode::FORBIDDEN, "{path}");
    }
    let (status, _) = bram
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": op, "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A citizen whose account has no citizen number cannot submit.
    let nameless = login(&app, "c", &["wetsproces-burger"], json!({})).await;
    assert_eq!(
        nameless.post("/api/indieningen", application()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(nameless.get("/api/dossier").await.0, StatusCode::FORBIDDEN);
    // An employee is no citizen.
    let lezer = employee(&app, "lezer-1", "wetsproces-lezer").await;
    assert_eq!(lezer.get("/api/dossier").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_employee_acts_only_for_the_executor_and_at_their_level() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    let decision = json!({ "op": op, "bevindingen": findings() });

    // A reader reads the municipality's chronicle, and no external party's.
    let lezer = employee(&app, "lezer-2", "wetsproces-lezer").await;
    let (status, _) = lezer
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK);
    for party in [GGD, RECHTBANK] {
        let (status, _) = lezer
            .get(&format!("/api/backoffice/cellen/{party}/kroniek"))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{party}");
    }
    // Nor does the admin: the panel shows what an input needs, not the
    // party's chronicle.
    let (status, _) = admin(&app)
        .await
        .get(&format!("/api/backoffice/cellen/{GGD}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Nobody records for the GGD through the employees' route, an admin
    // neither: that goes through the admin panel.
    let behandelaar = employee(&app, "gemeente-2", "wetsproces-behandelaar").await;
    for who in [&behandelaar, &admin(&app).await] {
        let (status, _) = who
            .post("/api/backoffice/vastleggingen", advice(ANNA))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // A handler of the municipality may read the application but not
    // decide on it.
    let (status, _) = behandelaar
        .post("/api/backoffice/besluiten", decision.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(head(&app).await, 2);

    // Only an admin sets the clock.
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, _) = beslisser
        .call(
            Method::PUT,
            "/api/backoffice/klok",
            Some(json!({ "datum": "2026-10-20" })),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A decision after a record in the GGD's chronicle is refused as if
    // that record did not exist: the municipality has the receipt in its
    // own chronicle, not the GGD's record.
    let advies = ggd_advises(&app, ANNA).await;
    let mut after_unreadable = decision.clone();
    after_unreadable["na"] = advies["id"].clone();
    let (status, fout) = beslisser
        .post("/api/backoffice/besluiten", after_unreadable)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");

    // The decider of the municipality decides.
    let (status, besluit) = beslisser.post("/api/backoffice/besluiten", decision).await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
}

#[tokio::test]
async fn the_forms_name_no_record_the_caller_may_not_see() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let id = aanvraag["id"].as_str().unwrap();
    let own = |forms: &Value| forms["eigen_velden"].get(id).is_some();
    let (_, forms) = citizen(&app, "anna", ANNA)
        .await
        .get("/api/formulier")
        .await;
    assert!(own(&forms), "{forms}");
    let (_, forms) = citizen(&app, "bram", BRAM)
        .await
        .get("/api/formulier")
        .await;
    assert!(!own(&forms), "{forms}");
    assert!(!forms.to_string().contains(id), "{forms}");
    // An employee sees the municipality's records, and of the GGD's advice
    // only the receipt in the municipality's chronicle.
    let advies = ggd_advises(&app, ANNA).await;
    let ggd_id = advies["id"].as_str().unwrap();
    let (_, forms) = employee(&app, "gemeente-3", "wetsproces-lezer")
        .await
        .get("/api/formulier")
        .await;
    assert!(own(&forms), "{forms}");
    // The application, the request to the GGD and the advice received.
    assert_eq!(
        forms["eigen_velden"].as_object().unwrap().len(),
        3,
        "{forms}"
    );
    assert!(!forms.to_string().contains(ggd_id), "{forms}");
}

#[tokio::test]
async fn a_retry_under_the_same_key_answers_the_same_and_records_nothing() {
    let app = server().await;
    let anna = citizen(&app, "anna", ANNA).await;
    let submit = |key: Option<&'static str>| {
        anna.call(Method::POST, "/api/indieningen", Some(application()), key)
    };
    let first = submit(Some("k-1")).await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    let again = submit(Some("k-1")).await;
    assert_eq!(again, first);
    // One application and one request to the GGD: the retry made neither
    // again.
    assert_eq!(head(&app).await, 2);
    // The same key from another account is refused, not answered with
    // Anna's application.
    let (status, fout) = citizen(&app, "bram", BRAM)
        .await
        .call(
            Method::POST,
            "/api/indieningen",
            Some(application()),
            Some("k-1"),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{fout}");
    assert!(!fout.to_string().contains(ANNA), "{fout}");
    // Without a key, every request counts.
    for _ in 0..2 {
        assert_eq!(submit(None).await.0, StatusCode::OK);
    }
    // Three applications and one request: while the first request is open,
    // the policy asks the GGD nothing again (art. 9, second paragraph).
    assert_eq!(head(&app).await, 4);
    assert_eq!(submit(Some("met spatie")).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn what_the_cell_refuses_and_an_unknown_cell() {
    let app = server().await;
    let lezer = employee(&app, "lezer-1", "wetsproces-lezer").await;
    let (status, _) = lezer.get("/api/backoffice/cellen/elders/kroniek").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut unknown_fact = advice(ANNA);
    unknown_fact["feit"] = json!("geen#1#feit");
    let (status, fout) = admin(&app)
        .await
        .post("/api/backoffice/beheer/vastleggingen", unknown_fact)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert!(fout["fout"].is_string(), "{fout}");
    assert_eq!(head(&app).await, 0);
}

#[tokio::test]
async fn the_test_login_exists_only_in_test_mode_without_oidc() {
    let body = json!({ "sub": "x", "rollen": ["wetsproces-admin"] }).to_string();
    let test_login = |app: Router| {
        let request = Request::post("/auth/testlogin")
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap();
        async move { app.oneshot(request).await.unwrap().status() }
    };
    let login_page = |app: Router| async move {
        let request = Request::get("/auth/login").body(Body::empty()).unwrap();
        app.oneshot(request).await.unwrap().status()
    };
    assert_eq!(test_login(server().await).await, StatusCode::OK);

    // Test mode with OIDC logs in through Keycloak.
    let with_oidc = app::build_with(&config(Mode::Test), offline_login())
        .await
        .unwrap();
    assert_eq!(test_login(with_oidc.clone()).await, StatusCode::NOT_FOUND);
    // The simulated DigiD is the same route, so it is not there either.
    let (status, _) = anonymous(&with_oidc)
        .post(
            "/auth/testlogin",
            json!({ "bsn": ANNA, "naam": "Anna Voorbeeld" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(login_page(with_oidc).await, StatusCode::TEMPORARY_REDIRECT);

    // Stable mode, on Postgres.
    let database = regelrecht_shared::postgres::test_utils::TestDatabase::create().await;
    let mut c = config(Mode::Stable);
    c.store = StoreChoice::postgres(database.url().to_string());
    let stable = app::build_with(&c, offline_login()).await.unwrap();
    assert_eq!(test_login(stable.clone()).await, StatusCode::NOT_FOUND);
    assert_eq!(
        login_page(stable.clone()).await,
        StatusCode::TEMPORARY_REDIRECT
    );
    let request = Request::get("/api/casus").body(Body::empty()).unwrap();
    assert_eq!(
        stable.oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_failed_return_from_keycloak_lands_on_the_login_screen_with_a_reason() {
    // Keycloak sends an error back, or a callback comes without the login
    // that started it: the app's own screen says so, it does not loop back.
    for (surface, screen) in [
        (Surface::All, "/inloggen"),
        (Surface::Backoffice, "/inloggen"),
        (Surface::Citizen, "/toegang"),
    ] {
        let mut c = config(Mode::Test);
        c.surface = surface;
        let app = app::build_with(&c, offline_login()).await.unwrap();
        for query in ["error=access_denied", "code=x&state=y"] {
            let request = Request::get(format!("/auth/callback?{query}"))
                .body(Body::empty())
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert!(response.status().is_redirection(), "{surface:?} {query}");
            assert_eq!(
                response.headers()["location"],
                format!("{screen}?fout=inloggen").as_str(),
                "{surface:?} {query}"
            );
        }
        // A 403 that Keycloak did not send is the server's own refusal: the
        // account lacks the role. Here the callback comes without the login
        // that started it, which is a 400, so only Keycloak's own error and
        // the state check are exercised; the role case is the same branch
        // with a 403 and has its own wording on the screen.
        // The way out to Keycloak itself is untouched.
        let request = Request::get("/auth/login").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert!(
            response.headers()["location"]
                .to_str()
                .unwrap()
                .starts_with("https://login.example.org/")
        );
    }
}

#[tokio::test]
async fn the_admin_empties_the_test_setup_and_nobody_is_logged_out() {
    let app = server().await;
    let anna = citizen(&app, "anna", ANNA).await;
    let admin = admin(&app).await;
    let (status, _) = anna.post("/api/indieningen", application()).await;
    assert_eq!(status, StatusCode::OK);
    set_clock(&app, "2026-12-01").await;
    assert!(head(&app).await > 0);
    let (status, klok) = admin
        .post("/api/backoffice/testomgeving/leegmaken", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{klok}");
    // The clock stands on the case's start again.
    assert_eq!(klok["datum"], json!("2026-10-13"));
    assert_eq!(head(&app).await, 0);
    // Both are still logged in; the citizen's dossier is empty.
    assert_eq!(admin.get("/api/ik").await.0, StatusCode::OK);
    let (status, dossier) = anna.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dossier["grammen"], json!([]));
    // The same application can be made again.
    assert_eq!(
        anna.post("/api/indieningen", application()).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn only_the_admin_empties_the_test_setup() {
    let app = server().await;
    let anna = citizen(&app, "anna", ANNA).await;
    let (status, _) = anna.post("/api/indieningen", application()).await;
    assert_eq!(status, StatusCode::OK);
    let path = "/api/backoffice/testomgeving/leegmaken";
    for (who, expected) in [
        (
            employee(&app, "beslisser-1", "wetsproces-beslisser").await,
            StatusCode::FORBIDDEN,
        ),
        (
            employee(&app, "behandelaar-1", "wetsproces-behandelaar").await,
            StatusCode::FORBIDDEN,
        ),
        (citizen(&app, "bram", BRAM).await, StatusCode::FORBIDDEN),
        (anonymous(&app), StatusCode::UNAUTHORIZED),
    ] {
        assert_eq!(who.post(path, json!({})).await.0, expected);
    }
    assert!(head(&app).await > 0, "nothing was emptied");
}

#[tokio::test]
async fn in_stable_mode_there_is_no_route_to_empty_anything() {
    let database = regelrecht_shared::postgres::test_utils::TestDatabase::create().await;
    let mut c = config(Mode::Stable);
    c.store = StoreChoice::postgres(database.url().to_string());
    let stable = app::build_with(&c, offline_login()).await.unwrap();
    let request = Request::post("/api/backoffice/testomgeving/leegmaken")
        .body(Body::empty())
        .unwrap();
    // Not refused for want of a login, but not there: a 404, as for any path
    // the server does not know. A route that existed would answer 401.
    assert_eq!(
        stable.oneshot(request).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn me_says_what_the_account_may_do_and_nothing_of_other_cells() {
    let app = server().await;
    let (status, ik) = citizen(&app, "anna", ANNA).await.get("/api/ik").await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    assert_eq!(ik["burger"], json!(true));
    assert_eq!(ik["identiteit"], json!(ANNA));
    assert_eq!(ik["niveau"], Value::Null);
    assert_eq!(ik["cellen"], json!([]));
    assert_eq!(ik["klok_zetten"], json!(false));
    assert_eq!(ik["partijen_invoeren"], json!(false));

    let (status, ik) = employee(&app, "gemeente-2", "wetsproces-behandelaar")
        .await
        .get("/api/ik")
        .await;
    assert_eq!(status, StatusCode::OK, "{ik}");
    assert_eq!(ik["burger"], json!(false));
    assert_eq!(ik["identiteit"], Value::Null);
    assert_eq!(ik["niveau"], json!("behandelaar"));
    // The executor only: nobody works for an external party here.
    assert_eq!(
        ik["cellen"],
        json!([{ "cel": GEMEENTE, "cel_naam": "gemeente Wilgendaal", "lezen": true,
                 "behandelen": true, "beslissen": false }])
    );
    assert_eq!(ik["partijen_invoeren"], json!(false));

    let (_, ik) = admin(&app).await.get("/api/ik").await;
    assert_eq!(ik["klok_zetten"], json!(true));
    assert_eq!(ik["partijen_invoeren"], json!(true));

    // A role of the old cell model alone is no role of this server.
    let cell_only = login(&app, "x", &["wetsproces-cel-ggd_wilgendaal"], json!({})).await;
    assert_eq!(cell_only.get("/api/ik").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_test_personas_follow_from_the_case() {
    let app = server().await;
    let (status, personas) = anonymous(&app).get("/auth/testpersonen").await;
    assert_eq!(status, StatusCode::OK, "{personas}");
    let personas = personas.as_array().unwrap();
    let citizens: Vec<&Value> = personas.iter().filter(|p| p["soort"] == "burger").collect();
    assert_eq!(citizens.len(), 2);
    assert_eq!(citizens[0]["claims"], json!({ "bsn": ANNA }));
    // A handler and a decider of the executor, and one admin; each holds
    // a step of the ladder and nothing else.
    let staff: Vec<&Value> = personas
        .iter()
        .filter(|p| p["soort"] == "medewerker")
        .collect();
    assert_eq!(staff.len(), 2);
    assert!(staff.iter().all(|p| p["cel"] == GEMEENTE), "{staff:?}");
    let decider = staff.iter().find(|p| p["niveau"] == "beslisser").unwrap();
    assert_eq!(decider["rollen"], json!(["wetsproces-beslisser"]));
    let admin = personas.iter().find(|p| p["soort"] == "beheerder").unwrap();
    assert_eq!(admin["rollen"], json!(["wetsproces-admin"]));
    // Each citizen has a made-up name beside the number.
    let names: Vec<&Value> = personas
        .iter()
        .filter(|p| p["soort"] == "burger")
        .map(|p| &p["naam"])
        .collect();
    assert_eq!(names, [&json!("Anna Voorbeeld"), &json!("Bram Proef")]);
    // Each persona logs in as the test login takes it.
    for p in personas {
        let body = json!({ "sub": p["sub"], "rollen": p["rollen"], "claims": p.get("claims").cloned().unwrap_or(json!({})) });
        let (status, _) = anonymous(&app).post("/auth/testlogin", body).await;
        assert_eq!(status, StatusCode::OK, "{p}");
    }
    // Beside a login through Keycloak there are none.
    let oidc = app::build_with(&config(Mode::Test), offline_login())
        .await
        .unwrap();
    assert_eq!(
        anonymous(&oidc).get("/auth/testpersonen").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_preview_shows_the_outcome_and_records_nothing() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    // Without the GGD's advice and without findings the policy finds no
    // examination, so the card is refused; nothing more is asked, because
    // the municipality's own judgements on it are optional.
    let (status, preview) = beslisser
        .post(
            "/api/backoffice/voorvertoning",
            json!({ "op": op, "bevindingen": {} }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["stil"], json!([]), "{preview}");
    assert_eq!(
        preview["voorlopig"]["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(false),
        "{preview}"
    );
    assert_eq!(head(&app).await, 2);
    // A handler does not decide, so does not preview either; the admin
    // panel does not reach the municipality's submission.
    let behandelaar = employee(&app, "gemeente-2", "wetsproces-behandelaar").await;
    let body = json!({ "op": op, "bevindingen": {} });
    assert_eq!(
        behandelaar
            .post("/api/backoffice/voorvertoning", body.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        admin(&app)
            .await
            .post("/api/backoffice/beheer/voorvertoning", body)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn why_a_decision_came_out_so_for_its_citizen_and_its_cell() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, besluit) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": op, "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    let id = besluit["besluit"]["id"].as_str().unwrap();

    let anna = citizen(&app, "anna", ANNA).await;
    let (status, waarom) = anna.get(&format!("/api/dossier/waarom/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{waarom}");
    assert!(
        !waarom["ingevuld"].as_array().unwrap().is_empty(),
        "{waarom}"
    );
    let (status, ook) = beslisser.get(&format!("/api/backoffice/waarom/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    // The same explanation, but the citizen does not see how far the
    // municipality read its own chronicle: that names records of others.
    assert_eq!(
        ook["keten"].as_array().map(Vec::len),
        waarom["keten"].as_array().map(Vec::len)
    );
    assert_eq!(ook["bevindingen"], waarom["bevindingen"]);
    assert!(ook["antwoorden"][0].get("tot_en_met").is_some(), "{ook}");
    assert!(
        waarom["antwoorden"][0].get("tot_en_met").is_none(),
        "{waarom}"
    );
    assert!(waarom.get("spoor").is_none() && ook.get("spoor").is_none());

    // Not for another citizen.
    let bram = citizen(&app, "bram", BRAM).await;
    let (status, fout) = bram.get(&format!("/api/dossier/waarom/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");
    // The back office explains the executor's decisions only: a record of
    // the GGD is as unknown as one that does not exist.
    let advies = ggd_advises(&app, ANNA).await;
    let ggd_id = advies["id"].as_str().unwrap();
    let (status, fout) = beslisser
        .get(&format!("/api/backoffice/waarom/{ggd_id}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");
    // An application is no decision: the kern refuses to explain it.
    let (status, fout) = anna.get(&format!("/api/dossier/waarom/{op}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert!(
        fout["fout"].as_str().unwrap().contains("not a decision"),
        "{fout}"
    );
}

/// The application form says per field where its value comes from, as the
/// UI spec has it, also for a field it does not show, and gives a citizen
/// what the system will record for them: the login (number and name), the
/// clock (the date), a fixed value (the requested decision) and a rule (the
/// signature, from article 8 of the policy, on the means of login); the
/// citizen fills in the address and the kind of card.
#[tokio::test]
async fn the_application_form_shows_where_each_field_comes_from_and_its_value() {
    let app = server().await;
    let anna = digid(&app, ANNA, "Anna Voorbeeld").await;
    let (status, vormen) = anna.get("/api/formulier").await;
    assert_eq!(status, StatusCode::OK, "{vormen}");
    let aanvraag =
        &vormen["indieningen"]["besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49"];
    let systeem = &aanvraag["systeem"];
    assert_eq!(
        systeem["bsn_aanvrager"],
        json!({ "soort": "inloggen", "waarde": ANNA }),
        "{systeem}"
    );
    assert_eq!(
        systeem["naam_aanvrager"],
        json!({ "soort": "inloggen", "inlog": "naam", "waarde": "Anna Voorbeeld" }),
        "{systeem}"
    );
    assert_eq!(
        systeem["gevraagde_beschikking"],
        json!({ "soort": "waarde", "waarde": "gehandicaptenparkeerkaart" }),
        "{systeem}"
    );
    assert_eq!(
        systeem["dagtekening"],
        json!({ "soort": "datum", "waarde": "2026-10-13" }),
        "{systeem}"
    );
    assert_eq!(
        systeem["aanvraag_ondertekend"],
        json!({
            "soort": "bron",
            "bron": "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#8#aanvraag_geldt_als_ondertekend",
            "parameters": { "inlogmiddel": "login_method" },
            "waarde": true
        }),
        "{systeem}"
    );
    // The requested decision, the signature and the date are not on the form,
    // but the forms still say how the system fills them and with what, so a
    // screen that shows a recorded application names their origin.
    assert_eq!(
        aanvraag["verborgen"],
        json!(["aanvraag_ondertekend", "dagtekening", "gevraagde_beschikking"])
    );
    // The citizen fills in what the system does not.
    for veld in [
        "adres_aanvrager",
        "kaart_als_bestuurder",
        "kaart_als_passagier",
    ] {
        assert!(systeem.get(veld).is_none(), "{veld}: {systeem}");
    }
    // The order and the groups come from the UI spec.
    let volgorde: Vec<&str> = vormen["volgorde"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        volgorde[..3],
        ["kaart_als_bestuurder", "kaart_als_passagier", "bsn_aanvrager"]
    );
    assert_eq!(
        vormen["teksten"]["adres_aanvrager"]["group"],
        json!("Gegevens van de aanvrager")
    );
    assert_eq!(
        vormen["teksten"]["gevraagde_beschikking"]["label"],
        json!("Wat vraagt u aan?")
    );
    // The deadline of a submission follows from the law: the date Awb 6:9
    // reads the receipt of an objection or appeal against. An application
    // names none.
    assert_eq!(aanvraag["termijn"], json!(null));
    let termijnen: Vec<(&str, &Value)> = vormen["indieningen"]
        .as_object()
        .unwrap()
        .values()
        .filter(|i| i["soort"] == json!("BEZWAAR") || i["soort"] == json!("BEROEP"))
        .map(|i| (i["soort"].as_str().unwrap(), &i["termijn"]))
        .collect();
    assert!(!termijnen.is_empty(), "{vormen}");
    for (soort, termijn) in termijnen {
        assert_eq!(*termijn, json!("termijn_einddatum"), "{soort}");
    }
    // An employee sees how the form fills, not a citizen's values.
    let lezer = employee(&app, "lezer-1", "wetsproces-lezer").await;
    let (_, vormen) = lezer.get("/api/formulier").await;
    let systeem = &vormen["indieningen"]["besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49"]
        ["systeem"];
    assert_eq!(systeem["bsn_aanvrager"], json!({ "soort": "inloggen" }));
}

/// The server fills what the UI spec has the system fill, on the day of the
/// (test) clock, and refuses a request that sends such a field; the
/// signature follows from the means of login, by article 8 of the policy.
#[tokio::test]
async fn the_server_fills_the_system_fields_and_the_signature_follows_from_the_login() {
    let app = server().await;
    set_clock(&app, "2026-10-20").await;
    let anna = digid(&app, ANNA, "Anna Voorbeeld").await;
    for veld in [
        "dagtekening",
        "naam_aanvrager",
        "gevraagde_beschikking",
        "aanvraag_ondertekend",
    ] {
        let mut body = application();
        body["velden"][veld] = json!("2026-10-01");
        let (status, fout) = anna.post("/api/indieningen", body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{veld}: {fout}");
        assert!(
            fout["fout"]
                .as_str()
                .unwrap()
                .contains("filled by the system"),
            "{fout}"
        );
    }
    // Who submits is the server's to say, not the request's.
    let mut body = application();
    body["indiener"] = json!({ "inlog": {}, "inlogmiddel": "DigiD" });
    let (status, fout) = anna.post("/api/indieningen", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert_eq!(head(&app).await, 0);

    let (status, aanvraag) = anna.post("/api/indieningen", application()).await;
    assert_eq!(status, StatusCode::OK, "{aanvraag}");
    let velden = &aanvraag["fields"];
    assert_eq!(velden["bsn_aanvrager"], json!(ANNA));
    assert_eq!(velden["naam_aanvrager"], json!("Anna Voorbeeld"));
    assert_eq!(velden["dagtekening"], json!("2026-10-20"), "the test clock");
    assert_eq!(
        velden["gevraagde_beschikking"],
        json!("gehandicaptenparkeerkaart")
    );
    assert_eq!(
        velden["aanvraag_ondertekend"],
        json!(true),
        "DigiD, article 8"
    );
    // The decision on it reads the 4:2 check as before.
    let id = aanvraag["id"].as_str().unwrap();
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, besluit) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": id, "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
}

#[tokio::test]
async fn the_frontend_is_served_beside_the_api_and_a_wrong_api_path_stays_a_404() {
    /// The directory, removed when the test ends, also when it fails.
    struct Weg(PathBuf);
    impl Drop for Weg {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = std::env::temp_dir().join(format!("wetsproces-frontend-{}", std::process::id()));
    let _weg = Weg(dir.clone());
    // One build per surface, in a directory of its name; test mode serves
    // both sides, so the combined build.
    let all = dir.join("all");
    std::fs::create_dir_all(all.join("assets")).unwrap();
    std::fs::write(all.join("index.html"), "<!doctype html><title>app</title>").unwrap();
    std::fs::write(all.join("assets/app.js"), "console.log(1)").unwrap();
    std::fs::create_dir_all(dir.join("burger")).unwrap();
    std::fs::write(dir.join("burger/index.html"), "<title>burger</title>").unwrap();
    let mut c = config(Mode::Test);
    c.frontend = dir.clone();
    let app = app::build_with(&c, Login::TestLogin).await.unwrap();
    let body = |path: &'static str| {
        let app = app.clone();
        async move {
            let response = app
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            (status, String::from_utf8_lossy(&bytes).into_owned())
        }
    };
    let (status, page) = body("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("<title>app</title>"));
    // A deep link opens the app.
    let (status, page) = body("/backoffice/werkvoorraad").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("<title>app</title>"));
    assert_eq!(body("/assets/app.js").await.1, "console.log(1)");
    let (status, fout) = body("/api/bestaat-niet").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(fout.contains("fout"), "{fout}");
    assert_eq!(body("/auth/bestaat-niet").await.0, StatusCode::NOT_FOUND);

    // An instance for citizens serves the citizens' build, and a path of
    // the back office is a 404 of the API there, not a page.
    c.surface = regelrecht_wetsproces::surface::Surface::Citizen;
    let citizens = app::build_with(&c, Login::TestLogin).await.unwrap();
    let response = citizens
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("<title>burger</title>"));
    let response = citizens
        .oneshot(
            Request::get("/api/backoffice/cellen/gemeente_wilgendaal/kroniek")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_admin_panel_is_for_an_admin_and_reaches_only_the_external_parties() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let op = aanvraag["id"].as_str().unwrap();
    const PANEL_WRITES: [&str; 4] = [
        "/api/backoffice/beheer/vastleggingen",
        "/api/backoffice/beheer/voorvertoning",
        "/api/backoffice/beheer/besluiten",
        "/api/backoffice/beheer/toezendingen",
    ];

    // Not for a decider of the executor, nor for a citizen: the gate
    // refuses before the body is read.
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let anna = citizen(&app, "anna", ANNA).await;
    for who in [&beslisser, &anna] {
        assert_eq!(
            who.get("/api/backoffice/beheer/partijen").await.0,
            StatusCode::FORBIDDEN
        );
        for path in PANEL_WRITES {
            assert_eq!(
                who.post(path, json!({})).await.0,
                StatusCode::FORBIDDEN,
                "{path}"
            );
        }
    }

    // The panel names the executor and the external parties. Of the GGD it
    // shows the request Anna's application brought about, by its receipt in
    // the GGD's chronicle, with the fact that answers it; nothing else waits.
    let admin = admin(&app).await;
    let (status, panel) = admin.get("/api/backoffice/beheer/partijen").await;
    assert_eq!(status, StatusCode::OK, "{panel}");
    let verzoek = open_request(&app, ANNA).await.unwrap();
    assert_eq!(verzoek["recording_actor"], json!(GGD));
    assert_eq!(verzoek["fields"]["aanvraag"], json!(op));
    assert_eq!(verzoek["fields"]["datum_verzoek"], json!("2026-10-13"));
    assert_eq!(
        panel,
        json!({
            "uitvoerder": { "cel": GEMEENTE, "cel_naam": "gemeente Wilgendaal" },
            "partijen": [
                {
                    "cel": GGD, "cel_naam": "GGD Wilgendaal", "open": [], "toe_te_zenden": [],
                    "verzoeken": [{ "verzoek": verzoek, "antwoordfeit": ADVICE }],
                },
                {
                    "cel": RECHTBANK, "cel_naam": "Rechtbank Wilgendaal", "open": [],
                    "toe_te_zenden": [], "verzoeken": [],
                },
            ],
            "antwoord_op_verzoek": [ADVICE],
        })
    );

    // The executor is no external party, and neither is a cell the case
    // does not have.
    for cell in [GEMEENTE, "elders"] {
        let mut fact = advice(ANNA);
        fact["cel"] = json!(cell);
        let (status, fout) = admin.post(PANEL_WRITES[0], fact).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{cell}: {fout}");
    }
    // A submission of the executor is as unknown to the panel as one that
    // does not exist.
    let decision = |op: &str| json!({ "op": op, "bevindingen": findings() });
    for path in [PANEL_WRITES[1], PANEL_WRITES[2]] {
        let (status, fout) = admin.post(path, decision(op)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{fout}");
        let (status, other) = admin.post(path, decision("no-such-record")).await;
        assert_eq!((status, &other), (StatusCode::NOT_FOUND, &fout));
    }
    assert_eq!(head(&app).await, 2);

    // The GGD's advice answers Anna's open request: recorded without it, it
    // is refused, and so is a request that is no receipt in the GGD's
    // chronicle.
    let (status, fout) = admin.post(PANEL_WRITES[0], advice(ANNA)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    let mut elsewhere = advice(ANNA);
    elsewhere["verzoek"] = json!(op);
    let (status, fout) = admin.post(PANEL_WRITES[0], elsewhere).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    // The executor answers no request.
    let behandelaar = employee(&app, "gemeente-2", "wetsproces-behandelaar").await;
    let mut own = advice(ANNA);
    own["cel"] = json!(GEMEENTE);
    own["verzoek"] = verzoek["id"].clone();
    let (status, fout) = behandelaar.post("/api/backoffice/vastleggingen", own).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{fout}");
    assert_eq!(head(&app).await, 2);

    // From the request: the answer is the GGD's record, for Anna, and the
    // municipality gets it in the same step. The request leaves the panel.
    let advies = ggd_advises(&app, ANNA).await;
    assert_eq!(advies["recording_actor"], json!(GGD));
    assert_eq!(advies["fields"]["bsn_aanvrager"], json!(ANNA));
    assert_eq!(head(&app).await, 3);
    assert!(open_request(&app, ANNA).await.is_none());
    // Without an open request about the applicant, such as for one who had
    // a card before (policy, art. 9, third paragraph), the advice is
    // recorded as before, with the citizen number in the form.
    let (status, advies) = admin.post(PANEL_WRITES[0], advice(BRAM)).await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    assert_eq!(advies["fields"]["bsn_aanvrager"], json!(BRAM));

    // A decision of the executor cannot be sent from the panel.
    let (status, besluit) = beslisser
        .post("/api/backoffice/besluiten", decision(op))
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    let (status, _) = admin
        .post(
            PANEL_WRITES[3],
            json!({ "besluit": besluit["besluit"]["id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// The objection fields and the appeal fields share their names (Awb 6:5).
/// Anna applies and is refused, objects, and the objection is decided; the
/// answer is the decision on her objection, which she can appeal.
async fn anna_is_refused_on_objection(app: &Router) -> Value {
    let aanvraag = anna_applies(app).await;
    let beslisser = employee(app, "gemeente-1", "wetsproces-beslisser").await;
    let decide = |body: Value| {
        let beslisser = &beslisser;
        async move {
            let (status, besluit) = beslisser.post("/api/backoffice/besluiten", body).await;
            assert_eq!(status, StatusCode::OK, "{besluit}");
            besluit["besluit"].clone()
        }
    };
    let besluit = decide(json!({ "op": aanvraag["id"], "bevindingen": {} })).await;
    set_clock(app, "2026-10-21").await;
    let (status, bezwaar) = citizen(app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#7:11",
                "op": besluit["id"],
                "velden": notice_of("de kaart is ten onrechte geweigerd"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bezwaar}");
    decide(json!({
        "op": bezwaar["id"],
        "bevindingen": {
            "indiener_bezwaarschrift_is_belanghebbende": true,
            "indiener_bezwaarschrift_niet_in_verzuim": true,
        },
    }))
    .await
}

#[tokio::test]
async fn the_court_decides_on_an_appeal_through_the_admin_panel() {
    let app = server().await;
    let op_bezwaar = anna_is_refused_on_objection(&app).await;
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let anna = citizen(&app, "anna", ANNA).await;
    let admin = admin(&app).await;

    // Anna appeals; the appeal lands with the court and waits in the panel.
    let (status, beroep) = anna
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#8:70",
                "op": op_bezwaar["id"],
                "velden": notice_of("het bezwaar is ten onrechte afgewezen"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beroep}");
    assert_eq!(beroep["recording_actor"], json!(RECHTBANK));
    let (_, panel) = admin.get("/api/backoffice/beheer/partijen").await;
    let court = &panel["partijen"][1];
    assert_eq!(court["cel"], json!(RECHTBANK));
    assert_eq!(court["open"][0]["stuk"]["id"], beroep["id"], "{panel}");
    // The municipality's employees do not reach it.
    let (status, _) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": beroep["id"], "bevindingen": {} }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The admin enters the judgment for the court.
    let judgment = json!({
        "op": beroep["id"],
        "bevindingen": {
            "indiener_is_belanghebbende": true,
            "indiener_beroepschrift_niet_in_verzuim": true,
            "rechtsregel_geschonden": false,
        },
    });
    let (status, preview) = admin
        .post("/api/backoffice/beheer/voorvertoning", judgment.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let (status, uitspraak) = admin
        .post("/api/backoffice/beheer/besluiten", judgment)
        .await;
    assert_eq!(status, StatusCode::OK, "{uitspraak}");
    let uitspraak_id = uitspraak["besluit"]["id"].clone();
    assert_eq!(uitspraak["besluit"]["recording_actor"], json!(RECHTBANK));
    let (_, panel) = admin.get("/api/backoffice/beheer/partijen").await;
    let court = &panel["partijen"][1];
    assert_eq!(court["open"], json!([]), "{panel}");
    assert_eq!(court["toe_te_zenden"][0]["id"], uitspraak_id, "{panel}");

    // The copy goes to the municipality, which records its receipt.
    let before = head(&app).await;
    let (status, afschrift) = admin
        .post(
            "/api/backoffice/beheer/toezendingen",
            json!({ "besluit": uitspraak_id }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{afschrift}");
    assert_eq!(head(&app).await, before + 1);
    let (_, panel) = admin.get("/api/backoffice/beheer/partijen").await;
    assert_eq!(panel["partijen"][1]["toe_te_zenden"], json!([]), "{panel}");
    // Anna sees the judgment in her dossier.
    let (_, dossier) = anna.get("/api/dossier").await;
    assert!(
        dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == uitspraak_id),
        "{dossier}"
    );
}

#[tokio::test]
async fn a_decision_for_a_party_stays_within_its_chronicle() {
    let app = server().await;
    let op_bezwaar = anna_is_refused_on_objection(&app).await;
    let (status, beroep) = citizen(&app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#8:70",
                "op": op_bezwaar["id"],
                "velden": notice_of("het bezwaar is ten onrechte afgewezen"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beroep}");
    let admin = admin(&app).await;
    // A judgment "after" a record of the municipality, or of the GGD, is
    // refused as if that record did not exist.
    let advies = ggd_advises(&app, ANNA).await;
    for na in [&op_bezwaar["id"], &advies["id"]] {
        let body = json!({ "op": beroep["id"], "na": na, "bevindingen": {} });
        for path in [
            "/api/backoffice/beheer/voorvertoning",
            "/api/backoffice/beheer/besluiten",
        ] {
            let (status, fout) = admin.post(path, body.clone()).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {fout}");
        }
    }
    // The court's judgment is no decision of the executor to explain.
    let (status, uitspraak) = admin
        .post(
            "/api/backoffice/beheer/besluiten",
            json!({ "op": beroep["id"], "bevindingen": {
                    "indiener_is_belanghebbende": true,
                    "indiener_beroepschrift_niet_in_verzuim": true,
                    "rechtsregel_geschonden": false,
                } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{uitspraak}");
    let id = uitspraak["besluit"]["id"].as_str().unwrap();
    let lezer = employee(&app, "lezer-1", "wetsproces-lezer").await;
    let (status, _) = lezer.get(&format!("/api/backoffice/waarom/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_municipality_sees_its_request_run_and_the_advice_come_back() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let lezer = employee(&app, "lezer-2", "wetsproces-lezer").await;
    let verzoeken = |who: &'static str| {
        let app = &app;
        async move {
            let (status, body) = employee(app, who, "wetsproces-lezer")
                .await
                .get("/api/backoffice/verzoeken")
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body["verzoeken"].as_array().unwrap().clone()
        }
    };
    // The request runs: the municipality's own record, to the GGD.
    let lopend = verzoeken("lezer-3").await;
    assert_eq!(lopend.len(), 1);
    assert_eq!(lopend[0]["open"], json!(true));
    assert_eq!(lopend[0]["aan"], json!(GGD));
    assert_eq!(lopend[0]["aan_naam"], json!("GGD Wilgendaal"));
    assert_eq!(lopend[0]["verzoek"]["recording_actor"], json!(GEMEENTE));
    assert_eq!(lopend[0]["verzoek"]["fields"]["aanvraag"], aanvraag["id"]);
    assert_eq!(lopend[0]["antwoord_ontvangen"], Value::Null);
    // A second application of Anna while it runs asks nothing again.
    anna_applies(&app).await;
    assert_eq!(verzoeken("lezer-4").await.len(), 1);

    set_clock(&app, "2026-10-16").await;
    let advies = ggd_advises(&app, ANNA).await;
    let binnen = verzoeken("lezer-5").await;
    assert_eq!(binnen[0]["open"], json!(false));
    // What came back is the municipality's own receipt, never the GGD's
    // record: the route shows no record of another cell.
    let ontvangen = &binnen[0]["antwoord_ontvangen"];
    assert_eq!(ontvangen["recording_actor"], json!(GEMEENTE));
    assert_ne!(ontvangen["id"], advies["id"]);
    assert!(
        !binnen[0]
            .to_string()
            .contains(advies["id"].as_str().unwrap())
    );
    // The GGD's chronicle stays closed to the municipality.
    let (status, _) = lezer
        .get(&format!("/api/backoffice/cellen/{GGD}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Not a route for a citizen.
    let (status, _) = citizen(&app, "anna", ANNA)
        .await
        .get("/api/backoffice/verzoeken")
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Anna's dossier holds the request, at the municipality and at the GGD.
    let (_, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    let feiten: Vec<&str> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|g| g["fact"].as_str())
        .collect();
    for feit in [
        "geneeskundig_onderzoek_aangevraagd",
        "verzoek_om_geneeskundig_onderzoek_ontvangen",
    ] {
        assert!(feiten.contains(&feit), "{feiten:?}");
    }
    // And says which of its records belong to that request, by id, so a
    // screen hangs the receipt, the advice and its receipt under it.
    let ids: Vec<&str> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|g| g["id"].as_str())
        .collect();
    let rij = &dossier["verzoeken"][0];
    assert_eq!(
        dossier["verzoeken"].as_array().unwrap().len(),
        1,
        "{dossier}"
    );
    assert_eq!(rij["verzoek"], lopend[0]["verzoek"]["id"]);
    assert_eq!(rij["antwoord"], advies["id"]);
    for key in ["verzoek", "ontvangen", "antwoord", "antwoord_ontvangen"] {
        assert!(ids.contains(&rij[key].as_str().unwrap()), "{key}: {rij}");
    }
    // Another citizen's dossier holds none of it.
    let (_, other) = citizen(&app, "bram", "999990020")
        .await
        .get("/api/dossier")
        .await;
    assert_eq!(other["verzoeken"], json!([]), "{other}");

    // Emptying the test setup takes the requests with it; a new application
    // asks the GGD again.
    let (status, fout) = admin(&app)
        .await
        .post("/api/backoffice/testomgeving/leegmaken", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{fout}");
    assert!(verzoeken("lezer-6").await.is_empty());
    assert!(open_request(&app, ANNA).await.is_none());
    anna_applies(&app).await;
    assert!(open_request(&app, ANNA).await.is_some());
}

#[tokio::test]
async fn after_a_court_annuls_the_decision_on_objection_the_municipality_decides_again() {
    let app = server().await;
    let op_bezwaar = anna_is_refused_on_objection(&app).await;
    let bezwaar_id = op_bezwaar["refers_to"].clone();
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let anna = citizen(&app, "anna", ANNA).await;
    let admin = admin(&app).await;
    // Nothing waits for the municipality: each submission has its decision.
    let (status, queue) = beslisser.get("/api/backoffice/werkvoorraad").await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_eq!(queue["open"], json!([]), "{queue}");

    let (status, beroep) = anna
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#8:70",
                "op": op_bezwaar["id"],
                "velden": notice_of("het bezwaar is ten onrechte afgewezen"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beroep}");
    // The court annuls the decision on the objection with its legal
    // effects, and orders a new one.
    set_clock(&app, "2026-11-20").await;
    let (status, uitspraak) = admin
        .post(
            "/api/backoffice/beheer/besluiten",
            json!({ "op": beroep["id"], "bevindingen": {
                "indiener_is_belanghebbende": true,
                "indiener_beroepschrift_niet_in_verzuim": true,
                "rechtsregel_geschonden": true,
                "geschonden_rechtsregel": "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht",
                "rechtsgevolgen_in_stand": false,
                "uitspraak_treedt_in_de_plaats": false,
                "termijn_nieuw_besluit": "2026-12-18",
            } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{uitspraak}");
    assert_eq!(
        uitspraak["besluit"]["outputs"]["opdracht_nieuw_besluit"],
        json!(true),
        "{uitspraak}"
    );
    let (status, ontvangst) = admin
        .post(
            "/api/backoffice/beheer/toezendingen",
            json!({ "besluit": uitspraak["besluit"]["id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ontvangst}");

    // The objection waits again, to be decided after the receipt.
    let (_, queue) = beslisser.get("/api/backoffice/werkvoorraad").await;
    assert_eq!(queue["open"].as_array().unwrap().len(), 1, "{queue}");
    assert_eq!(queue["open"][0]["stuk"]["id"], bezwaar_id, "{queue}");
    assert_eq!(queue["open"][0]["na"], ontvangst["id"], "{queue}");
    // Anna sees the decision on her objection revoked by the receipt.
    let (_, dossier) = anna.get("/api/dossier").await;
    let stand = &dossier["stand"];
    assert_eq!(
        stand["herroepen"][op_bezwaar["id"].as_str().unwrap()],
        ontvangst["id"]
    );
    assert_eq!(
        stand["antwoorden"][bezwaar_id.as_str().unwrap()]["soort"],
        json!("vervallen"),
        "{stand}"
    );

    // The GGD advises now, and the municipality decides on the objection
    // again, after the receipt: well founded, a new decision in the place of
    // the refusal.
    ggd_advises(&app, ANNA).await;
    let mut bevindingen = findings();
    bevindingen["indiener_bezwaarschrift_is_belanghebbende"] = json!(true);
    bevindingen["indiener_bezwaarschrift_niet_in_verzuim"] = json!(true);
    let (status, opnieuw) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": bezwaar_id, "na": ontvangst["id"], "bevindingen": bevindingen }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{opnieuw}");
    assert_eq!(opnieuw["besluit"]["after"], ontvangst["id"]);
    let (_, queue) = beslisser.get("/api/backoffice/werkvoorraad").await;
    assert_eq!(queue["open"], json!([]), "{queue}");

    // What answers Anna's application now is the new decision.
    let (_, dossier) = anna.get("/api/dossier").await;
    let aanvraag = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["kind"] == json!("AANVRAAG"))
        .unwrap();
    let antwoord = &dossier["stand"]["antwoorden"][aanvraag["id"].as_str().unwrap()];
    assert_eq!(antwoord["soort"], json!("in_de_plaats"), "{dossier}");
    assert_eq!(antwoord["door"], opnieuw["besluit"]["id"]);
    assert_eq!(
        antwoord["nieuw"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true),
        "{antwoord}"
    );
}

/// A guard over the whole answers, not a test of one filter: with today's
/// links every record that revokes or answers one of a citizen's records is
/// in their dossier, so the filter on what the kern says
/// (`Stand::binnen`, tested in the kern) has nothing to drop here. Should a
/// later step link otherwise, this turns red.
#[tokio::test]
async fn no_answer_names_a_record_outside_what_its_reader_may_see() {
    // Bram applies and the GGD advises on him. Anna: refused, objection,
    // appeal; the court annuls the decision on her objection and the GGD
    // advises.
    let app = server().await;
    let (status, bram_aanvraag) = citizen(&app, "bram", BRAM)
        .await
        .post("/api/indieningen", application())
        .await;
    assert_eq!(status, StatusCode::OK, "{bram_aanvraag}");
    ggd_advises(&app, BRAM).await;
    let op_bezwaar = anna_is_refused_on_objection(&app).await;
    let anna = citizen(&app, "anna", ANNA).await;
    let admin = admin(&app).await;
    let (status, beroep) = anna
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#8:70",
                "op": op_bezwaar["id"],
                "velden": notice_of("het bezwaar is ten onrechte afgewezen"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beroep}");
    set_clock(&app, "2026-11-20").await;
    let (status, uitspraak) = admin
        .post(
            "/api/backoffice/beheer/besluiten",
            json!({ "op": beroep["id"], "bevindingen": {
                "indiener_is_belanghebbende": true,
                "indiener_beroepschrift_niet_in_verzuim": true,
                "rechtsregel_geschonden": true,
                "geschonden_rechtsregel": "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht",
                "rechtsgevolgen_in_stand": false,
                "uitspraak_treedt_in_de_plaats": false,
                "termijn_nieuw_besluit": "2026-12-18",
            } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{uitspraak}");
    let (status, _) = admin
        .post(
            "/api/backoffice/beheer/toezendingen",
            json!({ "besluit": uitspraak["besluit"]["id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    ggd_advises(&app, ANNA).await;

    // Each citizen's dossier names only records of that dossier, in its
    // records as well as in what the kern says of them.
    let mut dossiers = Vec::new();
    for (sub, bsn) in [("anna", ANNA), ("bram", BRAM)] {
        let (status, dossier) = citizen(&app, sub, bsn).await.get("/api/dossier").await;
        assert_eq!(status, StatusCode::OK, "{dossier}");
        let own: std::collections::BTreeSet<String> = dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["id"].as_str().unwrap().to_string())
            .collect();
        if bsn == ANNA {
            // What the kern says comes along: the receipt annulled the
            // decision on her objection.
            assert!(
                dossier["stand"]["herroepen"]
                    .get(op_bezwaar["id"].as_str().unwrap())
                    .is_some(),
                "{dossier}"
            );
        }
        let named = ids_in(&dossier);
        let outside: Vec<_> = named.difference(&own).collect();
        assert!(outside.is_empty(), "{sub}: {outside:?} in {dossier}");
        dossiers.push(own);
    }
    // The court and the GGD hold records in Anna's dossier, and the two
    // dossiers share none.
    assert!(dossiers[0].contains(beroep["id"].as_str().unwrap()));
    assert!(dossiers[0].contains(uitspraak["besluit"]["id"].as_str().unwrap()));
    assert!(dossiers[0].is_disjoint(&dossiers[1]), "{dossiers:?}");

    // The municipality's work queue and chronicle name only its own records
    // and what reached it from another cell (`received`).
    let lezer = employee(&app, "lezer-1", "wetsproces-lezer").await;
    let (status, kroniek) = lezer
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    let grams = kroniek["grammen"].as_array().unwrap();
    let mut may_see: std::collections::BTreeSet<String> = grams
        .iter()
        .map(|g| g["id"].as_str().unwrap().to_string())
        .collect();
    may_see.extend(
        grams
            .iter()
            .filter_map(|g| g["received"]["id"].as_str().map(str::to_string)),
    );
    let (status, queue) = lezer.get("/api/backoffice/werkvoorraad").await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    for answer in [&queue, &kroniek] {
        let outside: Vec<_> = ids_in(answer).difference(&may_see).cloned().collect();
        assert!(outside.is_empty(), "{outside:?} in {answer}");
    }
    // The appeal sits with the court only; the municipality never sees it.
    assert!(!may_see.contains(beroep["id"].as_str().unwrap()));
}

/// The ids an employee of the municipality may see: its chronicle, and what
/// reached it from another cell.
async fn seen_by_the_municipality(app: &Router) -> std::collections::BTreeSet<String> {
    let lezer = employee(app, "lezer-1", "wetsproces-lezer").await;
    let (status, kroniek) = lezer
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    let grams = kroniek["grammen"].as_array().unwrap();
    grams
        .iter()
        .map(|g| g["id"].as_str().unwrap().to_string())
        .chain(
            grams
                .iter()
                .filter_map(|g| g["received"]["id"].as_str().map(str::to_string)),
        )
        .collect()
}

#[tokio::test]
async fn the_trace_an_employee_gets_holds_nothing_of_another_cells_own_chronicle() {
    // The court annuls the decision on Anna's objection and the GGD advises;
    // the municipality previews and takes the decision again, after the
    // receipt of the judgment.
    let app = server().await;
    let op_bezwaar = anna_is_refused_on_objection(&app).await;
    let bezwaar = op_bezwaar["refers_to"].clone();
    let anna = citizen(&app, "anna", ANNA).await;
    let admin = admin(&app).await;
    let (status, beroep) = anna
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#8:70",
                "op": op_bezwaar["id"],
                "velden": notice_of("het bezwaar is ten onrechte afgewezen"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beroep}");
    set_clock(&app, "2026-11-20").await;
    let (status, uitspraak) = admin
        .post(
            "/api/backoffice/beheer/besluiten",
            json!({ "op": beroep["id"], "bevindingen": {
                "indiener_is_belanghebbende": true,
                "indiener_beroepschrift_niet_in_verzuim": true,
                "rechtsregel_geschonden": true,
                "geschonden_rechtsregel": "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht",
                "rechtsgevolgen_in_stand": false,
                "uitspraak_treedt_in_de_plaats": false,
                "termijn_nieuw_besluit": "2026-12-18",
            } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{uitspraak}");
    let (status, ontvangst) = admin
        .post(
            "/api/backoffice/beheer/toezendingen",
            json!({ "besluit": uitspraak["besluit"]["id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ontvangst}");
    ggd_advises(&app, ANNA).await;

    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let mut bevindingen = findings();
    bevindingen["indiener_bezwaarschrift_is_belanghebbende"] = json!(true);
    bevindingen["indiener_bezwaarschrift_niet_in_verzuim"] = json!(true);
    let body = json!({ "op": bezwaar, "na": ontvangst["id"], "bevindingen": bevindingen });
    let (status, preview) = beslisser
        .post("/api/backoffice/voorvertoning", body.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    // The preview carries the decision as it would be taken, with its chain.
    let waarom = &preview["voorlopig"]["waarom"];
    assert!(!waarom["keten"].as_array().unwrap().is_empty(), "{preview}");
    let (status, genomen) = beslisser.post("/api/backoffice/besluiten", body).await;
    assert_eq!(status, StatusCode::OK, "{genomen}");
    let id = genomen["besluit"]["id"].as_str().unwrap().to_string();
    let (status, uitleg) = beslisser.get(&format!("/api/backoffice/waarom/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{uitleg}");

    // None of the three names a record the municipality may not see, nor
    // holds the raw trace or how another cell read its chronicle. In this
    // case the answers the decision reads are the municipality's own (its
    // reductions over what reached it), so the last check holds for any
    // answer of another cell that a later case adds; the unit test of
    // `visibility` covers one.
    let mut may_see = seen_by_the_municipality(&app).await;
    // The decision the preview would take was never recorded: it is the
    // municipality's own.
    may_see.extend(
        preview["voorlopig"]["besluit"]["id"]
            .as_str()
            .map(str::to_string),
    );
    for answer in [&preview, &genomen, &uitleg] {
        let outside: Vec<_> = ids_in(answer).difference(&may_see).cloned().collect();
        assert!(outside.is_empty(), "{outside:?} in {answer}");
        let text = answer.to_string();
        assert!(!text.contains("\"spoor\""), "{text}");
        assert!(!text.contains(beroep["id"].as_str().unwrap()), "{text}");
        let mut foreign = Vec::new();
        answers_of_others(answer, &mut foreign);
        assert!(foreign.is_empty(), "{foreign:?}");
    }
}

/// Every answer in `v` of a cell other than the municipality that still says
/// how far that cell read its chronicle.
fn answers_of_others(v: &Value, into: &mut Vec<Value>) {
    match v {
        Value::Object(map) => {
            let who = map.get("answerer").or_else(|| map.get("door"));
            let read = ["records_read", "last_record", "gelezen", "tot_en_met"]
                .iter()
                .any(|k| map.contains_key(*k));
            if read && who.and_then(Value::as_str).is_some_and(|w| w != GEMEENTE) {
                into.push(v.clone());
            }
            map.values().for_each(|x| answers_of_others(x, into));
        }
        Value::Array(items) => items.iter().for_each(|x| answers_of_others(x, into)),
        _ => {}
    }
}
