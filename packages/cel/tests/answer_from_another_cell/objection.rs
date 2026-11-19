//! The objection, the decision on it and the court's judgment on that decision.

use super::*;

#[test]
fn an_objection_does_not_change_the_application() {
    // Awb 6:5 and 7:11 lid 1: the objection gives grounds, the college
    // reconsiders on the application as it was. A card as a passenger is a
    // new application.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let amended = objection(
        "ik kan niet verder lopen dan vijftig meter",
        &[("kaart_als_passagier", json!(true))],
    );
    let e = object(&mut cell, &refused.id, &amended, "2026-11-01").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not ask 'kaart_als_passagier'")),
        "{e}"
    );
    // The application and the refusal; the examination is in the GGD's
    // chronicle.
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn without_a_new_finding_the_refusal_stands() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let bezwaar = object(
        &mut cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    assert_eq!(bezwaar.fields["termijn_einddatum"], json!("2026-12-01"));
    assert_eq!(
        given(&cell, &bezwaar)["bezwaar_of_beroepschrift_tijdig_ingediend"],
        json!(true)
    );

    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided.len(), 1);
    let decision = &decided[0];
    assert_eq!(decision.basis, "algemene_wet_bestuursrecht#7:11");
    assert_eq!(decision.outputs[ADMISSIBLE], json!(true));
    assert_eq!(decision.outputs[CAUSE], json!(false));
    assert_eq!(decision.outputs[REVOKED], json!(false));
    assert_eq!(decision.outputs[NEW], Value::Null);
}

#[test]
fn a_new_finding_revokes_the_refusal_and_takes_a_grant_in_its_place() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let bezwaar = object(
        &mut cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    // A new medical examination finds a long-term walking limitation; the
    // college gives that finding anew, and the others stand.
    let decided = reconsider(&mut cell, &bezwaar.id, &walking(true), "2026-11-20");
    // One act, one record: the decision on the objection holds the new one.
    assert_eq!(decided.len(), 1);
    let decision = &decided[0];
    let new = common::in_turn(decision);

    assert_eq!(decision.outputs[CAUSE], json!(true));
    assert_eq!(decision.outputs[REVOKED], json!(true));
    assert_eq!(decision.outputs[NEW]["taken_on"], json!(bezwaar.basis));
    assert_eq!(new.basis, format!("{LAW}#{ARTICLE}"));
    assert_eq!(new.fields[WALKING], json!(true));
    assert_eq!(new.fields[OTHER], json!(false));
    assert_eq!(
        new.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // Its own validity, from its own day of issue.
    assert_eq!(new.outputs["kaart_geldig_tot"], json!("2031-11-19"));

    // The new decision counts: a later application is refused.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(false));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn a_grant_revoked_on_objection_does_not_count() {
    // Constructed: on objection against the grant, a new examination finds
    // no limitation, so the grant is revoked and a refusal takes its place.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let bezwaar = object(
        &mut cell,
        &granted.id,
        &objection("de kaart vermeldt alleen de letter B", &[]),
        "2026-11-01",
    )
    .unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &walking(false), "2026-11-20");
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    assert_eq!(
        common::in_turn(&decided[0]).outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    // Neither the revoked card nor the refusal in its place counts.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(true));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_grant_whose_revocation_the_court_annulled_counts_again() {
    // As above, the grant is revoked on objection. The court annuls that
    // decision on the objection with its legal effects (Awb 8:72): the
    // revocation no longer holds, nor the refusal in its place, and the card
    // is valid again.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let bezwaar = object(
        &mut cell,
        &granted.id,
        &objection("de kaart vermeldt alleen de letter B", &[]),
        "2026-11-01",
    )
    .unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &walking(false), "2026-11-20");
    assert_eq!(decided[0].outputs[REVOKED], json!(true));

    let judgment = judged_by_the_court(&mut cell, &decided[0], &annulled());
    assert_eq!(judgment.outputs["rechtsgevolgen_vernietigd"], json!(true));

    // A second application while the revived card is valid is refused.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(false));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn a_difference_in_derived_data_alone_gives_no_cause() {
    // The heroverweging runs art. 49 on the day of the decision on the
    // objection, a month after the grant, so it would issue a card valid a
    // month longer. That end is derived data (`derived`, RFC-045 §1), no
    // outcome, so it gives no cause: the outcome is the same, and the grant
    // stands with its own validity. During the heroverweging the cell reads
    // the chronicle without the contested grant, so the reduction does not
    // refuse the card for being next to itself.
    let mut cell = cell();
    let (granted, bezwaar) = granted_and_objected(&mut cell);
    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(true));
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);

    // The grant is as it was, and it still counts.
    let recorded = cell.grams().iter().find(|g| g.id == granted.id).unwrap();
    assert_eq!(recorded.outputs["kaart_geldig_tot"], json!("2031-10-19"));
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(false));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}
