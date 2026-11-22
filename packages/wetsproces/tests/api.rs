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
    // The advice reaches the municipality's chronicle too: one step, two
    // cells.
    assert_eq!(head(&app).await, 2);

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
    assert_eq!(head(&app).await, 3);

    // Anna's dossier: her application, the advice in both cells, and the
    // decision on her application.
    let (status, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    let kinds: Vec<&str> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), 4, "{dossier}");
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
    assert_eq!(head(&app).await, 1);

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
    assert_eq!(head(&app).await, 1);

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
    assert_eq!(
        forms["eigen_velden"].as_object().unwrap().len(),
        2,
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
    assert_eq!(head(&app).await, 1);
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
    assert_eq!(head(&app).await, 3);
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
    assert_eq!(head(&app).await, 1);
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
    assert_eq!(ook, waarom);

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
    // The requested decision and the signature are not on the form, but
    // the forms still say how the system fills them and with what, so a
    // screen that shows a recorded application names their origin.
    assert_eq!(
        aanvraag["verborgen"],
        json!(["aanvraag_ondertekend", "gevraagde_beschikking"])
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
        ["bsn_aanvrager", "naam_aanvrager", "adres_aanvrager"]
    );
    assert_eq!(
        vormen["teksten"]["adres_aanvrager"]["group"],
        json!("Uw gegevens")
    );
    assert_eq!(
        vormen["teksten"]["gevraagde_beschikking"]["label"],
        json!("Wat vraagt u aan?")
    );
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

    // The panel names the executor and the external parties, with nothing
    // waiting for them yet.
    let admin = admin(&app).await;
    let (status, panel) = admin.get("/api/backoffice/beheer/partijen").await;
    assert_eq!(status, StatusCode::OK, "{panel}");
    assert_eq!(
        panel,
        json!({
            "uitvoerder": { "cel": GEMEENTE, "cel_naam": "gemeente Wilgendaal" },
            "partijen": [
                { "cel": GGD, "cel_naam": "GGD Wilgendaal", "open": [], "toe_te_zenden": [] },
                { "cel": RECHTBANK, "cel_naam": "Rechtbank Wilgendaal", "open": [], "toe_te_zenden": [] },
            ],
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
    assert_eq!(head(&app).await, 1);

    // The GGD's advice: the answer is the GGD's record, and the
    // municipality gets it in the same step.
    let (status, advies) = admin.post(PANEL_WRITES[0], advice(ANNA)).await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    assert_eq!(advies["recording_actor"], json!(GGD));
    assert_eq!(head(&app).await, 2);

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
fn notice_of(grounds: &str) -> Value {
    json!({
        "bezwaar_of_beroepschrift_ondertekend": true,
        "naam_indiener": "A. de Vries",
        "adres_indiener": "Voorbeeldlaan 2",
        "dagtekening_bezwaar_of_beroepschrift": "2026-10-21",
        "omschrijving_bestreden_besluit": "het besluit op de aanvraag",
        "gronden_bezwaar_of_beroep": grounds,
    })
}

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
    assert_eq!(court["open"][0]["id"], beroep["id"], "{panel}");
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
