//! Recording an application in the chronicle: its fields, its moment and
//! what the cell refuses.

use super::*;

#[test]
fn the_shape_of_the_application_comes_from_the_law() {
    let shape = shape::derive(&service(), LAW, ARTICLE, day("2026-10-13")).unwrap();
    assert_eq!(shape.kind, "AANVRAAG");
    // Art. 13a is also the decision: recording the application runs only
    // the hooks, at the stage AANVRAAG of the Awb procedure.
    assert_eq!(shape.stage.as_deref(), Some("AANVRAAG"));
    assert!(shape.outputs.is_empty());
    // Every parameter of art. 13a and of the Awb hooks on its application,
    // but the moment, which the cell fills.
    let supplied: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    assert_eq!(
        supplied,
        vec![
            ("bsn_jongere", "leerplichtwet_1969#13a"),
            ("eerste_verlofdag", "leerplichtwet_1969#13a"),
            ("laatste_verlofdag", "leerplichtwet_1969#13a"),
            ("aanvraag_ondertekend", "algemene_wet_bestuursrecht#4:2"),
            ("naam_aanvrager", "algemene_wet_bestuursrecht#4:2"),
            ("adres_aanvrager", "algemene_wet_bestuursrecht#4:2"),
            ("dagtekening", "algemene_wet_bestuursrecht#4:2"),
            ("gevraagde_beschikking", "algemene_wet_bestuursrecht#4:2"),
        ]
    );
    let moment = shape.moment.unwrap();
    assert_eq!(moment.parameter.as_deref(), Some("datum_ontvangst"));
    assert_eq!(moment.declared_by, "algemene_wet_bestuursrecht#4:13");
}

#[test]
fn an_application_is_recorded_with_its_basis_and_the_deadline_its_hooks_give() {
    let mut cell = cell();
    let gram = record(&mut cell, &application(), "2026-10-13").unwrap();

    assert_eq!(gram.kind, "AANVRAAG");
    assert_eq!(gram.basis, "leerplichtwet_1969#13a");
    assert_eq!(gram.recording_actor, RECORDING_ACTOR);
    assert_eq!(gram.recorded_on, day("2026-10-13"));
    assert_eq!(uuid_version(&gram.id), 7);

    // What the parent supplied; the receipt is no part of it.
    assert_eq!(gram.fields["eerste_verlofdag"], json!("2027-02-01"));
    assert_eq!(gram.fields.len(), 8);
    assert!(!gram.fields.contains_key("datum_ontvangst"));

    // The receipt the cell filled in.
    let moment = gram.moment.as_ref().unwrap();
    assert_eq!(moment.parameter.as_deref(), Some("datum_ontvangst"));
    assert_eq!(moment.on, day("2026-10-13"));
    assert_eq!(moment.declared_by, "algemene_wet_bestuursrecht#4:13");

    // What the Awb hooks give is not in the chronolexogram: it follows from
    // the fields and the law of the day, and the cell computes it when
    // asked. Art. 13a follows the Awb procedure, so the article runs its
    // stage AANVRAAG: the outputs of art. 13a belong to the decision
    // (`produces`), and no hook on the decision fires yet.
    assert!(gram.outputs.is_empty());
    assert_eq!(given(&cell, &gram).len(), 2);
    assert_eq!(
        given(&cell, &gram)["aanvraag_bevat_wat_artikel_4_2_eist"],
        json!(true)
    );
    assert_eq!(
        given(&cell, &gram)["redelijke_beslistermijn_einddatum"],
        json!("2026-12-08")
    );
    // The chronolexogram does not repeat the law: what each field and output
    // rests on follows from the law in force on `recorded_on`.
    let serialized = serde_json::to_value(&gram).unwrap();
    assert!(serialized.get("field_basis").is_none());
    assert!(serialized.get("output_basis").is_none());
    assert!(serialized["moment"].get("basis").is_none());
    assert_eq!(serialized["basis"], "leerplichtwet_1969#13a");

    assert_eq!(cell.grams(), [gram]);
}

#[test]
fn an_incomplete_application_is_recorded_and_its_hook_says_what_it_lacks() {
    let mut cell = cell();
    let mut empty = application();
    empty.insert("adres_aanvrager".into(), Value::Null);
    let mut left_out = application();
    left_out.remove("adres_aanvrager");
    let grams: Vec<_> = [empty, left_out]
        .iter()
        .map(|supplied| record(&mut cell, supplied, "2026-10-13").unwrap())
        .collect();
    for gram in &grams {
        assert_eq!(
            given(&cell, gram)["aanvraag_bevat_wat_artikel_4_2_eist"],
            json!(false)
        );
        // Left out or `null`, the chronolexogram shows what the engine
        // received.
        assert_eq!(gram.fields["adres_aanvrager"], Value::Null);
    }
    assert_eq!(grams[0].fields, grams[1].fields);
}

#[test]
fn a_field_the_law_does_not_ask_is_refused() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.insert("bestemming".into(), json!("Lissabon"));
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not ask 'bestemming'")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn the_moment_that_counts_is_not_the_submitters_to_supply() {
    let mut cell = cell();
    let mut supplied = application();
    // The receipt is the moment that counts; the cell records it.
    supplied.insert("datum_ontvangst".into(), json!("2026-10-01"));
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("moment that counts")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn a_value_of_the_wrong_type_is_refused() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.insert("eerste_verlofdag".into(), json!("volgende week"));
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not fit a non-nullable Date")),
        "{e}"
    );
}

#[test]
fn null_is_refused_for_a_field_that_cannot_be_absent() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.insert("bsn_jongere".into(), Value::Null);
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("'bsn_jongere' is null")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn an_article_that_establishes_no_submission_is_refused() {
    let mut cell = cell();
    let e = cell
        .record_submission(
            &mut service(),
            &[],
            LAW,
            "2",
            None,
            &Map::new(),
            day("2026-10-13"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("establishes no submission")),
        "{e}"
    );
}

#[test]
fn a_gram_dated_before_the_last_one_is_refused() {
    let mut cell = cell();
    record(&mut cell, &application(), "2026-10-13").unwrap();
    // The same day is fine.
    record(&mut cell, &application(), "2026-10-13").unwrap();
    let e = record(&mut cell, &application(), "2026-10-12").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn a_chronicle_in_a_file_reads_back_what_was_recorded() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("aanvragen.jsonl");
    let recorded = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        assert!(cell.grams().is_empty());
        record(&mut cell, &application(), "2026-10-13").unwrap()
    };
    let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    assert_eq!(cell.grams(), [recorded]);
    // The clock of the file holds after reopening.
    let e = record(&mut cell, &application(), "2026-10-01").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );

    // Another recording actor does not open this chronicle.
    let e = Cell::open("obs_het_anker", &path, dir.path()).unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
}

#[test]
fn a_left_out_field_that_cannot_be_absent_is_named_as_missing() {
    // Awb 4:5: an incomplete application is still an application. The
    // chronolexogram records it, and says what it lacks.
    let mut cell = cell();
    let mut supplied = application();
    supplied.remove("eerste_verlofdag");
    let gram = record(&mut cell, &supplied, "2026-10-13").unwrap();
    assert_eq!(gram.missing, vec!["eerste_verlofdag"]);
    assert!(!gram.fields.contains_key("eerste_verlofdag"));

    let complete = record(&mut cell, &application(), "2026-10-13").unwrap();
    assert!(complete.missing.is_empty());
}
