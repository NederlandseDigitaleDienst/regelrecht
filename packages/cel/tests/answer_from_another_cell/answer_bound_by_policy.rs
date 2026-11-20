//! An answer of another cell that the policy binds to its terms, over the
//! deciding authority's own finding.

use super::*;

#[test]
fn the_answer_fills_the_term_over_the_authoritys_finding_to_grant() {
    // The college would find no limitation, but the advice does. The policy
    // fills the four judgements from the advice, and reads the college's own
    // judgement only without one (RFC-048 §2): the college cannot deviate
    // (Awb 4:84 is open).
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    advised(APPLICANT, "2026-10-16", true, "2026-10-19");
    let gram = decide(&mut cell, &app.id, false, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // The decision holds what the engine used: the advice.
    assert_eq!(gram.fields[WALKING], json!(true));
    assert_eq!(gram.fields[OTHER], json!(false));
    assert_eq!(
        filled_by(&cell, &gram.id, WALKING),
        regelrecht_engine::FilledBy::Regulation {
            law_id: POLICY.into(),
            article: "6".into()
        }
    );
    // The policy did not read the college's own judgement, so the decision
    // holds no finding on the walking limitation.
    let found = cell.findings(&mut service(), &[&ggd()], &gram.id).unwrap();
    assert!(!found.contains(&WALKING.to_string()), "{found:?}");
    assert!(!found.contains(&WALKING_C.to_string()), "{found:?}");
    // Asked why: in the trace, art. 7 asks the GGD's cell for the answer,
    // which its reduction gave from its advice.
    let result = cell.explain(&mut service(), &[&ggd()], &gram.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    fn find<'a>(
        node: &'a regelrecht_engine::PathNode,
        name: &str,
    ) -> Option<&'a regelrecht_engine::PathNode> {
        if node.name == name && node.answer.is_some() {
            return Some(node);
        }
        node.children.iter().find_map(|c| find(c, name))
    }
    let node = find(root, ADVICE[0]).unwrap_or_else(|| panic!("{}", root.render_box_drawing()));
    let by = node.answer.as_ref().unwrap();
    assert_eq!(by.answerer, GGD_ACTOR);
    assert_eq!(by.answer, ADVICE[0]);
    assert_eq!(by.law_id, ADVICE_REDUCTION);
}

#[test]
fn the_answer_fills_the_term_over_the_authoritys_finding_to_refuse() {
    // The college would find a limitation; the advice finds none, and the
    // advice decides.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    advised(APPLICANT, "2026-10-16", false, "2026-10-19");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    assert_eq!(gram.fields[WALKING], json!(false));
}

#[test]
fn without_the_answer_the_policy_takes_the_authoritys_finding() {
    // The GGD is silent for this application, so the policy is too (policy
    // art. 6 lid 2), and the college's finding counts.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // The policy fills the criterion, with the college's own judgement
    // where there is no advice (art. 6, lid 2).
    assert_eq!(
        filled_by(&cell, &gram.id, WALKING),
        regelrecht_engine::FilledBy::Regulation {
            law_id: POLICY.to_string(),
            article: "6".to_string(),
        }
    );
    let found = cell.findings(&mut service(), &[&ggd()], &gram.id).unwrap();
    assert!(found.contains(&WALKING_C.to_string()), "{found:?}");
}

#[test]
fn only_the_last_executogram_in_the_period_counts() {
    // An advice from before the date of the application does not count, and
    // of two in the term the later one does (policy art. 5 lid 3).
    let mut cell = cell();
    advised(APPLICANT, "2026-10-01", true, "2026-10-02");
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-14", "2026-10-15");
    advised(APPLICANT, "2026-10-14", true, "2026-10-15");
    advised(APPLICANT, "2026-10-16", false, "2026-10-17");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn an_executogram_outside_the_period_does_not_count() {
    // Policy art. 5 lid 3: only an examination between the date of the
    // application and the day of the decision. Without one the policy is
    // silent and the college's finding counts.
    let mut cell = cell();
    advised(APPLICANT, "2026-10-01", false, "2026-10-02");
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-14", "2026-10-15");
    advised(APPLICANT, "2026-10-21", false, "2026-10-21");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(
        filled_by(&cell, &gram.id, WALKING),
        regelrecht_engine::FilledBy::Regulation {
            law_id: POLICY.to_string(),
            article: "6".to_string(),
        }
    );
}

#[test]
fn two_executograms_from_the_same_day_are_an_error() {
    // Which of two advices of one examination day counts, the policy does
    // not say: the GGD's policy (art. 1) refuses to pick (FOREACH combine
    // ONLY), and the decision is not taken.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    advised(APPLICANT, "2026-10-16", true, "2026-10-19");
    advised(APPLICANT, "2026-10-16", false, "2026-10-19");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    assert!(e.to_string().contains("ONLY"), "{e}");
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn only_the_authority_the_fact_names_records_the_executogram() {
    // The advice is a fact of who examines (policy art. 5): the
    // municipality cannot record it, and so cannot give itself an advice.
    let mut cell = cell();
    let supplied = object_map(json!({
        "bsn_aanvrager": APPLICANT,
        "datum_onderzoek": "2026-10-16",
        WALKING_C: true,
        HELP_C: false,
        WHEELCHAIR_C: false,
        OTHER_C: false,
    }));
    let e = cell
        .record_executogram(&service(), ADVISED, &supplied, day("2026-10-19"))
        .unwrap_err();
    assert!(
        e.to_string().contains("is established by 'Gemeentelijke Gezondheidsdienst' or 'vanwege het gemeentelijk gezag aangewezen deskundige'"),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn a_new_answer_at_the_reconsideration_revokes_the_refusal() {
    // The advice refuses; on objection the GGD examines again and finds the
    // limitation. The new advice counts at the heroverweging, which falls
    // later (policy art. 5 lid 3), and the refusal is revoked.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    advised(APPLICANT, "2026-10-16", false, "2026-10-19");
    let refused = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        refused.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    let bezwaar = object(
        &mut cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    reported(&mut cell, APPLICANT, "2026-11-10", "2026-11-11");
    advised(APPLICANT, "2026-11-10", true, "2026-11-11");
    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    assert_eq!(
        common::in_turn(&decided[0]).outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_cell_the_binding_does_not_name_is_not_asked_even_with_the_answer() {
    // The advice is the GGD's (policy art. 5): its terms name who is
    // competent, so the binding of art. 7 asks the GGD and not the
    // municipality, even where its catalogue gives the answer (RFC-048 §4).
    // Without the GGD the college judges the criteria itself.
    let dir = tempfile::tempdir().unwrap();
    let identity = std::fs::read_to_string(cell_dir().join("cel.yaml")).unwrap();
    common::write_identity(
        dir.path(),
        &format!("{identity}  {}: {ADVICE_REDUCTION}\n", ADVICE[0]),
    );
    let mut cell = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap();
    assert_eq!(cell.answers().len(), 4);
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut findings = examined(true);
    findings.insert(EXAMINED_C.into(), json!(true));
    let gram = cell
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &findings,
            day("2026-10-20"),
        )
        .unwrap();
    let result = cell.explain(&mut service(), &[], &gram[0].id).unwrap();
    let walking = result
        .open_terms
        .iter()
        .find(|f| f.term == WALKING_C && f.law_id == POLICY)
        .unwrap();
    assert_eq!(walking.filled_by, regelrecht_engine::FilledBy::Finding);
    // Nowhere in the trace did the municipality give the advice.
    fn by_municipality(node: &regelrecht_engine::PathNode) -> bool {
        node.answer
            .as_ref()
            .is_some_and(|a| a.answer == ADVICE[0] && a.answerer == RECORDING_ACTOR)
            || node.children.iter().any(by_municipality)
    }
    let root = result.trace.as_ref().unwrap();
    assert!(!by_municipality(root), "{}", root.render_box_drawing());
}
