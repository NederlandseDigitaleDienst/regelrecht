//! The answer of the cell's own reduction over its decisions, and where the
//! deciding authority finds the term instead.

use super::*;

#[test]
fn without_an_earlier_grant_in_the_period_the_answer_is_false() {
    let mut cell = cell();
    let gram = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    // The reduction's answer, kept with the decision like an application
    // field.
    assert_eq!(gram.fields[ONCE], json!(false));
}

#[test]
fn an_earlier_grant_in_the_period_makes_the_answer_true() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "2026-10-05", "2026-10-09", "2026-09-21");
    // Five days in May: on their own they would be granted.
    let gram = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(gram.fields[ONCE], json!(true));
    assert_eq!(gram.outputs["verlofdagen"], json!(5));
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
}

#[test]
fn a_grant_in_another_period_does_not_count() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    // Leave in October 2027 is in the school year 2027-2028, whose first
    // lesdag comes from next year's policy.
    let gram = apply_and_decide(&mut cell, "2027-10-04", "2027-10-08", "2027-09-20");
    assert_eq!(gram.fields["eerste_lesdag_schooljaar"], json!("2027-08-30"));
    assert_eq!(gram.fields[ONCE], json!(false));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn the_period_comes_from_the_policy_the_reduction_calls() {
    let mut cell = cell();
    // Granted leave at the end of the school year 2026-2027.
    apply_and_decide(&mut cell, "2027-07-26", "2027-07-30", "2027-06-01");
    // Leave starting in July is in the same school year.
    let july = apply_and_decide(&mut cell, "2027-07-19", "2027-07-23", "2027-06-02");
    assert_eq!(july.fields[ONCE], json!(true));
    assert_eq!(july.outputs["verlof_verleend"], json!(false));
    // Leave starting on 2 August is in the next one. Decided in that school
    // year, the first lesdag comes from next year's policy too.
    let august = apply_and_decide(&mut cell, "2027-08-02", "2027-08-06", "2027-08-02");
    assert_eq!(
        august.fields["eerste_lesdag_schooljaar"],
        json!("2027-08-30")
    );
    assert_eq!(august.fields[ONCE], json!(false));
    assert_eq!(august.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_refusal_does_not_count() {
    let mut cell = cell();
    // Eleven school days: refused. The reduction leaves it out of the
    // granted leave.
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-15", "2026-10-20");
    assert_eq!(refused.outputs["verlof_verleend"], json!(false));
    let gram = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-27");
    assert_eq!(gram.fields[ONCE], json!(false));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_grant_to_another_subject_does_not_count() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let mut supplied = application();
    supplied.insert("bsn_jongere".into(), json!("999990020"));
    let app = record(&mut cell, &supplied, "2026-10-21").unwrap();
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-21").unwrap();
    assert_eq!(gram.fields[ONCE], json!(false));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_grant_on_the_last_day_of_the_period_counts_in_that_period() {
    let mut cell = cell();
    // Granted leave whose first day is Saturday 31 July 2027.
    let last = apply_and_decide(&mut cell, "2027-07-31", "2027-08-06", "2027-06-01");
    assert_eq!(last.outputs["verlof_verleend"], json!(true));
    // So the school year 2026-2027 has its leave.
    let july = apply_and_decide(&mut cell, "2027-07-12", "2027-07-16", "2027-06-02");
    assert_eq!(july.fields[ONCE], json!(true));
    assert_eq!(july.outputs["verlof_verleend"], json!(false));
    // Leave whose first day is Sunday 1 August 2027 is in the next one.
    let first = apply_and_decide(&mut cell, "2027-08-01", "2027-08-06", "2027-06-03");
    assert_eq!(first.fields[ONCE], json!(false));
    assert_eq!(first.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_finding_on_the_term_the_policy_binds_to_the_cell_is_refused() {
    // The policy fills the term with the cell's answer: no finding of the
    // head takes its place (RFC-048 §2).
    let mut cell = cell();
    apply_and_decide(&mut cell, "2026-10-05", "2026-10-09", "2026-09-21");
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let mut given = judgement(true, true);
    given.insert(ONCE.into(), json!(false));
    let e = decide(&mut cell, &app.id, &given, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("is filled by the policy")),
        "{e}"
    );
    // Leave was granted already this school year: the cell says so.
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.fields[ONCE], json!(true));
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
}

#[test]
fn the_decisions_the_reduction_reads_are_bound_for_the_decision_only() {
    let mut service = service();
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    cell.decide(
        &mut service,
        &[],
        LAW,
        ARTICLE,
        &app.id,
        &Map::new(),
        &judgement(true, true),
        day("2026-10-20"),
    )
    .unwrap();
    assert!(service.data_registry().list_sources().is_empty());
    // The reduction is a loaded rulework, as before the decision.
    assert!(service.has_law(REDUCTION));
}

/// The cell gives its reduction the decisions itself, so a data source in
/// the service that answers the same input does not reach it.
#[test]
fn a_source_for_the_decisions_does_not_reach_the_cells_reduction() {
    let mut service = service();
    service.add_dict_source(
        "elders",
        10,
        [(
            "999990019".to_string(),
            [("besluiten".to_string(), regelrecht_engine::Value::Null)].into(),
        )]
        .into(),
    );
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let decided = cell
        .decide(
            &mut service,
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &judgement(true, true),
            day("2026-10-20"),
        )
        .unwrap();
    assert_eq!(decided[0].fields[ONCE], json!(false));
    assert_eq!(decided[0].outputs["verlof_verleend"], json!(true));
    assert_eq!(service.data_registry().list_sources(), ["elders"]);
}

#[test]
fn without_the_answer_the_deciding_authority_finds_it() {
    // A cell that gives no answer: the binding of the head's policy gets
    // none, so it fills nothing, and the head finds it per case
    // (decided_per_case_by).
    let mut cell = without_reductions();
    let app = applied(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    let mut supplied = judgement(true, true);
    supplied.insert(ONCE_HEAD.into(), json!(true));
    let gram = decide(&mut cell, &app.id, &supplied, "2027-03-01").unwrap();
    assert_eq!(gram.fields[ONCE], json!(true));
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
}

#[test]
fn without_the_answer_or_a_finding_it_is_not_known() {
    let mut cell = without_reductions();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap_err();
    assert!(
        e.to_string()
            .contains(&format!("'{ONCE_HEAD}' on {LEAVE_POLICY}#2"))
            && e.to_string().contains("no judgement was given"),
        "{e}"
    );
}

#[test]
fn a_decision_from_before_the_reduction_takes_the_finding_and_replays() {
    // The reduction comes into force on 2027-08-01: before that the cell
    // gives no answer, and the head finds the term.
    let dir = lid_2_dir();
    let path = dir.path().join("kroniek.jsonl");
    let mut cell = Cell::open(RECORDING_ACTOR, &path, dir.path()).unwrap();
    let app = applied(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    let mut supplied = judgement(true, true);
    supplied.insert(ONCE_HEAD.into(), json!(false));
    let gram = cell
        .decide(
            &mut service_with_reduction_from("2027-08-01"),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &supplied,
            day("2027-03-01"),
        )
        .unwrap()
        .remove(0);
    assert_eq!(gram.fields[ONCE], json!(false));
    let mut later = service_with_reduction_from("2027-08-01");
    cell.explain(&mut later, &[], &gram.id).unwrap();
    let found = cell.findings(&mut later, &[], &gram.id).unwrap();
    assert!(found.contains(&ONCE_HEAD.to_string()), "{found:?}");
}

#[test]
fn an_answer_whose_reduction_the_service_does_not_hold_is_refused() {
    // Never silently the head's: the cell gives this answer, and cannot.
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let mut supplied = judgement(true, true);
    supplied.insert(ONCE_HEAD.into(), json!(false));
    let e = cell
        .decide(
            &mut service_without(&[REDUCTION]),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &supplied,
            day("2026-10-20"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("which is not loaded"), "{e}");
    assert_eq!(cell.grams().len(), 1);
}

/// The head's policy asks the cell of the head (RFC-048): a cell of another
/// school with the same reduction is not asked, and the head finds the term.
#[test]
fn the_policy_does_not_ask_a_cell_of_another_authority() {
    let dir = lid_2_dir();
    common::write_identity(
        dir.path(),
        &format!(
            "name: Het hoofd van OBS Het Anker\ncategory: het hoofd\nanswers:\n  {ONCE}: {REDUCTION}\n"
        ),
    );
    let mut cell = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    // The binding asks the cell of the head of De Klimop; this one is not
    // asked, so the policy's own term is reached. That term is the finding
    // of the policy's holder, not of this head: it gives none.
    let profession = object_map(json!({ PROFESSION: true }));
    let e = decide(&mut cell, &app.id, &profession, "2026-10-20").unwrap_err();
    assert!(e.to_string().contains("no judgement was given"), "{e}");
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap_err();
    assert!(
        e.to_string()
            .contains("answered by 'Het hoofd van OBS De Klimop'"),
        "{e}"
    );
}
