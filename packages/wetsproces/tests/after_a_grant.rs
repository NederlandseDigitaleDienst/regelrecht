//! The acts that follow a grant through the HTTP API (`na_toekenning` in the
//! case file): for the parking card, ordering the card from the printer, its
//! receipt by the municipality's post room and handing it out. Each comes in
//! its turn and once, refers to the record that granted, and none follows a
//! refusal or a grant revoked on objection. Employees see every grant and how
//! far it is; a citizen sees only their own.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

const POLICY: &str = "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5";

fn act(fact: &str) -> String {
    format!("{POLICY}#{fact}")
}

/// A fact of the executor about the record `about`, with `fields`.
fn record(fact: &str, about: &Value, fields: &Value) -> Value {
    json!({
        "cel": GEMEENTE,
        "feit": act(fact),
        "velden": fields,
        "verwijst_naar": about,
    })
}

/// Anna applies, the GGD advises, and the municipality grants the card on
/// 2026-10-20; the answer is the decision.
async fn anna_is_granted(app: &Router) -> Value {
    let aanvraag = anna_applies(app).await;
    set_clock(app, "2026-10-16").await;
    ggd_advises(app, ANNA).await;
    set_clock(app, "2026-10-20").await;
    let beslisser = employee(app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, besluit) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    assert_eq!(
        besluit["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    besluit["besluit"].clone()
}

/// The grants and how far each is, as a reader sees them.
async fn grants(app: &Router) -> Vec<Value> {
    let reader = employee(app, "lezer-1", "wetsproces-lezer").await;
    let (status, vervolg) = reader.get("/api/backoffice/vervolg").await;
    assert_eq!(status, StatusCode::OK, "{vervolg}");
    vervolg["vervolg"].as_array().unwrap().clone()
}

/// A handler records `body`; the answer is the status and the record or the
/// reason.
async fn handler_records(app: &Router, body: Value) -> (StatusCode, Value) {
    employee(app, "behandelaar-1", "wetsproces-behandelaar")
        .await
        .post("/api/backoffice/vastleggingen", body)
        .await
}

#[tokio::test]
async fn nothing_comes_before_its_turn_and_an_act_refers_to_the_grant() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let id = &besluit["id"];

    let vervolg = grants(&app).await;
    assert_eq!(vervolg.len(), 1, "{vervolg:?}");
    assert_eq!(vervolg[0]["toekenning"], *id);
    assert_eq!(vervolg[0]["besluit"], *id);
    assert_eq!(vervolg[0]["aanvrager"], json!(ANNA));
    assert_eq!(vervolg[0]["toegekend_op"], json!("2026-10-20"));
    assert_eq!(
        vervolg[0]["volgende"],
        json!(act("kaart_besteld_bij_drukker"))
    );

    // Nothing before its turn: no receipt and no handing out before the order.
    let (status, why) =
        handler_records(&app, record("gedrukte_kaart_ontvangen", id, &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("comes after"),
        "{why}"
    );
    let (status, why) = handler_records(
        &app,
        record(
            "kaart_uitgereikt",
            id,
            &json!({ "per_post_verzonden": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");

    // An act refers to the grant: not to nothing, and not to the application.
    let (status, why) = handler_records(
        &app,
        json!({ "cel": GEMEENTE, "feit": act("kaart_besteld_bij_drukker"), "velden": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"]
            .as_str()
            .unwrap()
            .contains("refers to the record that granted"),
        "{why}"
    );
    let (status, why) = handler_records(
        &app,
        record(
            "kaart_besteld_bij_drukker",
            &besluit["refers_to"],
            &json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("is a submission"),
        "{why}"
    );

    // The day is the system's to fill, by the clock.
    let (status, why) = handler_records(
        &app,
        record(
            "kaart_besteld_bij_drukker",
            id,
            &json!({ "datum_bestelling": "2026-10-01" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"]
            .as_str()
            .unwrap()
            .contains("filled by the system"),
        "{why}"
    );
}

#[tokio::test]
async fn each_act_comes_once_on_the_day_of_the_clock() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let id = &besluit["id"];

    set_clock(&app, "2026-10-21").await;
    let (status, besteld) =
        handler_records(&app, record("kaart_besteld_bij_drukker", id, &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{besteld}");
    assert_eq!(besteld["refers_to"], *id);
    assert_eq!(besteld["fields"]["datum_bestelling"], json!("2026-10-21"));
    // Once.
    let (status, why) =
        handler_records(&app, record("kaart_besteld_bij_drukker", id, &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("already recorded"),
        "{why}"
    );

    // The post room establishes the receipt: the record says so.
    set_clock(&app, "2026-11-02").await;
    let (status, ontvangen) =
        handler_records(&app, record("gedrukte_kaart_ontvangen", id, &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{ontvangen}");
    assert_eq!(
        ontvangen["fields"],
        json!({ "datum_ontvangst": "2026-11-02", "ontvangst_vastgesteld_door": "postkamer" })
    );
    let (status, why) = handler_records(
        &app,
        record(
            "gedrukte_kaart_ontvangen",
            id,
            &json!({ "ontvangst_vastgesteld_door": "balie" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");

    set_clock(&app, "2026-11-04").await;
    let (status, uitgereikt) = handler_records(
        &app,
        record(
            "kaart_uitgereikt",
            id,
            &json!({ "per_post_verzonden": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uitgereikt}");
    assert_eq!(
        uitgereikt["fields"]["datum_uitreiking"],
        json!("2026-11-04")
    );
    let (status, why) = handler_records(
        &app,
        record(
            "kaart_uitgereikt",
            id,
            &json!({ "per_post_verzonden": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");

    let vervolg = grants(&app).await;
    assert_eq!(vervolg[0]["volgende"], Value::Null, "{vervolg:?}");
    let dagen: Vec<&Value> = vervolg[0]["stappen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["gedaan"])
        .collect();
    assert_eq!(
        dagen,
        [
            &json!("2026-10-21"),
            &json!("2026-11-02"),
            &json!("2026-11-04")
        ]
    );
}

#[tokio::test]
async fn each_employee_sees_the_grants_and_a_handler_records_the_acts() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let order = record("kaart_besteld_bij_drukker", &besluit["id"], &json!({}));

    // A reader sees how far the card is, but records nothing.
    assert_eq!(grants(&app).await.len(), 1);
    let reader = employee(&app, "lezer-1", "wetsproces-lezer").await;
    let (status, _) = reader
        .post("/api/backoffice/vastleggingen", order.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A citizen reaches no route of the back office.
    let anna = citizen(&app, "anna", ANNA).await;
    let (status, _) = anna.get("/api/backoffice/vervolg").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = anna
        .post("/api/backoffice/vastleggingen", order.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A handler and a decider record the acts.
    let (status, besteld) = handler_records(&app, order).await;
    assert_eq!(status, StatusCode::OK, "{besteld}");
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, ontvangen) = beslisser
        .post(
            "/api/backoffice/vastleggingen",
            record("gedrukte_kaart_ontvangen", &besluit["id"], &json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ontvangen}");
}

#[tokio::test]
async fn a_citizen_sees_how_far_their_own_card_is_and_no_other() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let (status, besteld) = handler_records(
        &app,
        record("kaart_besteld_bij_drukker", &besluit["id"], &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{besteld}");

    let (status, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    assert_eq!(dossier["vervolg"].as_array().unwrap().len(), 1, "{dossier}");
    assert_eq!(dossier["vervolg"][0]["toekenning"], besluit["id"]);
    assert_eq!(
        dossier["vervolg"][0]["volgende"],
        json!(act("gedrukte_kaart_ontvangen"))
    );
    // The order is a row of her dossier, under the decision.
    assert!(
        dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == besteld["id"] && g["refers_to"] == besluit["id"]),
        "{dossier}"
    );
    // Bram's dossier holds neither her card nor its order.
    let (_, dossier) = citizen(&app, "bram", BRAM).await.get("/api/dossier").await;
    assert_eq!(dossier["vervolg"], json!([]));
    assert_eq!(dossier["grammen"], json!([]));
}

#[tokio::test]
async fn no_act_follows_a_refusal() {
    let app = server().await;
    // Without the GGD's advice the municipality refuses.
    let aanvraag = anna_applies(&app).await;
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, besluit) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": {} }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    assert_eq!(
        besluit["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    assert_eq!(grants(&app).await, Vec::<Value>::new());
    let (status, why) = handler_records(
        &app,
        record(
            "kaart_besteld_bij_drukker",
            &besluit["besluit"]["id"],
            &json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("grants nothing"),
        "{why}"
    );
}

#[tokio::test]
async fn no_act_follows_a_grant_revoked_on_objection() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let id = besluit["id"].clone();
    set_clock(&app, "2026-10-21").await;
    let (status, besteld) =
        handler_records(&app, record("kaart_besteld_bij_drukker", &id, &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{besteld}");

    // Anna objects; a new examination finds no criterion, and the
    // municipality puts a refusal in the place of the grant (Awb 7:11).
    let (status, bezwaar) = citizen(&app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#7:11",
                "op": id,
                "velden": {
                    "bezwaar_of_beroepschrift_ondertekend": true,
                    "naam_indiener": "A. de Vries",
                    "adres_indiener": "Voorbeeldlaan 2",
                    "dagtekening_bezwaar_of_beroepschrift": "2026-10-21",
                    "omschrijving_bestreden_besluit": "het besluit op de aanvraag",
                    "gronden_bezwaar_of_beroep": "ik vraag een kaart als passagier",
                },
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bezwaar}");
    set_clock(&app, "2026-11-10").await;
    let mut advies = advice(ANNA);
    advies["velden"]["datum_onderzoek"] = json!("2026-11-10");
    advies["velden"]["langdurige_loopbeperking"] = json!(false);
    let (status, advies) = admin(&app)
        .await
        .post("/api/backoffice/beheer/vastleggingen", advies)
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    set_clock(&app, "2026-11-20").await;
    let mut bevindingen = findings();
    bevindingen["langdurige_loopbeperking_volgens_college"] = json!(false);
    bevindingen["indiener_bezwaarschrift_is_belanghebbende"] = json!(true);
    bevindingen["indiener_bezwaarschrift_niet_in_verzuim"] = json!(true);
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, beslissing) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": bezwaar["id"], "bevindingen": bevindingen }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beslissing}");
    assert_eq!(
        beslissing["besluit"]["outputs"]["nieuw_besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(false),
        "{beslissing}"
    );

    // The card is no longer granted: it leaves the list, and the next act is
    // refused, as is any act referring to the decision on the objection.
    assert_eq!(grants(&app).await, Vec::<Value>::new());
    let (status, why) =
        handler_records(&app, record("gedrukte_kaart_ontvangen", &id, &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("grants nothing"),
        "{why}"
    );
    let (status, why) = handler_records(
        &app,
        record(
            "kaart_besteld_bij_drukker",
            &beslissing["besluit"]["id"],
            &json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("grants nothing"),
        "{why}"
    );
}

#[tokio::test]
async fn emptying_the_test_setup_empties_the_acts_too() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let (status, _) = handler_records(
        &app,
        record("kaart_besteld_bij_drukker", &besluit["id"], &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = admin(&app)
        .await
        .post("/api/backoffice/testomgeving/leegmaken", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grants(&app).await, Vec::<Value>::new());
}

#[tokio::test]
async fn a_grant_on_objection_is_the_record_the_acts_refer_to() {
    let app = server().await;
    // Without the GGD's advice the municipality refuses.
    let aanvraag = anna_applies(&app).await;
    let beslisser = employee(&app, "gemeente-1", "wetsproces-beslisser").await;
    let (status, weigering) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": {} }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{weigering}");
    let weigering = weigering["besluit"].clone();
    set_clock(&app, "2026-10-21").await;
    let (status, bezwaar) = citizen(&app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#7:11",
                "op": weigering["id"],
                "velden": {
                    "bezwaar_of_beroepschrift_ondertekend": true,
                    "naam_indiener": "A. de Vries",
                    "adres_indiener": "Voorbeeldlaan 2",
                    "dagtekening_bezwaar_of_beroepschrift": "2026-10-21",
                    "omschrijving_bestreden_besluit": "het besluit op de aanvraag",
                    "gronden_bezwaar_of_beroep": "ik heb een loopbeperking",
                },
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bezwaar}");
    // The GGD examines her now, on the request her application brought
    // about, and on the objection the municipality puts a grant in the place
    // of the refusal (Awb 7:11).
    set_clock(&app, "2026-11-10").await;
    let mut advies = advice(ANNA);
    advies["velden"]["datum_onderzoek"] = json!("2026-11-10");
    advies["verzoek"] = open_request(&app, ANNA)
        .await
        .expect("an open request about Anna")["id"]
        .clone();
    let (status, advies) = admin(&app)
        .await
        .post("/api/backoffice/beheer/vastleggingen", advies)
        .await;
    assert_eq!(status, StatusCode::OK, "{advies}");
    set_clock(&app, "2026-11-20").await;
    let mut bevindingen = findings();
    bevindingen["indiener_bezwaarschrift_is_belanghebbende"] = json!(true);
    bevindingen["indiener_bezwaarschrift_niet_in_verzuim"] = json!(true);
    let (status, beslissing) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": bezwaar["id"], "bevindingen": bevindingen }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{beslissing}");
    let beslissing = beslissing["besluit"].clone();

    // The grant hangs in the aggregate of the refusal, and the decision on
    // the objection is the record that granted.
    let vervolg = grants(&app).await;
    assert_eq!(vervolg.len(), 1, "{vervolg:?}");
    assert_eq!(vervolg[0]["besluit"], weigering["id"]);
    assert_eq!(vervolg[0]["toekenning"], beslissing["id"]);
    let (status, why) = handler_records(
        &app,
        record("kaart_besteld_bij_drukker", &weigering["id"], &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    let (status, besteld) = handler_records(
        &app,
        record("kaart_besteld_bij_drukker", &beslissing["id"], &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{besteld}");
    let (_, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(
        dossier["vervolg"][0]["toekenning"], beslissing["id"],
        "{dossier}"
    );
}
