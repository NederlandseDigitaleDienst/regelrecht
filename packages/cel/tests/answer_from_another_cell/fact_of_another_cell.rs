//! A fact another cell records, read through its reduction: the decision
//! waits for it, the competent cell answers it, and a finding may leave it out.

use super::*;

#[test]
fn without_the_fact_of_the_other_cell_the_decision_waits() {
    // Art. 2 lid 1: "wordt niet afgegeven alvorens een geneeskundig onderzoek
    // heeft plaatsgehad". Not a ground for refusal: the decision waits for
    // the report, and the cell records nothing.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);
    assert_eq!(cell.grams().len(), 1);

    // Once the GGD has recorded the examination, the card is issued; the
    // GGD's cell answered the term of art. 3 (RFC-048).
    let report = reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    assert_eq!(report.kind, "EXECUTOGRAM");
    assert_eq!(report.basis, format!("{REGELING}#3"));
    assert_eq!(
        report.fact.as_deref(),
        Some("geneeskundig_onderzoek_verricht")
    );
    let gram = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let result = cell.explain(&mut service(), &[&ggd()], &gram.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let node = answered(root)
        .into_iter()
        .find(|n| n.name == EXAMINED)
        .unwrap_or_else(|| panic!("{}", root.render_box_drawing()));
    assert_eq!(
        node.result,
        Some(regelrecht_engine::Value::from(json!(true)))
    );
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, GGD_ACTOR);
    assert_eq!(answer.law_id, REPORTS);
    assert_eq!(answer.last_record.as_deref(), Some(report.id.as_str()));
    // The municipality did not answer it, so it has nothing to explain;
    // the GGD's reasoning is its own.
    let e = cell
        .explain_answer(&mut service(), &[&ggd()], &gram.id, EXAMINED)
        .unwrap_err();
    assert!(e.to_string().contains("did not ask the cell"), "{e}");
}

#[test]
fn a_decision_that_does_not_read_the_fact_does_not_wait_for_it() {
    // Art. 2 lid 1 is about issuing a card. Art. 49 asks it only for a card
    // it would issue, so a refusal on the policy needs no report.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2027-03-01");
    let second = decide_with(&mut cell, &app.id, &examined(true), "2027-03-01").unwrap();
    assert_eq!(second.fields[POWER], json!(false));
    assert_eq!(
        second.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn only_a_fact_on_the_subject_within_the_period_of_the_application_counts() {
    // Own choice: from the date of the application (Awb 4:2) up to the day
    // of issue. Not before: lid 2 onder a en b would mean nothing if an
    // examination for an earlier card still counted.
    let mut cell = cell();
    reported(&mut cell, APPLICANT, "2026-10-01", "2026-10-02");
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, "999990032", "2026-10-16", "2026-10-16");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);

    // An examination after the day of issue does not count either. The cell
    // records the report on the day it receives it, and the examination
    // date in it is the GGD's; it does not check that the examination came
    // before the report, so this constructed report tests the upper bound.
    reported(&mut cell, APPLICANT, "2026-10-25", "2026-10-19");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);

    // On the date of the application, it counts.
    reported(&mut cell, APPLICANT, "2026-10-13", "2026-10-19");
    let gram = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn the_cell_refuses_an_executogram_that_is_not_whole() {
    let partial =
        object_map(json!({ "bsn_aanvrager": APPLICANT, "datum_onderzoek": "2026-10-16" }));
    GGD.with(|ggd| {
        let mut ggd = ggd.borrow_mut();
        let e = ggd
            .record_executogram(&service(), EXAMINATION, &partial, day("2026-10-19"))
            .unwrap_err();
        assert!(e.to_string().contains("verricht_door"), "{e}");
        assert!(ggd.grams().is_empty());
    });
}

#[test]
fn only_the_authority_the_fact_names_records_it() {
    // Regeling art. 3 lid 1: the GGD or the designated expert. The
    // municipality receives the report; it does not record the examination.
    let mut cell = cell();
    let whole = object_map(json!({
        "bsn_aanvrager": APPLICANT,
        "datum_onderzoek": "2026-10-16",
        "verricht_door": "GGD Wilgendaal",
    }));
    let e = cell
        .record_executogram(&service(), EXAMINATION, &whole, day("2026-10-19"))
        .unwrap_err();
    assert!(
        e.to_string().contains("is established by 'Gemeentelijke Gezondheidsdienst' or 'vanwege het gemeentelijk gezag aangewezen deskundige'"),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn a_finding_leaves_the_fact_out_on_an_earlier_grant_the_cell_answers() {
    // The card of 2026 has expired. The college knows the applicant still
    // meets the criteria and leaves the examination out: no report needed.
    // That the applicant had a card before, the cell answers from its own
    // decisions.
    let mut cell = cell();
    let first = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2031-10-20");
    let gram = decide_with(&mut cell, &app.id, &lid_2(true, true, false), "2031-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(gram.fields[LEFT_OUT], json!(true));
    assert_eq!(gram.fields[KNOWN], json!(true));

    let result = cell.explain(&mut service(), &[&ggd()], &gram.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let names: Vec<&str> = answered(root).iter().map(|n| n.name.as_str()).collect();
    // The examination is not asked: lid 2 decides first.
    assert_eq!(names, [VALID, EARLIER], "{}", root.render_box_drawing());
    let why = cell
        .explain_answer(&mut service(), &[&ggd()], &gram.id, EARLIER)
        .unwrap();
    let why = why[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&first.id), "{why}");
}

#[test]
fn a_finding_leaves_the_fact_out_on_its_other_ground() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2031-10-20");
    let gram = decide_with(&mut cell, &app.id, &lid_2(true, false, true), "2031-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn leaving_the_fact_out_without_its_ground_still_waits_for_it() {
    // Neither a nor b: the college may not leave it out, so the card waits
    // for a report.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2031-10-20");
    let e = decide_with(&mut cell, &app.id, &lid_2(true, false, false), "2031-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);
    // The college that knows but does not leave it out, waits too.
    let e = decide_with(&mut cell, &app.id, &lid_2(false, true, true), "2031-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);
}

#[test]
fn without_an_earlier_grant_in_the_chronicle_the_cell_is_silent() {
    // A card from another municipality counts too, so the cell does not say
    // there was none: the college would have to find it, and without that
    // finding the decision fails. A refused or revoked card is no card
    // issued.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2027-03-01");
    let e = decide_with(&mut cell, &app.id, &lid_2(true, true, false), "2027-03-01").unwrap_err();
    blocked_on(&e, EARLIER_C);
}

#[test]
fn where_the_other_cell_is_silent_the_authority_finds_the_fact() {
    // The term is the college's to decide per case: the cell is silent, so
    // the college's own finding counts. Finding that no examination took
    // place (the applicant did not appear, say) refuses the card: own
    // choice, a decision of the college and no consequence of lid 1.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut given = examined(true);
    given.insert(EXAMINED_C.into(), json!(false));
    let refused = decide_with(&mut cell, &app.id, &given, "2026-10-20").unwrap();
    assert_eq!(
        refused.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    let app = applied(&mut cell, "999990032", "2026-10-21");
    given.insert(EXAMINED_C.into(), json!(true));
    let granted = decide_with(&mut cell, &app.id, &given, "2026-10-21").unwrap();
    assert_eq!(
        granted.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let found = cell
        .findings(&mut service(), &[&ggd()], &granted.id)
        .unwrap();
    assert!(found.contains(&EXAMINED_C.to_string()), "{found:?}");
}

#[test]
fn a_grant_the_court_set_in_place_counts_as_earlier_but_carries_no_end() {
    let mut cell = cell();
    granted_by_the_court(&mut cell, APPLICANT);
    let app = applied(&mut cell, APPLICANT, "2027-03-01");

    // Whether that card is still valid the cell does not know: the decision
    // the court set in place carries no kaart_geldig_tot. It is silent, so
    // the college judges, and without that finding the decision fails.
    let e = without_examination(&mut cell, &[&ggd()], &app.id, "2027-03-01").unwrap_err();
    blocked_on(&e, VALID_C);

    // The college finds the card no longer valid. That a card was issued
    // before, the cell answers from the judgment, so the examination may be
    // left out (Regeling art. 2 lid 2 onder a).
    let mut findings = examined(true);
    findings.insert(LEFT_OUT.into(), json!(true));
    findings.insert(KNOWN.into(), json!(true));
    findings.insert(VALID_C.into(), json!(false));
    let gram = decide_with(&mut cell, &app.id, &findings, "2027-03-01").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let regelrecht_engine::FilledBy::Answer { answers, .. } =
        earlier_filled_by(&cell, &[&ggd()], &gram.id)
    else {
        panic!("one answer")
    };
    let [by] = answers.as_slice() else {
        panic!("one answer: {answers:?}")
    };
    assert_eq!(by.answerer, RECORDING_ACTOR);
}

#[test]
fn a_cell_the_term_does_not_name_is_not_asked_even_with_the_answer() {
    // The examination is the GGD's (Regeling art. 3): the term names who is
    // competent, so the binding asks the GGD and not the municipality, even
    // where its catalogue gives the answer (RFC-048 §4). Without the GGD the
    // college finds it.
    let dir = tempfile::tempdir().unwrap();
    let identity = std::fs::read_to_string(cell_dir().join("cel.yaml")).unwrap();
    common::write_identity(dir.path(), &format!("{identity}  {EXAMINED}: {REPORTS}\n"));
    let mut cell = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap();
    assert_eq!(cell.answers().len(), 4);
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let e = cell
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &examined(true),
            day("2026-10-20"),
        )
        .unwrap_err();
    assert!(
        e.to_string().contains(EXAMINED) && e.to_string().contains("no judgement was given"),
        "{e}"
    );
}
