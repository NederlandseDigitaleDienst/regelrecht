//! The cell's answer in the trace of a decision, and its own account of it
//! (RFC-048 §5).

use super::*;

#[test]
fn the_cell_answers_the_policy_and_explains_the_decision_it_counted() {
    let mut cell = cell();
    let first = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let second = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(
        second.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    // Step 1: the trace of the second decision holds one node for the term,
    // within the policy that fills art. 49: the cell's answer, with the
    // reduction, its version and how far it read the chronicle. The policy
    // refuses, so art. 2 of the Regeling is not asked.
    let result = cell.explain(&mut service(), &[&ggd()], &second.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let nodes = answered(root);
    assert_eq!(nodes.len(), 1, "{}", root.render_box_drawing());
    let node = nodes[0];
    assert_eq!(node.name, VALID);
    assert_eq!(
        node.result,
        Some(regelrecht_engine::Value::from(json!(true)))
    );
    // Under it, how the cell came to the answer: the run of its reduction.
    assert_eq!(node.children.len(), 1, "{node:?}");
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, RECORDING_ACTOR);
    assert_eq!(answer.law_id, REDUCTION);
    assert_eq!(answer.valid_from, "2026-01-01");
    // Everything before this decision: the first application and decision,
    // and the second application.
    assert_eq!(answer.records_read, 3);
    let second_application = &cell.grams()[2];
    assert_eq!(second_application.kind, "AANVRAAG");
    assert_eq!(
        answer.last_record.as_deref(),
        Some(second_application.id.as_str())
    );
    // The reduction's FOREACH hangs under the answer in the decision's trace.
    assert!(node.render_box_drawing().contains("FOREACH"), "{node:?}");
    // The report says the same: the cell answered the policy's term.
    assert!(
        result.open_terms.iter().any(|f| f.term == VALID
            && f.law_id == POLICY
            && matches!(&f.filled_by, regelrecht_engine::FilledBy::Answer { law_id, article, answers, .. }
                if law_id == POLICY && article == "2" && answers == std::slice::from_ref(answer))),
        "{:?}",
        result.open_terms
    );

    // Step 2: asked why, the cell runs its reduction again with its own
    // trace, which shows the first card it counted.
    let explained = cell
        .explain_answer(&mut service(), &[&ggd()], &second.id, VALID)
        .unwrap();
    assert_eq!(explained.len(), 1);
    assert_eq!(
        explained[0].outputs[VALID],
        regelrecht_engine::Value::from(json!(true))
    );
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&first.id), "{why}");

    // Nothing stays behind in the service.
    let mut kept = service();
    let laws = kept.law_count();
    cell.explain_answer(&mut kept, &[&ggd()], &second.id, VALID)
        .unwrap();
    assert_eq!(kept.law_count(), laws);
}

#[test]
fn a_term_the_evaluation_does_not_reach_asks_no_cell() {
    // Art. 49 reads the college's power only for an applicant who meets the
    // criteria (AND stops at the first false), so the policy and its term
    // are not read, and the cell answers nothing.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let result = cell
        .explain(&mut service(), &[&ggd()], &refused.id)
        .unwrap();
    assert!(answered(result.trace.as_ref().unwrap()).is_empty());
    let e = cell
        .explain_answer(&mut service(), &[&ggd()], &refused.id, VALID)
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("did not ask")),
        "{e}"
    );
}

#[test]
fn at_the_reconsideration_the_cell_answers_without_the_contested_decision() {
    let mut cell = cell();
    let (granted, bezwaar) = granted_and_objected(&mut cell);
    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");

    // One node for the term, within art. 49 called anew: the cell read
    // everything before this decision but the contested grant. Art. 49
    // would issue the card again, so it asks art. 2 of the Regeling too,
    // and the cell answers from the report it received for the grant.
    let result = cell
        .explain(&mut service(), &[&ggd()], &decided[0].id)
        .unwrap();
    let root = result.trace.as_ref().unwrap();
    let all = answered(root);
    let names: Vec<&str> = all.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, [VALID, EXAMINED], "{}", root.render_box_drawing());
    assert_eq!(
        all[1].answer.as_ref().unwrap().law_id,
        REPORTS,
        "{}",
        root.render_box_drawing()
    );
    let nodes = &all[..1];
    assert_eq!(
        nodes[0].result,
        Some(regelrecht_engine::Value::from(json!(false)))
    );
    let answer = nodes[0].answer.as_ref().unwrap();
    let before = cell
        .grams()
        .iter()
        .take_while(|g| g.id != decided[0].id)
        .filter(|g| g.id != granted.id)
        .count();
    assert_eq!(answer.records_read, before);
    assert_eq!(answer.last_record.as_deref(), Some(bezwaar.id.as_str()));

    // The reduction's own account does not read the contested grant.
    let explained = cell
        .explain_answer(&mut service(), &[&ggd()], &decided[0].id, VALID)
        .unwrap();
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(!why.contains(&granted.id), "{why}");
}
