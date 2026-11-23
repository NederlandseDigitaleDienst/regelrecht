//! A second process in the case: a citizen whose card was handed out
//! reports it lost and applies for a duplicate in one submission (BABW,
//! article 52, under the case's `na_toekenning.aanvragen`). The municipality
//! decides on it as on any application. A duplicate takes the place of the
//! lost card, which no longer counts (article 53, first paragraph, under b),
//! and the duplicate goes through the acts after a grant again, starting with
//! the order the system makes itself. No request to the GGD follows.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

const DUPLICATE: &str = "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#52";
const RECEIPT: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#gedrukte_kaart_ontvangen";
const HANDOUT: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#kaart_uitgereikt";
const GGD_REQUEST: &str = "geneeskundig_onderzoek_aangevraagd";

/// A fact `feit` of the executor about the record `about`.
fn act(feit: &str, about: &Value, fields: &Value) -> Value {
    json!({ "cel": GEMEENTE, "feit": feit, "velden": fields, "verwijst_naar": about })
}

/// What a citizen fills in to report the card lost: the rest the system fills.
fn report() -> Value {
    json!({
        "datum_vermissing": "2026-11-10",
        "omstandigheden_verlies": "uit de auto gestolen bij het station",
        "verklaring_naar_waarheid": true,
    })
}

async fn decider_records(app: &Router, body: Value) -> (StatusCode, Value) {
    employee(app, "beslisser-1", "wetsproces-beslisser")
        .await
        .post("/api/backoffice/vastleggingen", body)
        .await
}

async fn decide(app: &Router, op: &Value, findings: Value) -> Value {
    let (status, besluit) = employee(app, "gemeente-1", "wetsproces-beslisser")
        .await
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": op, "bevindingen": findings }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    besluit["besluit"].clone()
}

/// Anna is granted a card on 2026-10-20; the answer is the decision.
async fn anna_is_granted(app: &Router) -> Value {
    let aanvraag = anna_applies(app).await;
    set_clock(app, "2026-10-16").await;
    ggd_advises(app, ANNA).await;
    set_clock(app, "2026-10-20").await;
    decide(app, &aanvraag["id"], findings()).await
}

/// Anna's card is granted, received by the post room and handed out on
/// 2026-11-04; the answer is the decision that granted it.
async fn anna_holds_her_card(app: &Router) -> Value {
    let besluit = anna_is_granted(app).await;
    set_clock(app, "2026-11-02").await;
    let (status, why) = decider_records(app, act(RECEIPT, &besluit["id"], &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{why}");
    set_clock(app, "2026-11-04").await;
    let (status, why) = decider_records(
        app,
        act(
            HANDOUT,
            &besluit["id"],
            &json!({ "per_post_verzonden": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{why}");
    besluit
}

/// Anna reports the card that `grant` granted lost; the answer is the status
/// and the submission or the reason.
async fn anna_reports(app: &Router, grant: &Value, fields: Value) -> (StatusCode, Value) {
    citizen(app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({ "artikel": DUPLICATE, "op": grant, "velden": fields }),
        )
        .await
}

/// The grants and how far each is, as a reader sees them.
async fn grants(app: &Router) -> Vec<Value> {
    let (status, vervolg) = employee(app, "lezer-1", "wetsproces-lezer")
        .await
        .get("/api/backoffice/vervolg")
        .await;
    assert_eq!(status, StatusCode::OK, "{vervolg}");
    vervolg["vervolg"].as_array().unwrap().clone()
}

/// How many requests to the GGD the municipality's chronicle holds.
async fn ggd_requests(app: &Router) -> usize {
    let (_, kroniek) = employee(app, "lezer-1", "wetsproces-lezer")
        .await
        .get(&format!("/api/backoffice/cellen/{GEMEENTE}/kroniek"))
        .await;
    kroniek["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["fact"] == json!(GGD_REQUEST))
        .count()
}

#[tokio::test]
async fn a_lost_card_is_replaced_by_a_duplicate_that_goes_through_the_acts_again() {
    let app = server().await;
    let besluit = anna_holds_her_card(&app).await;
    let requests = ggd_requests(&app).await;

    set_clock(&app, "2026-11-12").await;
    let (status, melding) = anna_reports(&app, &besluit["id"], report()).await;
    assert_eq!(status, StatusCode::OK, "{melding}");
    assert_eq!(melding["kind"], json!("AANVRAAG"));
    assert_eq!(melding["refers_to"], besluit["id"]);
    // The system fills the number, the day and the signature, as in the
    // case's application; nothing comes over from the decision it refers to.
    assert_eq!(melding["fields"]["bsn_aanvrager"], json!(ANNA));
    assert_eq!(melding["fields"]["dagtekening"], json!("2026-11-12"));
    assert_eq!(melding["fields"]["aanvraag_ondertekend"], json!(true));
    // No new examination: no request to the GGD.
    assert_eq!(ggd_requests(&app).await, requests);

    // The report waits in the work queue as any application.
    let (_, wachtrij) = employee(&app, "gemeente-1", "wetsproces-beslisser")
        .await
        .get("/api/backoffice/werkvoorraad")
        .await;
    assert!(
        wachtrij
            .to_string()
            .contains(melding["id"].as_str().unwrap()),
        "{wachtrij}"
    );

    set_clock(&app, "2026-11-16").await;
    let duplicaat = decide(&app, &melding["id"], json!({})).await;
    assert_eq!(
        duplicaat["outputs"]["duplicaat_verstrekt"],
        json!(true),
        "{duplicaat}"
    );
    assert_eq!(ggd_requests(&app).await, requests);

    // The duplicate takes the place of the lost card: the acts start again,
    // at the decision on the report, and the system ordered it already.
    let vervolg = grants(&app).await;
    assert_eq!(vervolg.len(), 1, "{vervolg:?}");
    assert_eq!(vervolg[0]["besluit"], besluit["id"]);
    assert_eq!(vervolg[0]["toekenning"], duplicaat["id"]);
    assert_eq!(vervolg[0]["stappen"][0]["gedaan"], json!("2026-11-16"));
    assert_eq!(vervolg[0]["stappen"][1]["gedaan"], Value::Null);
    assert_eq!(vervolg[0]["volgende"], json!(RECEIPT));
    // Acts about the lost card are refused now; the duplicate's come in turn.
    let (status, why) = decider_records(&app, act(RECEIPT, &besluit["id"], &json!({}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    let (status, why) = decider_records(&app, act(RECEIPT, &duplicaat["id"], &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{why}");

    // Anna sees the report, the decision and the order in her dossier.
    let (_, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    let ids: Vec<&Value> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| &g["id"])
        .collect();
    assert!(
        ids.contains(&&melding["id"]) && ids.contains(&&duplicaat["id"]),
        "{dossier}"
    );
    assert_eq!(dossier["vervolg"][0]["toekenning"], duplicaat["id"]);
}

#[tokio::test]
async fn no_duplicate_for_a_card_that_was_not_handed_out() {
    let app = server().await;
    let besluit = anna_is_granted(&app).await;
    set_clock(&app, "2026-10-25").await;
    let (status, melding) = anna_reports(&app, &besluit["id"], report()).await;
    assert_eq!(status, StatusCode::OK, "{melding}");
    let duplicaat = decide(&app, &melding["id"], json!({})).await;
    assert_eq!(
        duplicaat["outputs"]["duplicaat_verstrekt"],
        json!(false),
        "{duplicaat}"
    );
    // The card stays where it was: ordered, waiting for the post room.
    let vervolg = grants(&app).await;
    assert_eq!(vervolg[0]["toekenning"], besluit["id"]);
    assert_eq!(vervolg[0]["volgende"], json!(RECEIPT));
}

#[tokio::test]
async fn a_report_without_the_declaration_is_refused() {
    let app = server().await;
    let besluit = anna_holds_her_card(&app).await;
    let mut fields = report();
    fields["verklaring_naar_waarheid"] = json!(false);
    let (status, melding) = anna_reports(&app, &besluit["id"], fields).await;
    assert_eq!(status, StatusCode::OK, "{melding}");
    let duplicaat = decide(&app, &melding["id"], json!({})).await;
    assert_eq!(duplicaat["outputs"]["duplicaat_verstrekt"], json!(false));
    assert_eq!(grants(&app).await[0]["toekenning"], besluit["id"]);
}

#[tokio::test]
async fn a_second_report_while_the_first_runs_gets_no_second_duplicate() {
    let app = server().await;
    let besluit = anna_holds_her_card(&app).await;
    set_clock(&app, "2026-11-12").await;
    let (_, eerste) = anna_reports(&app, &besluit["id"], report()).await;
    let (status, tweede) = anna_reports(&app, &besluit["id"], report()).await;
    assert_eq!(status, StatusCode::OK, "{tweede}");
    let duplicaat = decide(&app, &eerste["id"], json!({})).await;
    assert_eq!(duplicaat["outputs"]["duplicaat_verstrekt"], json!(true));
    // The duplicate is not handed out yet, so the second report finds no
    // card to replace.
    let nog_een = decide(&app, &tweede["id"], json!({})).await;
    assert_eq!(nog_een["outputs"]["duplicaat_verstrekt"], json!(false));
    assert_eq!(grants(&app).await[0]["toekenning"], duplicaat["id"]);
}

#[tokio::test]
async fn a_citizen_reports_only_their_own_card_and_the_system_fills_its_fields() {
    let app = server().await;
    let besluit = anna_holds_her_card(&app).await;
    // Bram cannot report Anna's card.
    let (status, _) = citizen(&app, "bram", BRAM)
        .await
        .post(
            "/api/indieningen",
            json!({ "artikel": DUPLICATE, "op": besluit["id"], "velden": report() }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A report names the card it is about.
    let (status, _) = citizen(&app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({ "artikel": DUPLICATE, "velden": report() }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // The day and the signature are the system's to fill.
    let mut fields = report();
    fields["dagtekening"] = json!("2026-01-01");
    let (status, why) = anna_reports(&app, &besluit["id"], fields).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}");
    // The form of the report shows what the system fills, with its value.
    let (_, forms) = citizen(&app, "anna", ANNA)
        .await
        .get("/api/formulier")
        .await;
    let systeem = &forms["indieningen"][DUPLICATE]["systeem"];
    assert_eq!(
        systeem["aanvraag_ondertekend"]["waarde"],
        json!(true),
        "{systeem}"
    );
    assert_eq!(systeem["dagtekening"]["soort"], json!("datum"), "{systeem}");
}

#[tokio::test]
async fn a_report_is_about_the_card_it_names_and_the_duplicate_keeps_its_last_day() {
    let app = server().await;
    let besluit = anna_holds_her_card(&app).await;
    let geldig_tot = besluit["outputs"]["kaart_geldig_tot"].clone();
    set_clock(&app, "2026-11-12").await;
    let (_, melding) = anna_reports(&app, &besluit["id"], report()).await;
    // The system records which card the report is about.
    assert_eq!(melding["fields"]["verloren_kaart"], besluit["id"]);
    let duplicaat = decide(&app, &melding["id"], json!({})).await;
    assert_eq!(duplicaat["outputs"]["duplicaat_verstrekt"], json!(true));

    // The duplicate is received and handed out; then Anna reports the lost
    // card again. That card no longer counts, so no second duplicate comes
    // of it.
    set_clock(&app, "2026-11-20").await;
    let (status, why) = decider_records(&app, act(RECEIPT, &duplicaat["id"], &json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{why}");
    let (status, why) = decider_records(
        &app,
        act(
            HANDOUT,
            &duplicaat["id"],
            &json!({ "per_post_verzonden": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{why}");
    let (status, weer) = anna_reports(&app, &besluit["id"], report()).await;
    assert_eq!(status, StatusCode::OK, "{weer}");
    let nee = decide(&app, &weer["id"], json!({})).await;
    assert_eq!(nee["outputs"]["duplicaat_verstrekt"], json!(false));

    // A report about the refusal or about nothing granted is refused too:
    // the card it names must be the one that counts now.
    let (_, rows) = employee(&app, "lezer-1", "wetsproces-lezer")
        .await
        .get("/api/backoffice/vervolg")
        .await;
    assert_eq!(rows["vervolg"][0]["toekenning"], duplicaat["id"]);
    // The duplicate keeps the last day of the lost card.
    let (_, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    assert!(geldig_tot.is_string(), "{besluit}");
    assert!(
        dossier.to_string().contains(geldig_tot.as_str().unwrap()),
        "{dossier}"
    );
    // A report about the duplicate itself, once it is handed out, is possible.
    let (status, over_duplicaat) = anna_reports(&app, &duplicaat["id"], report()).await;
    assert_eq!(status, StatusCode::OK, "{over_duplicaat}");
    let tweede = decide(&app, &over_duplicaat["id"], json!({})).await;
    assert_eq!(tweede["outputs"]["duplicaat_verstrekt"], json!(true));
}
