//! The timeline of one application, for the citizen and for the employee:
//! what happened about it in order, and what comes next
//! (`GET /api/dossier/tijdlijn/{id}`, `GET /api/backoffice/tijdlijn/{id}`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};

const RECEIPT: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#gedrukte_kaart_ontvangen";
const HANDOUT: &str =
    "beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#kaart_uitgereikt";

async fn timeline_of_anna(app: &Router, id: &Value) -> Value {
    let (status, t) = citizen(app, "anna", ANNA)
        .await
        .get(&format!("/api/dossier/tijdlijn/{}", id.as_str().unwrap()))
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    t
}

async fn timeline_of_employee(app: &Router, id: &Value) -> Value {
    let (status, t) = employee(app, "lezer-1", "wetsproces-lezer")
        .await
        .get(&format!(
            "/api/backoffice/tijdlijn/{}",
            id.as_str().unwrap()
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{t}");
    t
}

/// Per record of the timeline, in order, its fact or else its kind.
fn steps(t: &Value) -> Vec<String> {
    t["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            g["fact"]
                .as_str()
                .or(g["kind"].as_str())
                .unwrap()
                .to_string()
        })
        .collect()
}

/// The next steps, each as `<soort> <wie>`.
fn next(t: &Value) -> Vec<String> {
    t["volgende"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            format!(
                "{} {}",
                n["soort"].as_str().unwrap(),
                n["wie"].as_str().unwrap()
            )
        })
        .collect()
}

async fn decide(app: &Router, body: Value) -> Value {
    let (status, besluit) = employee(app, "gemeente-1", "wetsproces-beslisser")
        .await
        .post("/api/backoffice/besluiten", body)
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    besluit["besluit"].clone()
}

async fn record(app: &Router, feit: &str, about: &Value, fields: Value) {
    let (status, why) = employee(app, "behandelaar-1", "wetsproces-behandelaar")
        .await
        .post(
            "/api/backoffice/vastleggingen",
            json!({ "cel": GEMEENTE, "feit": feit, "velden": fields, "verwijst_naar": about }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{why}");
}

#[tokio::test]
async fn route_one_from_the_application_to_the_card_handed_out() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;

    // Just applied: the GGD is to answer the request, the municipality to
    // decide.
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    assert_eq!(t["stuk"], aanvraag["id"]);
    assert_eq!(
        steps(&t),
        [
            "AANVRAAG",
            "geneeskundig_onderzoek_aangevraagd",
            "verzoek_om_geneeskundig_onderzoek_ontvangen"
        ]
    );
    assert_eq!(next(&t), ["besluit uitvoerder", "antwoord externe_partij"]);
    // The case names the answer as a step to take, not by what it records.
    assert_eq!(
        t["volgende"][1]["taak"],
        json!("Geneeskundig advies uitbrengen")
    );
    // The request the system made says by which article.
    let verzoek = &t["grammen"][1]["id"];
    assert!(
        t["door_het_systeem"][verzoek.as_str().unwrap()].is_string(),
        "{t}"
    );

    set_clock(&app, "2026-10-16").await;
    ggd_advises(&app, ANNA).await;
    set_clock(&app, "2026-10-20").await;
    let besluit = decide(
        &app,
        json!({ "op": aanvraag["id"], "bevindingen": findings() }),
    )
    .await;

    // Granted: the system ordered the card, a person records its receipt,
    // and an objection can be made until the last day the decision gives.
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    assert_eq!(
        next(&t),
        ["handeling uitvoerder", "termijn indiener"],
        "{}",
        t["volgende"]
    );
    assert_eq!(t["volgende"][0]["feit"], json!(RECEIPT));
    assert_eq!(t["volgende"][0]["taak"], json!("Gedrukte kaart ontvangen"));
    assert_eq!(t["volgende"][1]["op"], besluit["id"]);
    assert_eq!(
        t["volgende"][1]["tot"],
        besluit["outputs"]["termijn_einddatum"]
    );

    set_clock(&app, "2026-11-02").await;
    record(&app, RECEIPT, &besluit["id"], json!({})).await;
    set_clock(&app, "2026-11-04").await;
    record(
        &app,
        HANDOUT,
        &besluit["id"],
        json!({ "per_post_verzonden": false }),
    )
    .await;

    let full = [
        "AANVRAAG",
        "geneeskundig_onderzoek_aangevraagd",
        "verzoek_om_geneeskundig_onderzoek_ontvangen",
        "geneeskundig_advies_uitgebracht",
        "geneeskundig_advies_ontvangen",
        "BESCHIKKING",
        "kaart_besteld_bij_drukker",
        "gedrukte_kaart_ontvangen",
        "kaart_uitgereikt",
    ];
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    assert_eq!(steps(&t), full);
    assert_eq!(next(&t), ["termijn indiener"]);
    // The same timeline from any record of it, such as the decision.
    assert_eq!(timeline_of_anna(&app, &besluit["id"]).await, t);

    // The employee sees the municipality's own records: not the GGD's
    // receipt of the request nor the advice in the GGD's chronicle.
    let m = timeline_of_employee(&app, &aanvraag["id"]).await;
    assert_eq!(
        steps(&m),
        [
            "AANVRAAG",
            "geneeskundig_onderzoek_aangevraagd",
            "geneeskundig_advies_ontvangen",
            "BESCHIKKING",
            "kaart_besteld_bij_drukker",
            "gedrukte_kaart_ontvangen",
            "kaart_uitgereikt",
        ]
    );
    assert!(
        m["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["recording_actor"] == json!(GEMEENTE)),
        "{m}"
    );

    // Once the deadline passed, nothing is left to do.
    set_clock(&app, "2026-12-02").await;
    assert_eq!(
        next(&timeline_of_anna(&app, &aanvraag["id"]).await),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn objection_appeal_annulment_and_a_new_decision() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let geweigerd = decide(&app, json!({ "op": aanvraag["id"], "bevindingen": {} })).await;
    set_clock(&app, "2026-10-21").await;
    let anna = citizen(&app, "anna", ANNA).await;
    let (status, bezwaar) = anna
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#7:11",
                "op": geweigerd["id"],
                "velden": notice_of("de kaart is ten onrechte geweigerd"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bezwaar}");
    // The objection waits for the municipality; the deadline it was made in
    // runs no longer.
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    assert!(
        next(&t).contains(&"besluit uitvoerder".to_string()),
        "{}",
        t["volgende"]
    );
    assert!(
        !t["volgende"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["soort"] == json!("termijn") && n["op"] == geweigerd["id"]),
        "{}",
        t["volgende"]
    );

    let op_bezwaar = decide(
        &app,
        json!({ "op": bezwaar["id"], "bevindingen": {
            "indiener_bezwaarschrift_is_belanghebbende": true,
            "indiener_bezwaarschrift_niet_in_verzuim": true,
        } }),
    )
    .await;
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
    // The appeal waits for the court, an external party.
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    assert!(
        next(&t).contains(&"besluit externe_partij".to_string()),
        "{}",
        t["volgende"]
    );

    set_clock(&app, "2026-11-20").await;
    let admin = admin(&app).await;
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
    // The court is to send the copy.
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    assert!(
        next(&t).contains(&"toezending externe_partij".to_string()),
        "{}",
        t["volgende"]
    );
    let (status, ontvangst) = admin
        .post(
            "/api/backoffice/beheer/toezendingen",
            json!({ "besluit": uitspraak["besluit"]["id"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ontvangst}");

    // The decision on the objection is annulled: the municipality decides
    // again, after the receipt.
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    let opnieuw = t["volgende"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["soort"] == json!("besluit"))
        .unwrap();
    assert_eq!(opnieuw["op"], bezwaar["id"]);
    assert_eq!(opnieuw["na"], ontvangst["id"]);
    assert_eq!(opnieuw["wie"], json!("uitvoerder"));
    assert_eq!(
        t["stand"]["herroepen"][op_bezwaar["id"].as_str().unwrap()],
        ontvangst["id"]
    );

    ggd_advises(&app, ANNA).await;
    let mut bevindingen = findings();
    bevindingen["indiener_bezwaarschrift_is_belanghebbende"] = json!(true);
    bevindingen["indiener_bezwaarschrift_niet_in_verzuim"] = json!(true);
    let nieuw = decide(
        &app,
        json!({ "op": bezwaar["id"], "na": ontvangst["id"], "bevindingen": bevindingen }),
    )
    .await;

    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    let ids: Vec<&Value> = t["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| &g["id"])
        .collect();
    let order = [
        &aanvraag["id"],
        &geweigerd["id"],
        &bezwaar["id"],
        &op_bezwaar["id"],
        &beroep["id"],
        &uitspraak["besluit"]["id"],
        &ontvangst["id"],
        &nieuw["id"],
    ];
    let places: Vec<usize> = order
        .iter()
        .map(|id| {
            ids.iter()
                .position(|x| x == id)
                .unwrap_or_else(|| panic!("{id} in {t}"))
        })
        .collect();
    assert!(places.windows(2).all(|w| w[0] < w[1]), "{places:?}");
    // What answers the application now is the new decision.
    assert_eq!(
        t["stand"]["antwoorden"][aanvraag["id"].as_str().unwrap()]["door"],
        nieuw["id"]
    );
    assert!(
        !next(&t).contains(&"besluit uitvoerder".to_string()),
        "{}",
        t["volgende"]
    );

    // The employee gets the court's judgment through the receipt, not the
    // appeal in the court's own chronicle.
    let m = timeline_of_employee(&app, &bezwaar["id"]).await;
    let mids = ids_in(&m["grammen"]);
    assert!(mids.contains(uitspraak["besluit"]["id"].as_str().unwrap()));
    assert!(!mids.contains(beroep["id"].as_str().unwrap()), "{m}");
}

#[tokio::test]
async fn a_citizen_timeline_holds_nothing_outside_their_dossier() {
    let app = server().await;
    let (status, bram) = citizen(&app, "bram", BRAM)
        .await
        .post("/api/indieningen", application())
        .await;
    assert_eq!(status, StatusCode::OK, "{bram}");
    let aanvraag = anna_applies(&app).await;
    set_clock(&app, "2026-10-16").await;
    ggd_advises(&app, BRAM).await;
    ggd_advises(&app, ANNA).await;
    decide(
        &app,
        json!({ "op": aanvraag["id"], "bevindingen": findings() }),
    )
    .await;

    let (_, dossier) = citizen(&app, "anna", ANNA).await.get("/api/dossier").await;
    let mine: BTreeSet<String> = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["id"].as_str().unwrap().to_string())
        .collect();
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    let outside: Vec<_> = ids_in(&t).difference(&mine).cloned().collect();
    assert!(outside.is_empty(), "{outside:?} in {t}");
    assert!(!ids_in(&t).contains(bram["id"].as_str().unwrap()));
    // Bram's own record is no record of Anna's.
    let (status, _) = citizen(&app, "anna", ANNA)
        .await
        .get(&format!(
            "/api/dossier/tijdlijn/{}",
            bram["id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Nor is an employee route a citizen's.
    let (status, _) = citizen(&app, "anna", ANNA)
        .await
        .get(&format!(
            "/api/backoffice/tijdlijn/{}",
            aanvraag["id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_lost_card_and_its_duplicate_belong_to_the_application() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    set_clock(&app, "2026-10-16").await;
    ggd_advises(&app, ANNA).await;
    let besluit = decide(
        &app,
        json!({ "op": aanvraag["id"], "bevindingen": findings() }),
    )
    .await;
    record(&app, RECEIPT, &besluit["id"], json!({})).await;
    record(
        &app,
        HANDOUT,
        &besluit["id"],
        json!({ "per_post_verzonden": false }),
    )
    .await;
    set_clock(&app, "2026-11-12").await;
    let (status, melding) = citizen(&app, "anna", ANNA)
        .await
        .post(
            "/api/indieningen",
            json!({
                "artikel": "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#52",
                "op": besluit["id"],
                "velden": {
                    "datum_vermissing": "2026-11-10",
                    "omstandigheden_verlies": "uit de auto gestolen",
                    "verklaring_naar_waarheid": true,
                },
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{melding}");
    let t = timeline_of_anna(&app, &melding["id"]).await;
    assert_eq!(t["stuk"], aanvraag["id"]);
    assert_eq!(t["volgende"][0]["soort"], json!("besluit"));
    assert_eq!(t["volgende"][0]["op"], melding["id"]);

    let duplicaat = decide(&app, json!({ "op": melding["id"], "bevindingen": {} })).await;
    let t = timeline_of_anna(&app, &aanvraag["id"]).await;
    // The report, the duplicate and its order come after the handout.
    let s = steps(&t);
    assert_eq!(
        s[s.len() - 3..],
        ["AANVRAAG", "BESCHIKKING", "kaart_besteld_bij_drukker"]
    );
    let n = t["grammen"].as_array().unwrap().len();
    assert_eq!(t["grammen"][n - 3]["id"], melding["id"]);
    assert_eq!(t["grammen"][n - 2]["id"], duplicaat["id"]);
    // Next: the receipt of the duplicate.
    assert_eq!(t["volgende"][0]["feit"], json!(RECEIPT));
    assert_eq!(t["volgende"][0]["op"], duplicaat["id"]);
}
