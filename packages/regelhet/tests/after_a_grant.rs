//! The acts that follow a grant through the HTTP API (`na_toekenning` in the
//! case file): for the parking card, ordering the card from the printer, its
//! receipt by the municipality's post room and handing it out. The order is
//! the system's: it lands with the step that makes a grant count, recorded by
//! the article of the policy that has the system take it. The other two are a
//! person's. Each comes in its turn and once, refers to the record that
//! granted, and none follows a refusal or a grant revoked on objection.
//! Employees see every grant and how far it is; a citizen sees only their
//! own.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::{Method, StatusCode};
use common::*;
use serde_json::{Value, json};

const ORDER: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#7#kaart_besteld_bij_drukker";
const ORDER_ARTICLE: &str = "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#7";
const RECEIPT: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#gedrukte_kaart_ontvangen";
const HANDOUT: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#kaart_uitgereikt";

/// A fact `feit` of the executor about the record `about`, with `fields`.
fn record(feit: &str, about: &Value, fields: &Value) -> Value {
    json!({
        "cel": GEMEENTE,
        "feit": feit,
        "velden": fields,
        "verwijst_naar": about,
    })
}

/// The decision on Anna's application, with the findings that grant.
fn granting(aanvraag: &Value) -> Value {
    json!({ "op": aanvraag["id"], "bevindingen": findings() })
}

/// Anna applies and the GGD advises; the answer is the application, and the
/// clock stands on 2026-10-20.
async fn anna_is_advised(app: &Router) -> Value {
    let aanvraag = anna_applies(app).await;
    set_clock(app, "2026-10-16").await;
    ggd_advises(app, ANNA).await;
    set_clock(app, "2026-10-20").await;
    aanvraag
}

/// Anna applies, the GGD advises, and the municipality grants the card on
/// 2026-10-20; the answer is the decision.
async fn anna_is_granted(app: &Router) -> Value {
    let aanvraag = anna_is_advised(app).await;
    let beslisser = employee(app, "gemeente-1", "regelhet-beslisser").await;
    let (status, besluit) = beslisser
        .post("/api/backoffice/besluiten", granting(&aanvraag))
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    assert_eq!(
        besluit["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    besluit["besluit"].clone()
}

/// The executor's chronicle as a reader sees it.
async fn chronicle(app: &Router) -> Value {
    let reader = employee(app, "lezer-1", "regelhet-lezer").await;
    let (status, kroniek) = reader
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    assert_eq!(status, StatusCode::OK, "{kroniek}");
    kroniek
}

/// The orders in the executor's chronicle about `grant`.
async fn orders_of(app: &Router, grant: &Value) -> Vec<Value> {
    chronicle(app).await["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["fact"] == json!("kaart_besteld_bij_drukker") && g["refers_to"] == *grant)
        .cloned()
        .collect()
}

/// The order about `grant`, if the system made one, and at most one.
async fn order_of(app: &Router, grant: &Value) -> Option<Value> {
    let orders = orders_of(app, grant).await;
    assert!(orders.len() <= 1, "{orders:?}");
    orders.into_iter().next()
}

/// The grants and how far each is, as a reader sees them.
async fn grants(app: &Router) -> Vec<Value> {
    let reader = employee(app, "lezer-1", "regelhet-lezer").await;
    let (status, vervolg) = reader.get("/api/backoffice/vervolg").await;
    assert_eq!(status, StatusCode::OK, "{vervolg}");
    vervolg["vervolg"].as_array().unwrap().clone()
}

/// A decider records `body`; the answer is the status and the record or the
/// reason.
async fn decider_records(app: &Router, body: Value) -> (StatusCode, Value) {
    employee(app, "beslisser-1", "regelhet-beslisser")
        .await
        .post("/api/backoffice/vastleggingen", body)
        .await
}

fn reason(why: &Value) -> &str {
    why["fout"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn a_grant_brings_the_order_in_the_same_step_by_the_article_of_the_policy() {
    let app = server().await;
    let aanvraag = anna_is_advised(&app).await;
    let before = head(&app).await;
    let besluit = anna_decides(&app, &aanvraag).await;
    // The decision and the order: one step, two records.
    assert_eq!(head(&app).await, before + 2);
    let order = order_of(&app, &besluit["id"]).await.unwrap();
    assert_eq!(order["fields"], json!({ "datum_bestelling": "2026-10-20" }));
    assert_eq!(order["basis"], json!(ORDER_ARTICLE));
    // The chronicle says the system made it, by article 7.
    let kroniek = chronicle(&app).await;
    assert_eq!(
        kroniek["door_het_systeem"][order["id"].as_str().unwrap()],
        json!(ORDER_ARTICLE),
        "{kroniek}"
    );
    assert!(
        kroniek["door_het_systeem"]
            .get(besluit["id"].as_str().unwrap())
            .is_none()
    );
    // The next act is a person's: the receipt.
    let vervolg = grants(&app).await;
    assert_eq!(vervolg[0]["volgende"], json!(RECEIPT));
    assert_eq!(vervolg[0]["stappen"][0]["gedaan"], json!("2026-10-20"));
    assert_eq!(vervolg[0]["aanvrager"], json!(ANNA));
}

#[tokio::test]
async fn an_employee_sees_whom_a_grant_is_for_by_name_and_a_citizen_gets_no_such_field() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let kroniek = chronicle(&app).await;
    let aanvraag = kroniek["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == besluit["refers_to"])
        .cloned()
        .unwrap();
    // The name the application recorded from the login.
    let naam = aanvraag["fields"]["naam_aanvrager"].clone();
    assert!(naam.is_string(), "{aanvraag}");
    let vervolg = grants(&app).await;
    assert_eq!(vervolg[0]["naam"], naam);
    let (status, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    // Her own row of the grant is there, without the name the back office gets.
    assert_eq!(dossier["vervolg"][0]["besluit"], besluit["id"], "{dossier}");
    assert!(dossier["vervolg"][0].get("naam").is_none(), "{dossier}");
}

/// The decider grants Anna's application; the answer is the decision.
async fn anna_decides(app: &Router, aanvraag: &Value) -> Value {
    let beslisser = employee(app, "gemeente-1", "regelhet-beslisser").await;
    let (status, besluit) = beslisser
        .post("/api/backoffice/besluiten", granting(aanvraag))
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    besluit["besluit"].clone()
}

#[tokio::test]
async fn a_repeated_decision_under_its_key_orders_nothing_twice() {
    let app = server().await;
    let aanvraag = anna_is_advised(&app).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let send = || {
        beslisser.call(
            Method::POST,
            "/api/backoffice/besluiten",
            Some(granting(&aanvraag)),
            Some("besluit-met-bestelling"),
        )
    };
    let (status, first) = send().await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let after = head(&app).await;
    let (status, again) = send().await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, first);
    assert_eq!(head(&app).await, after);
    assert_eq!(orders_of(&app, &first["besluit"]["id"]).await.len(), 1);
}

#[tokio::test]
async fn the_preview_of_a_decision_says_what_the_system_then_does() {
    let app = server().await;
    let aanvraag = anna_is_advised(&app).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let before = head(&app).await;
    let (status, preview) = beslisser
        .post("/api/backoffice/voorvertoning", granting(&aanvraag))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["automatisch"][0]["feit"], json!(ORDER), "{preview}");
    assert_eq!(preview["automatisch"][0]["artikel"], json!(ORDER_ARTICLE));
    // A preview records nothing; one that refuses foresees nothing. Bram
    // applies without an advice of the GGD, which the policy then refuses.
    assert_eq!(head(&app).await, before);
    let (status, bram) = citizen(&app, "bram", BRAM)
        .await
        .post("/api/indieningen", application())
        .await;
    assert_eq!(status, StatusCode::OK, "{bram}");
    let (status, refusal) = beslisser
        .post(
            "/api/backoffice/voorvertoning",
            json!({ "op": bram["id"], "bevindingen": {} }),
        )
        .await;
    assert_eq!(
        refusal["voorlopig"]["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(false),
        "{refusal}"
    );
    assert_eq!(status, StatusCode::OK, "{refusal}");
    assert_eq!(refusal["automatisch"], json!([]), "{refusal}");
}

#[tokio::test]
async fn nobody_orders_by_hand_and_a_persons_act_waits_its_turn() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let id = &besluit["id"];

    // The order is the system's: a decider cannot record it, also not a
    // second time.
    let (status, why) = decider_records(&app, record(ORDER, id, &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(reason(&why).contains("records no fact"), "{why}");

    // Nothing before its turn: no handing out before the receipt.
    let (status, why) = decider_records(
        &app,
        record(HANDOUT, id, &json!({ "per_post_verzonden": false })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(reason(&why).contains("comes after"), "{why}");

    // An act refers to the grant: not to nothing, and not to the application.
    let (status, why) = decider_records(
        &app,
        json!({ "cel": GEMEENTE, "feit": RECEIPT, "velden": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        reason(&why).contains("refers to the record that granted"),
        "{why}"
    );
    let (status, why) =
        decider_records(&app, record(RECEIPT, &besluit["refers_to"], &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(reason(&why).contains("is a submission"), "{why}");

    // The day and the post room are the system's to fill.
    let (status, why) = decider_records(
        &app,
        record(RECEIPT, id, &json!({ "datum_ontvangst": "2026-10-01" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(reason(&why).contains("filled by the system"), "{why}");
}

#[tokio::test]
async fn the_post_room_receives_and_the_card_is_handed_out_once_each() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let id = &besluit["id"];

    set_clock(&app, "2026-11-02").await;
    let (status, ontvangen) = decider_records(&app, record(RECEIPT, id, &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{ontvangen}");
    assert_eq!(
        ontvangen["fields"],
        json!({ "datum_ontvangst": "2026-11-02", "ontvangst_vastgesteld_door": "postkamer" })
    );
    // A person's act is no record of the system.
    let kroniek = chronicle(&app).await;
    assert!(
        kroniek["door_het_systeem"]
            .get(ontvangen["id"].as_str().unwrap())
            .is_none()
    );
    let (status, why) = decider_records(&app, record(RECEIPT, id, &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(reason(&why).contains("already recorded"), "{why}");

    set_clock(&app, "2026-11-04").await;
    let (status, uitgereikt) = decider_records(
        &app,
        record(HANDOUT, id, &json!({ "per_post_verzonden": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uitgereikt}");
    assert_eq!(
        uitgereikt["fields"]["datum_uitreiking"],
        json!("2026-11-04")
    );
    let (status, why) = decider_records(
        &app,
        record(HANDOUT, id, &json!({ "per_post_verzonden": false })),
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
            &json!("2026-10-20"),
            &json!("2026-11-02"),
            &json!("2026-11-04")
        ]
    );
}

#[tokio::test]
async fn each_employee_sees_the_grants_and_a_decider_records_the_acts() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let receipt = record(RECEIPT, &besluit["id"], &json!({}));

    // A reader sees how far the card is, but records nothing.
    assert_eq!(grants(&app).await.len(), 1);
    let reader = employee(&app, "lezer-1", "regelhet-lezer").await;
    let (status, _) = reader
        .post("/api/backoffice/vastleggingen", receipt.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A citizen reaches no route of the back office.
    let anna = citizen(&app, "anna", ANNA).await;
    let (status, _) = anna.get("/api/backoffice/vervolg").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = anna
        .post("/api/backoffice/vastleggingen", receipt.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A decider records the acts.
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (status, ontvangen) = beslisser
        .post("/api/backoffice/vastleggingen", receipt)
        .await;
    assert_eq!(status, StatusCode::OK, "{ontvangen}");
}

#[tokio::test]
async fn a_citizen_sees_how_far_their_own_card_is_and_no_other() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let order = order_of(&app, &besluit["id"]).await.unwrap();

    let (status, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(status, StatusCode::OK, "{dossier}");
    assert_eq!(dossier["vervolg"].as_array().unwrap().len(), 1, "{dossier}");
    assert_eq!(dossier["vervolg"][0]["toekenning"], besluit["id"]);
    assert_eq!(dossier["vervolg"][0]["volgende"], json!(RECEIPT));
    // The order is a row of her dossier, under the decision, made by the
    // system.
    assert!(
        dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == order["id"] && g["refers_to"] == besluit["id"]),
        "{dossier}"
    );
    assert_eq!(
        dossier["door_het_systeem"][order["id"].as_str().unwrap()],
        json!(ORDER_ARTICLE)
    );
    // Bram's dossier holds neither her card nor its order.
    let (_, dossier) = citizen(&app, "bram", BRAM).await.get("/api/dossier").await;
    assert_eq!(dossier["vervolg"], json!([]));
    assert_eq!(dossier["grammen"], json!([]));
    assert_eq!(dossier["door_het_systeem"], json!({}));
}

#[tokio::test]
async fn no_act_follows_a_refusal() {
    let app = server().await;
    // Without the GGD's advice the municipality refuses.
    let aanvraag = anna_applies(&app).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
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
    assert!(order_of(&app, &besluit["besluit"]["id"]).await.is_none());
    let (status, why) =
        decider_records(&app, record(RECEIPT, &besluit["besluit"]["id"], &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(reason(&why).contains("grants nothing"), "{why}");
}

#[tokio::test]
async fn no_act_follows_a_grant_revoked_on_objection() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    let id = besluit["id"].clone();
    // The system ordered the card with the grant.
    assert!(order_of(&app, &id).await.is_some());
    set_clock(&app, "2026-10-21").await;

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
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
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

    // The card is no longer granted: it leaves the list, the system orders
    // nothing for the refusal put in its place, and the next act is refused,
    // also when it refers to the decision on the objection.
    assert!(order_of(&app, &beslissing["besluit"]["id"]).await.is_none());
    assert_eq!(grants(&app).await, Vec::<Value>::new());
    let (status, why) = decider_records(&app, record(RECEIPT, &id, &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("grants nothing"),
        "{why}"
    );
    let (status, why) = decider_records(
        &app,
        record(RECEIPT, &beslissing["besluit"]["id"], &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    assert!(
        why["fout"].as_str().unwrap().contains("grants nothing"),
        "{why}"
    );
}

#[tokio::test]
async fn a_grant_on_objection_is_the_record_the_acts_refer_to() {
    let app = server().await;
    // Without the GGD's advice the municipality refuses.
    let aanvraag = anna_applies(&app).await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
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
    // The system ordered the card for the decision on the objection, and
    // nothing for the refusal.
    assert!(order_of(&app, &beslissing["id"]).await.is_some());
    assert!(order_of(&app, &weigering["id"]).await.is_none());
    assert_eq!(vervolg[0]["volgende"], json!(RECEIPT));
    let (_, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert_eq!(
        dossier["vervolg"][0]["toekenning"], beslissing["id"],
        "{dossier}"
    );
}

#[tokio::test]
async fn emptying_the_test_setup_empties_the_acts_too() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    assert!(order_of(&app, &besluit["id"]).await.is_some());
    let (status, _) = admin(&app)
        .await
        .post("/api/backoffice/testomgeving/leegmaken", json!({}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grants(&app).await, Vec::<Value>::new());
}
