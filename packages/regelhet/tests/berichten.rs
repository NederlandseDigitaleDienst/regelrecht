//! The messages to the citizen, on the parking card of Wilgendaal: the forms
//! carry the message spec (`berichten.yaml`) and the text layer of the
//! municipality (`berichtteksten.yaml`), and every record a message follows
//! from is bound to exactly one kind of message: the acknowledgement of the
//! application, the invitation to complete it (Awb 4:5), the decision as a
//! grant and as a refusal, and the decision on the objection. Each kind
//! names the outcomes and fields its keys read, and each record has them.
//!
//! With `BERICHT_UIT` set, the test writes per record what a screen makes
//! the message from as JSON in a directory of that name, for an export.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{
    ANNA, GEMEENTE, admin, advice, anna_applies, citizen, employee, findings, ggd_advises, server,
    set_clock,
};
use serde_json::{Value, json};

const BABW: &str = "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49";
const HERSTEL: &str =
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#12#verzoek_tot_aanvulling_verzonden";

/// What a screen makes the message of `record` from, kept under `naam`.
async fn keep(app: &Router, naam: &str, record: &Value, uitkomsten: Option<&Value>) {
    let Ok(dir) = std::env::var("BERICHT_UIT") else {
        return;
    };
    let anna = citizen(app, "anna", ANNA).await;
    let (_, dossier) = anna.get("/api/dossier").await;
    let (_, forms) = anna.get("/api/formulier").await;
    let id = record["id"].as_str().unwrap();
    let (_, why) = anna.get(&format!("/api/dossier/waarom/{id}")).await;
    let dir = std::path::Path::new(&dir).join(naam);
    std::fs::create_dir_all(&dir).unwrap();
    for (bestand, waarde) in [
        ("dossier", dossier),
        ("formulier", forms),
        ("waarom", why),
        ("stuk", record.clone()),
        ("uitkomsten", uitkomsten.cloned().unwrap_or(Value::Null)),
    ] {
        std::fs::write(
            dir.join(format!("{bestand}.json")),
            serde_json::to_string_pretty(&waarde).unwrap(),
        )
        .unwrap();
    }
}

/// The kind of message `record` gets in `spec`, as the screen finds it: by
/// role, article and, for a fact, the fact.
fn soort<'s>(spec: &'s Value, record: &Value, rol: &str) -> Vec<&'s Value> {
    let feit = record["fact"]
        .as_str()
        .map(|f| format!("{}#{f}", record["basis"].as_str().unwrap()));
    spec["berichtsoorten"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            let v = &s["vastlegging"];
            v["rol"] == rol
                && v["artikel"] == record["basis"]
                && (rol != "vastlegging" || v["feit"].as_str() == feit.as_deref())
        })
        .collect()
}

/// Every key of `soort` that reads an outcome or a field of the record has
/// it there (or in `extra`, the submission it answers or what it gives):
/// a key the spec binds is a key the record fills.
fn bronnen_aanwezig(soort: &Value, record: &Value, extra: &[&Value]) {
    for (naam, bron) in soort["sleutels"].as_object().unwrap() {
        let (waar, veld) = if let Some(u) = bron["uitkomst"].as_str() {
            ("outputs", u)
        } else if let Some(v) = bron["veld"].as_str() {
            ("fields", v)
        } else {
            continue;
        };
        let heeft = record[waar].get(veld).is_some()
            || extra
                .iter()
                .any(|e| e[waar].get(veld).is_some() || e.get(veld).is_some());
        assert!(
            heeft,
            "{}: the key '{naam}' reads {waar}.{veld}, which the record has not",
            soort["id"]
        );
    }
}

#[tokio::test]
async fn the_forms_carry_the_message_spec_with_the_text_layer() {
    let app = server().await;
    let (status, forms) = citizen(&app, "anna", ANNA)
        .await
        .get("/api/formulier")
        .await;
    assert_eq!(status, StatusCode::OK, "{forms}");
    let spec = &forms["berichten"];
    let ids: Vec<&str> = spec["berichtsoorten"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "beschikking",
            "ontvangstbevestiging",
            "herstelverzoek",
            "beslissing_op_bezwaar"
        ]
    );
    // Every paragraph of every template stands in a part: nothing to report.
    assert!(spec.get("meldingen").is_none(), "{}", spec["meldingen"]);
    let sjablonen = spec["tekstlaag"]["sjablonen"].as_array().unwrap();
    assert_eq!(sjablonen.len(), 4);
    // The letter element of before is gone from the forms.
    assert!(forms.get("brieven").is_none());
    // Per form what the law says of each output: a message writes an amount or a date by it.
    let besluit = forms["indieningen"][BABW]["besluit"].as_str().unwrap();
    assert!(
        forms["besluiten"][besluit]["uitleg"]
            .as_object()
            .is_some_and(|u| !u.is_empty())
    );
}

#[tokio::test]
async fn each_record_a_message_follows_from_has_one_kind_and_what_its_keys_read() {
    let app = server().await;
    let anna = citizen(&app, "anna", ANNA).await;
    let (_, forms) = anna.get("/api/formulier").await;
    let spec = forms["berichten"].clone();

    // The application: its acknowledgement reads what follows from it under its own article.
    let aanvraag = anna_applies(&app).await;
    let id = aanvraag["id"].as_str().unwrap();
    let (status, uitkomsten) = anna.get(&format!("/api/dossier/uitkomsten/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{uitkomsten}");
    let s = soort(&spec, &aanvraag, "indiening");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0]["id"], "ontvangstbevestiging");
    bronnen_aanwezig(s[0], &aanvraag, &[&json!({ "outputs": uitkomsten })]);
    for (naam, bron) in s[0]["sleutels"].as_object().unwrap() {
        if let Some(n) = bron["indiening"].as_str() {
            assert!(
                uitkomsten.get(n).is_some(),
                "'{naam}' reads '{n}', which the submission does not give: {uitkomsten}"
            );
        }
    }
    keep(&app, "ontvangstbevestiging", &aanvraag, Some(&uitkomsten)).await;

    // The municipality asks to complete it (Awb 4:5): a fact about the application.
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (status, herstel) = beslisser
        .post(
            "/api/backoffice/vastleggingen",
            json!({
                "cel": GEMEENTE,
                "feit": HERSTEL,
                "velden": {
                    "bsn_aanvrager": ANNA,
                    "aanvraag": aanvraag["id"],
                    "ontbrekende_gegevens": "Een recente pasfoto ontbreekt.",
                    "aanvullen_uiterlijk": "2026-10-27",
                },
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{herstel}");
    let s = soort(&spec, &herstel, "vastlegging");
    assert_eq!(s.len(), 1, "{herstel}");
    assert_eq!(s[0]["id"], "herstelverzoek");
    bronnen_aanwezig(s[0], &herstel, &[&aanvraag]);
    // The citizen sees it in her dossier: it names her.
    let (_, dossier) = anna.get("/api/dossier").await;
    assert!(
        dossier["grammen"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == herstel["id"])
    );
    keep(&app, "herstelverzoek", &herstel, None).await;

    // The grant.
    set_clock(&app, "2026-10-16").await;
    ggd_advises(&app, ANNA).await;
    set_clock(&app, "2026-10-20").await;
    let (status, besluit) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    let besluit = besluit["besluit"].clone();
    assert_eq!(
        besluit["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let s = soort(&spec, &besluit, "besluit");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0]["vastlegging"]["rechtskarakter"], besluit["kind"]);
    bronnen_aanwezig(s[0], &besluit, &[&aanvraag]);
    keep(&app, "toekenning", &besluit, None).await;

    // The objection, and the decision on it: a new examination finds no
    // criterion, and the grant is revoked (Awb 7:11).
    set_clock(&app, "2026-10-21").await;
    let (status, bezwaar) = anna
        .post(
            "/api/indieningen",
            json!({
                "artikel": "algemene_wet_bestuursrecht#7:11",
                "op": besluit["id"],
                "velden": common::notice_of("ik vraag een kaart als passagier"),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bezwaar}");
    set_clock(&app, "2026-11-10").await;
    let mut advies = advice(ANNA);
    advies["velden"]["datum_onderzoek"] = json!("2026-11-10");
    advies["velden"]["langdurige_loopbeperking"] = json!(false);
    let (status, _) = admin(&app)
        .await
        .post("/api/backoffice/beheer/vastleggingen", advies)
        .await;
    assert_eq!(status, StatusCode::OK);
    set_clock(&app, "2026-11-20").await;
    let mut bevindingen = findings();
    bevindingen["langdurige_loopbeperking_volgens_college"] = json!(false);
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
    let s = soort(&spec, &beslissing, "besluit");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0]["id"], "beslissing_op_bezwaar");
    assert_eq!(s[0]["vastlegging"]["rechtskarakter"], beslissing["kind"]);
    bronnen_aanwezig(s[0], &beslissing, &[&bezwaar]);
    assert_eq!(
        beslissing["outputs"]["bestreden_besluit_herroepen"],
        json!(true)
    );
    keep(&app, "beslissing-op-bezwaar", &beslissing, None).await;
}

#[tokio::test]
async fn a_refusal_has_the_same_kind_of_message_as_a_grant() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    set_clock(&app, "2026-10-16").await;
    let verzoek = common::open_request(&app, ANNA)
        .await
        .expect("an open request");
    let mut advies = advice(ANNA);
    advies["verzoek"] = verzoek["id"].clone();
    advies["velden"]["langdurige_loopbeperking"] = json!(false);
    if let Some(v) = advies["velden"].as_object_mut() {
        v.remove("bsn_aanvrager");
    }
    let (status, antwoord) = admin(&app)
        .await
        .post("/api/backoffice/beheer/vastleggingen", advies)
        .await;
    assert_eq!(status, StatusCode::OK, "{antwoord}");
    set_clock(&app, "2026-10-20").await;
    let mut bevindingen = findings();
    bevindingen["langdurige_loopbeperking_volgens_college"] = json!(false);
    let (status, besluit) = employee(&app, "gemeente-1", "regelhet-beslisser")
        .await
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": bevindingen }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    let besluit = besluit["besluit"].clone();
    assert_eq!(
        besluit["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    let (_, forms) = citizen(&app, "anna", ANNA)
        .await
        .get("/api/formulier")
        .await;
    let s = soort(&forms["berichten"], &besluit, "besluit");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0]["id"], "beschikking");
    bronnen_aanwezig(s[0], &besluit, &[&aanvraag]);
    keep(&app, "afwijzing", &besluit, None).await;
}

#[tokio::test]
async fn a_submission_gives_what_follows_from_it_only_to_its_own_citizen() {
    let app = server().await;
    let aanvraag = anna_applies(&app).await;
    let id = aanvraag["id"].as_str().unwrap();
    let (status, outcomes) = citizen(&app, "anna", ANNA)
        .await
        .get(&format!("/api/dossier/uitkomsten/{id}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{outcomes}");
    let (status, _) = citizen(&app, "bram", common::BRAM)
        .await
        .get(&format!("/api/dossier/uitkomsten/{id}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, backoffice) = employee(&app, "beslisser-1", "regelhet-beslisser")
        .await
        .get(&format!("/api/backoffice/uitkomsten/{id}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{backoffice}");
    assert_eq!(backoffice, outcomes);
}

const VERZONDEN: &str = "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#13#bericht_verzonden";
const DOORGESTUURD: &str =
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#13#bericht_doorgestuurd";

/// The hash of a frozen message, as the screen computes it: SHA-256 in hexadecimal.
fn hash(inhoud: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(inhoud.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test]
async fn a_message_sent_is_a_fact_with_its_content_frozen_and_a_forwarded_one_lands_with_the_court()
{
    let app = server().await;
    let anna = citizen(&app, "anna", ANNA).await;
    let (_, forms) = anna.get("/api/formulier").await;
    assert_eq!(forms["berichten"]["verzending"]["feit"], json!(VERZONDEN));
    assert_eq!(
        forms["berichten"]["verzending"]["doorsturen"],
        json!(DOORGESTUURD)
    );

    let aanvraag = anna_applies(&app).await;
    set_clock(&app, "2026-10-16").await;
    ggd_advises(&app, ANNA).await;
    set_clock(&app, "2026-10-20").await;
    let beslisser = employee(&app, "gemeente-1", "regelhet-beslisser").await;
    let (status, besluit) = beslisser
        .post(
            "/api/backoffice/besluiten",
            json!({ "op": aanvraag["id"], "bevindingen": findings() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{besluit}");
    let besluit = besluit["besluit"].clone();

    // Sending: a fact in the chronicle of the municipality, with the message as it went.
    let verzend = |inhoud: &str, vervangt: Value| {
        json!({
            "cel": GEMEENTE,
            "feit": VERZONDEN,
            "velden": {
                "bsn_aanvrager": ANNA,
                "berichtsoort": "beschikking",
                "vastlegging": besluit["id"],
                "kanaal": "portaal",
                "verzonden_op": "2026-10-20",
                "inhoud": inhoud,
                "hash": hash(inhoud),
                "vervangt": vervangt,
            },
        })
    };
    let eerste_inhoud =
        r#"{"soort":"beschikking","onderwerp":"U krijgt een gehandicaptenparkeerkaart"}"#;
    let (status, eerste) = beslisser
        .post(
            "/api/backoffice/vastleggingen",
            verzend(eerste_inhoud, Value::Null),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{eerste}");
    assert_eq!(eerste["recording_actor"], json!(GEMEENTE));
    // The citizen reads it from her dossier, as it went.
    let (_, dossier) = anna.get("/api/dossier").await;
    let gelezen = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == eerste["id"])
        .cloned()
        .expect("the message in the dossier");
    assert_eq!(gelezen["fields"]["inhoud"], json!(eerste_inhoud));
    assert_eq!(gelezen["fields"]["hash"], json!(hash(eerste_inhoud)));

    // A new message replaces it and names it; the first stays as it was.
    set_clock(&app, "2026-10-21").await;
    let tweede_inhoud = r#"{"soort":"beschikking","onderwerp":"U krijgt een gehandicaptenparkeerkaart (hersteld)"}"#;
    let (status, tweede) = beslisser
        .post(
            "/api/backoffice/vastleggingen",
            verzend(tweede_inhoud, eerste["id"].clone()),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{tweede}");
    let (_, dossier) = anna.get("/api/dossier").await;
    let grammen = dossier["grammen"].as_array().unwrap();
    let eerst = grammen.iter().find(|g| g["id"] == eerste["id"]).unwrap();
    assert_eq!(
        eerst["fields"]["inhoud"],
        json!(eerste_inhoud),
        "a message sent does not change"
    );
    let daarna = grammen.iter().find(|g| g["id"] == tweede["id"]).unwrap();
    assert_eq!(daarna["fields"]["vervangt"], eerste["id"]);

    // Forwarding: a fact of the municipality, and the court records the receipt in its own chronicle.
    let (status, door) = beslisser
        .post(
            "/api/backoffice/vastleggingen",
            json!({
                "cel": GEMEENTE,
                "feit": DOORGESTUURD,
                "velden": {
                    "bsn_aanvrager": ANNA,
                    "bericht": tweede["id"],
                    "berichtsoort": "beschikking",
                    "inhoud": tweede_inhoud,
                    "hash": hash(tweede_inhoud),
                },
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{door}");
    let (_, dossier) = anna.get("/api/dossier").await;
    let ontvangen = dossier["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["fact"] == json!("doorgestuurd_bericht_ontvangen"))
        .cloned()
        .unwrap_or_else(|| panic!("the court's receipt in the dossier: {dossier}"));
    assert_eq!(ontvangen["recording_actor"], json!(common::RECHTBANK));
    assert_eq!(ontvangen["fields"]["inhoud"], json!(tweede_inhoud));
    assert_eq!(ontvangen["fields"]["hash"], json!(hash(tweede_inhoud)));
}
