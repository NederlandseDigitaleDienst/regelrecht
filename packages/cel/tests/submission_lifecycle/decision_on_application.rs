//! The decision on an application: what it takes from the application, the
//! cell and the policy, and what the deciding authority finds.

use super::*;

#[test]
fn the_decision_takes_the_application_and_the_cell_and_the_policy_says_what_the_authority_finds() {
    let shape = decision::derive(&service(), LAW, ARTICLE, day("2026-10-20")).unwrap();
    assert_eq!(shape.kind, "BESCHIKKING");
    assert_eq!(shape.submission_kind.as_deref(), Some("AANVRAAG"));
    let asked: Vec<(&str, &Source)> = shape
        .inputs
        .iter()
        .map(|i| (i.field.name.as_str(), &i.source))
        .collect();
    assert_eq!(
        asked,
        vec![
            ("bsn_jongere", &Source::Application),
            ("eerste_verlofdag", &Source::Application),
            ("laatste_verlofdag", &Source::Application),
            ("besluit_datum", &Source::Cell),
            ("bekendmaking_datum", &Source::Cell),
        ]
    );
    // The school's policy fills the first school day, and binds two
    // findings of the head (art. 11 onder f, lid 2) to an answer of his
    // cell (RFC-048): it fills them, so no finding takes their place.
    for name in ["eerste_lesdag_schooljaar", OUTSIDE_HOLIDAYS, ONCE] {
        assert!(shape.filled(name), "{name}");
    }
    // Where the cell gives no answer, the policy lets the head judge, with
    // an open term of its own (policy art. 2 and 3).
    let head = cell();
    let findings: Vec<&str> = shape
        .terms
        .iter()
        .filter(|t| t.is_finding(head.authority()))
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(
        findings,
        [
            PROFESSION,
            "verlof_valt_buiten_schoolvakanties_volgens_hoofd",
            "verlof_al_verleend_in_schooljaar_volgens_hoofd",
        ]
    );
}

#[test]
fn a_decision_records_its_outcome_and_the_values_it_used_and_refers_to_its_application() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();

    assert_eq!(gram.kind, "BESCHIKKING");
    assert_eq!(gram.basis, "leerplichtwet_1969#13a");
    assert_eq!(gram.refers_to.as_deref(), Some(app.id.as_str()));
    assert_eq!(gram.recorded_on, day("2026-10-20"));
    assert_eq!(uuid_version(&gram.id), 7);

    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    assert_eq!(gram.outputs["verlofdagen"], json!(5));
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
    // The chronolexogram does not repeat the law: the article is `basis`, and
    // what rests on which provision follows from the law on `recorded_on`.
    let json = serde_json::to_value(&gram).unwrap();
    assert!(json.get("field_basis").is_none() && json.get("output_basis").is_none());
    // Nor where each value came from: that follows from the law and
    // `refers_to`, so there is no `field_source`.
    assert!(json.get("field_source").is_none());

    // The values the decision used.
    assert_eq!(gram.fields["eerste_verlofdag"], json!("2027-02-01"));
    assert_eq!(gram.fields["eerste_lesdag_schooljaar"], json!("2026-08-31"));
    for judged in [PROFESSION, OUTSIDE_HOLIDAYS] {
        assert_eq!(gram.fields[judged], json!(true), "{judged}");
    }
    assert_eq!(gram.fields["besluit_datum"], json!("2026-10-20"));
    // What the application holds and the decision does not ask stays in
    // the application gram.
    assert!(!gram.fields.contains_key("naam_aanvrager"));
}

#[test]
fn the_hooks_on_the_decision_land_in_its_gram_and_not_in_the_application() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();

    // Which Awb articles take part at which stage follows from the law.
    let shape = decision::derive(&service(), LAW, ARTICLE, day("2026-10-20")).unwrap();
    let taking_part = |stage: usize| -> Vec<String> {
        shape.stages[stage]
            .articles
            .iter()
            .map(|a| format!("{}#{}", a.law_id, a.article_number))
            .collect()
    };
    let awb = |article: &str| format!("{AWB}#{article}");
    for (output, value, article) in [
        ("motivering_vereist", json!(true), "3:46"),
        ("termijn_weken", json!(6), "6:7"),
        // Made known on the day of the decision (assumption of the cell).
        ("termijn_startdatum", json!("2026-10-21"), "6:8"),
        ("termijn_einddatum", json!("2026-12-01"), "6:8"),
    ] {
        assert_eq!(gram.outputs[output], value, "{output}");
        let stage = usize::from(article == "6:8");
        assert!(taking_part(stage).contains(&awb(article)), "{output}");
        assert!(!app.outputs.contains_key(output), "{output}");
    }
    // And the hooks on the application do not fire again.
    assert!(
        !gram
            .outputs
            .contains_key("redelijke_beslistermijn_einddatum")
    );
    assert!(
        !gram
            .outputs
            .contains_key("aanvraag_bevat_wat_artikel_4_2_eist")
    );
    assert_eq!(cell.grams(), [app, gram]);
}

#[test]
fn a_decision_needs_every_finding_the_engine_asks() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    // The law asks the head for the profession itself; whether the leave
    // falls outside the holidays the policy binds to an answer this cell
    // does not give, and then leaves to the head (policy art. 3): the
    // engine says that judgement is missing.
    for (judged, on, other) in [
        (
            PROFESSION,
            format!("{LAW}#{ARTICLE}"),
            OUTSIDE_HOLIDAYS_HEAD,
        ),
        (
            OUTSIDE_HOLIDAYS_HEAD,
            format!("{LEAVE_POLICY}#3"),
            PROFESSION,
        ),
    ] {
        let mut supplied = judgement(true, true);
        supplied.remove(judged);
        let e = decide(&mut cell, &app.id, &supplied, "2026-10-20").unwrap_err();
        let says = e.to_string();
        assert!(
            (says.contains(&format!("asks '{judged}'"))
                || says.contains(&format!("'{judged}' on {on}"))
                    && says.contains("no judgement was given"))
                && !says.contains(&format!("'{other}'")),
            "{e}"
        );
    }
    let mut wrong = judgement(true, true);
    wrong.insert(PROFESSION.into(), json!("ja"));
    let e = decide(&mut cell, &app.id, &wrong, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not fit a Boolean")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn the_deciding_authority_does_not_supply_what_comes_from_elsewhere() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    for (name, value, from) in [
        ("eerste_verlofdag", json!("2027-02-02"), "the application"),
        (
            "eerste_lesdag_schooljaar",
            json!("2026-09-07"),
            "the policy",
        ),
        ("besluit_datum", json!("2026-10-01"), "the cell"),
    ] {
        let mut supplied = judgement(true, true);
        supplied.insert(name.into(), value);
        let e = decide(&mut cell, &app.id, &supplied, "2026-10-20").unwrap_err();
        assert!(
            matches!(&e, Error::Refused(m) if m.contains(&format!("'{name}'")) && m.contains(from)),
            "{e}"
        );
    }
    let mut supplied = judgement(true, true);
    supplied.insert("bestemming".into(), json!("Lissabon"));
    let e = decide(&mut cell, &app.id, &supplied, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not ask 'bestemming'")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn a_decision_refers_to_an_application_of_this_cell_that_is_not_decided_yet() {
    let mut cell = cell();
    let e = decide(
        &mut cell,
        "019a0000-0000-7000-8000-000000000000",
        &judgement(true, true),
        "2026-10-20",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("no chronolexogram")),
        "{e}"
    );

    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let decided = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    let e = decide(&mut cell, &app.id, &judgement(false, true), "2026-10-21").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("already decided on")),
        "{e}"
    );
    // A decision is no application to decide on.
    let e = decide(&mut cell, &decided.id, &judgement(true, true), "2026-10-21").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("is a BESCHIKKING")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn a_decision_is_not_dated_before_its_application() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-12").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );
    // The same day is fine.
    decide(&mut cell, &app.id, &judgement(true, true), "2026-10-13").unwrap();
}

#[test]
fn a_decision_reads_back_from_the_file() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("kroniek.jsonl");
    let recorded = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
        let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
        vec![app, gram]
    };
    let cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    assert_eq!(cell.grams(), recorded);
}

#[test]
fn an_application_that_lacks_what_the_decision_needs_is_not_decided_on() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.remove("laatste_verlofdag");
    let app = record(&mut cell, &supplied, "2026-10-13").unwrap();
    assert_eq!(app.missing, vec!["laatste_verlofdag"]);
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("lacks 'laatste_verlofdag'")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}
