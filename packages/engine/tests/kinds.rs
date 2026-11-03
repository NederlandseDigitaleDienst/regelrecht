//! Kinds that a regulation defines (schema v0.9.0), over the real corpus.
//!
//! One mechanism for two general laws. The Awb defines the besluit, the
//! beschikking, the aanvraag, the bezwaar and the beroep (articles 1:3, 1:5
//! and 8:70) and intervenes in them with hooks and a procedure. The Awir
//! defines the inkomensafhankelijke regeling and the tegemoetkoming (articles
//! 1 lid 3 and 2 lid 1 onder h) and intervenes in the tegemoetkoming with
//! articles 14 and 15. The Leerplichtwet gives a beschikking; the Wet op de
//! zorgtoeslag gives a tegemoetkoming, which the Awir makes into a
//! beschikking on an aanvraag. The schema names none of these.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::BTreeMap;
use walkdir::WalkDir;

const AWB: &str = "algemene_wet_bestuursrecht";
const AWIR: &str = "algemene_wet_inkomensafhankelijke_regelingen";
const WZT: &str = "wet_op_de_zorgtoeslag";
const LPW: &str = "leerplichtwet_1969";

/// The whole corpus, or `None` when its Awb defines no kinds (the demo
/// corpus, which is still on an earlier schema).
fn corpus() -> Option<LawExecutionService> {
    let mut service = LawExecutionService::new();
    let dir = common::regulation_base_path().join("nl");
    for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            if let Ok(content) = std::fs::read_to_string(path) {
                let _ = service.load_law(&content);
            }
        }
    }
    let defines_kinds = service
        .resolver()
        .kind(&format!("{AWB}/beschikking"))
        .is_some();
    if !defines_kinds {
        eprintln!("skipped: the Awb of this corpus defines no kinds");
    }
    defines_kinds.then_some(service)
}

fn hooks_at(service: &LawExecutionService, law: &str, article: &str, stage: &str) -> Vec<String> {
    service
        .procedure_stage(law, article, stage, "2026-09-01")
        .unwrap()
        .articles
        .iter()
        .skip(1)
        .map(|a| format!("{}#{}", a.law_id, a.article_number))
        .collect()
}

#[test]
fn every_kind_in_the_corpus_is_defined_and_used_as_defined() {
    let Some(service) = corpus() else { return };
    assert_eq!(service.resolver().validate_kinds(), Vec::<String>::new());
}

#[test]
fn a_kind_names_its_wider_kinds() {
    let Some(service) = corpus() else { return };
    assert_eq!(
        service
            .resolver()
            .kind_lineage(&format!("{AWB}/beschikking")),
        vec![format!("{AWB}/beschikking"), format!("{AWB}/besluit")]
    );
}

/// The Leerplichtwet gives a beschikking on an aanvraag. The Awb finds both
/// through the kinds of Awb 1:3: its procedure, the hooks on the aanvraag at
/// the stage that receives one, and the hooks on the besluit when it is
/// taken. No Awir article takes part.
#[test]
fn the_awb_intervenes_in_a_beschikking_of_the_leerplichtwet() {
    let Some(service) = corpus() else { return };
    let submission = service
        .submission(LPW, "13a", "2026-09-01")
        .unwrap()
        .unwrap();
    assert_eq!(submission.kind, format!("{AWB}/aanvraag"));
    assert_eq!(submission.stage.as_deref(), Some("AANVRAAG"));

    let at_application = hooks_at(&service, LPW, "13a", "AANVRAAG");
    assert!(
        at_application.contains(&format!("{AWB}#4:2")),
        "{at_application:?}"
    );

    let at_decision = hooks_at(&service, LPW, "13a", "BESLUIT");
    assert!(
        at_decision.contains(&format!("{AWB}#3:46")),
        "{at_decision:?}"
    );
    assert!(
        at_decision.contains(&format!("{AWB}#6:7")),
        "{at_decision:?}"
    );
    assert!(
        !at_decision
            .iter()
            .chain(&at_application)
            .any(|h| h.starts_with(AWIR)),
        "an Awir hook fired on a beschikking that is no tegemoetkoming"
    );
}

/// An aanvraag on Awir 15 names the claim it asks for, Wzt 2, and Awir 14
/// decides. The Wzt has no decision article of its own: the aanvraag, the
/// decision and the Awb hooks all sit on the Awir side of the reference.
#[test]
fn an_aanvraag_on_the_awir_refers_to_the_claim_in_the_wzt() {
    let Some(service) = corpus() else { return };
    let submission = service
        .submission(AWIR, "15", "2025-06-01")
        .unwrap()
        .unwrap();
    assert_eq!(submission.kind, format!("{AWB}/aanvraag"));
    assert_eq!(
        service
            .requested_decision(AWIR, "15", "2025-06-01")
            .unwrap(),
        Some(format!("{AWIR}#14"))
    );
    assert_eq!(
        service.requested_decision(WZT, "2", "2025-06-01").unwrap(),
        None
    );

    let at_decision = hooks_at(&service, AWIR, "14", "BESLUIT");
    assert!(
        at_decision.contains(&format!("{AWB}#3:46")),
        "{at_decision:?}"
    );
    assert!(
        at_decision.contains(&format!("{AWB}#6:7")),
        "{at_decision:?}"
    );

    // Awir 14 runs on the reference; the Awb reaches the decision through
    // the kind beschikking, and the amount comes from Wzt 2.
    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));
    params.insert(
        "tegemoetkoming".to_string(),
        Value::String(format!("{WZT}#2")),
    );
    let granted = service
        .evaluate_law_output(AWIR, "toegekende_tegemoetkoming", params, "2025-01-01")
        .unwrap();
    assert_eq!(granted.outputs.get("termijn_weken"), Some(&Value::Int(6)));
    assert_eq!(
        granted.outputs.get("motivering_vereist"),
        Some(&Value::Bool(true))
    );

    // A reference to an article of another kind is refused.
    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));
    params.insert(
        "tegemoetkoming".to_string(),
        Value::String("vreemdelingenwet_2000#14".to_string()),
    );
    let refused = service
        .evaluate_law_output(AWIR, "toegekende_tegemoetkoming", params, "2025-01-01")
        .unwrap_err();
    assert!(
        refused.to_string().contains("produces nothing of kind"),
        "{refused}"
    );
}

/// A regulation that claims a kind of regulation quotes its own words; the
/// check holds them against its text.
#[test]
fn the_wzt_claims_to_be_an_inkomensafhankelijke_regeling_in_its_own_words() {
    let Some(service) = corpus() else { return };
    let law = service.resolver().get_law(WZT).unwrap();
    let claim = &law.member_of.as_ref().unwrap()[0];
    assert_eq!(claim.kind, format!("{AWIR}/inkomensafhankelijke_regeling"));
    assert!(claim
        .legal_text_excerpt
        .as_deref()
        .unwrap()
        .contains("afhankelijk van de draagkracht"));
}
