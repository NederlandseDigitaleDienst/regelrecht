//! The application, the decision on it, and what the policy leaves to the
//! deciding authority.

use super::*;

#[test]
fn the_fields_of_an_application_come_from_its_article_and_the_hooks_on_it() {
    let shape = shape::derive(&service(), LAW, ARTICLE, day("2026-10-13")).unwrap();
    assert_eq!(shape.kind, "AANVRAAG");
    assert_eq!(shape.stage.as_deref(), Some("AANVRAAG"));
    assert_eq!(
        shape.requests.as_deref(),
        Some(format!("{LAW}#{ARTICLE}").as_str())
    );
    let fields: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    let babw = format!("{LAW}#{ARTICLE}");
    assert_eq!(
        fields,
        vec![
            ("bsn_aanvrager", babw.as_str()),
            ("kaart_als_bestuurder", babw.as_str()),
            ("kaart_als_passagier", babw.as_str()),
            (
                "aanvraag_ondertekend",
                "test_general_administrative_act#4:2"
            ),
            ("naam_aanvrager", "test_general_administrative_act#4:2"),
            ("adres_aanvrager", "test_general_administrative_act#4:2"),
            ("dagtekening", "test_general_administrative_act#4:2"),
            (
                "gevraagde_beschikking",
                "test_general_administrative_act#4:2"
            ),
        ]
    );
    let moment = shape.moment.unwrap();
    assert_eq!(moment.parameter.as_deref(), Some("datum_ontvangst"));
    assert_eq!(moment.declared_by, "test_general_administrative_act#4:13");
}

#[test]
fn an_application_is_recorded_with_the_deadline_its_hooks_give() {
    let mut cell = cell();
    let gram = applied(&mut cell, APPLICANT, "2026-10-13");
    assert_eq!(gram.kind, "AANVRAAG");
    assert_eq!(gram.recording_actor, RECORDING_ACTOR);
    assert_eq!(gram.fields["kaart_als_bestuurder"], json!(true));
    assert_eq!(
        given(&cell, &gram)["aanvraag_bevat_wat_artikel_4_2_eist"],
        json!(true)
    );
    assert_eq!(
        given(&cell, &gram)["redelijke_beslistermijn_einddatum"],
        json!("2026-12-08")
    );
    // The decision's outputs wait for the decision.
    assert!(
        !gram
            .outputs
            .contains_key("gehandicaptenparkeerkaart_verstrekt")
    );
}

#[test]
fn the_decision_takes_the_application_and_the_cell_and_the_policy_says_what_the_authority_finds() {
    let shape = decision::derive(&service(), LAW, ARTICLE, day("2026-10-20")).unwrap();
    assert_eq!(shape.kind, "BESCHIKKING");
    // The outcome; the last day of validity is derived, no outcome (RFC-045 §1).
    assert_eq!(
        shape.outputs,
        [
            "gehandicaptenparkeerkaart_verstrekt",
            "kaart_vermeldt_bestuurder",
            "kaart_vermeldt_passagier"
        ]
    );
    let asked: Vec<(&str, &Source)> = shape
        .inputs
        .iter()
        .map(|i| (i.field.name.as_str(), &i.source))
        .collect();
    assert_eq!(
        asked,
        vec![
            ("bsn_aanvrager", &Source::Application),
            ("kaart_als_bestuurder", &Source::Application),
            ("kaart_als_passagier", &Source::Application),
            // The day of the decision (a parameter of art. 49 that the Awb
            // stage also needs) and the day it is made known, both from the
            // cell.
            ("besluit_datum", &Source::Cell),
            // The date of the application (Awb 4:2): an examination counts
            // for it from that day (art. 2 of the Regeling, own choice).
            ("dagtekening", &Source::Application),
            ("bekendmaking_datum", &Source::Cell),
        ]
    );
    // The four criteria and the college's power: the municipality's policy
    // fills them, so no finding takes their place (RFC-048 §2).
    let college = cell();
    let by = college.authority();
    for name in [WALKING, HELP, WHEELCHAIR, OTHER, POWER] {
        assert!(shape.filled(name), "{name}");
        assert!(shape.finding(name, by).is_none(), "{name}");
    }
    // The findings: art. 2 lid 2 of the Regeling (whether to leave the
    // examination out, and the judgements under a and b), and where the
    // policy leaves a case to the college itself: the criteria without an
    // advice (policy art. 6, lid 2) and a valid card without an answer of
    // the cell (policy art. 2).
    let mut findings: Vec<&str> = shape
        .terms
        .iter()
        .filter(|t| t.is_finding(by))
        .map(|t| t.id.as_str())
        .collect();
    findings.sort_unstable();
    let mut expected = vec![
        LEFT_OUT,
        KNOWN,
        JUDGED,
        "langdurige_loopbeperking_volgens_college",
        "continu_afhankelijk_van_hulp_bestuurder_volgens_college",
        "permanent_rolstoelgebonden_volgens_college",
        "ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college",
        "aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college",
    ];
    expected.sort_unstable();
    assert_eq!(findings, expected);
    // Each is the college's: the GGD answers none of them (RFC-002), so a
    // finding of hers on them would be refused.
    let ggd = ggd();
    assert!(shape.terms.iter().all(|t| !t.is_finding(ggd.authority())));
    // The college may leave the three of lid 2 out: then it does not leave
    // the examination out.
    for name in [LEFT_OUT, KNOWN, JUDGED] {
        let term = shape.finding(name, by).unwrap();
        assert!(
            regelrecht_cel::decision::finding_field(term).nullable,
            "{name}"
        );
    }
    // Whether the examination took place is no input of art. 49: the
    // Regeling asks it, and in the cell the reduction answers it there.
    assert!(shape.input(EXAMINED).is_none());
    // Whether the applicant holds a valid card is no input of art. 49: the
    // policy asks it, and in the cell the reduction answers it there.
    assert!(shape.input(VALID).is_none());
}

#[test]
fn a_grant_records_its_outcome_its_derived_data_and_the_hooks_on_it() {
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");

    assert_eq!(gram.kind, "BESCHIKKING");
    assert_eq!(gram.refers_to.as_deref(), Some(app.id.as_str()));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(gram.outputs["kaart_vermeldt_bestuurder"], json!(true));
    assert_eq!(gram.outputs["kaart_vermeldt_passagier"], json!(false));
    // Art. 51 lid 1: five years from the day of issue, which is the day of
    // the decision (own choice). Derived, and recorded with the decision.
    assert_eq!(gram.outputs["kaart_geldig_tot"], json!("2031-10-19"));
    // The Awb on the decision.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(gram.outputs["termijn_einddatum"], json!("2026-12-01"));
    // What the policy filled for art. 49, and the college's findings.
    assert_eq!(gram.fields[POWER], json!(true));
    assert_eq!(gram.fields[WALKING], json!(true));
    // The cell's answer to the policy's term is no field of the decision;
    // a replay reports it (see the trace tests below).
    assert!(!gram.fields.contains_key(VALID));
}

#[test]
fn a_refusal_is_a_decision_with_the_hooks_on_it() {
    let mut cell = cell();
    let gram = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    assert_eq!(gram.outputs["kaart_geldig_tot"], Value::Null);
    // A refusal is a decision too: motivated and open to objection.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(gram.outputs["termijn_weken"], json!(6));
}

#[test]
fn a_term_the_policy_leaves_to_the_authority_needs_its_finding() {
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut partial = examined(true);
    partial.remove(OTHER_C);
    // Without an advice the policy is silent, and the engine misses the
    // college's judgement (policy art. 6 lid 2).
    let e = decide_with(&mut cell, &app.id, &partial, "2026-10-20").unwrap_err();
    assert!(
        e.to_string()
            .contains(&format!("Required open term '{OTHER_C}'"))
            && e.to_string().contains("no judgement was given"),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn a_finding_on_a_term_the_policy_binds_to_a_cell_is_asked_by_none() {
    // The college finds that the applicant holds a valid card. Its policy
    // fills that term with the cell's answer, so no finding of the college
    // takes its place (RFC-048 §2); without one the cell's answer counts:
    // there is no card in its chronicle, and the card is granted.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    let mut given = examined(true);
    given.insert(VALID.into(), json!(true));
    let e = decide_with(&mut cell, &app.id, &given, "2026-10-20").unwrap_err();
    assert!(e.to_string().contains("is asked by none"), "{e}");
    let gram = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let found = cell.findings(&mut service(), &[&ggd()], &gram.id).unwrap();
    assert!(!found.contains(&VALID_C.to_string()), "{found:?}");
    assert!(found.contains(&WALKING_C.to_string()), "{found:?}");
}
