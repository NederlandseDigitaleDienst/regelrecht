//! The answer of the deciding cell's own reduction over its decisions.

use super::*;

#[test]
fn a_grant_still_in_force_counts_for_a_second_application() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let second = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    // The municipality's policy: no card next to a valid one.
    assert_eq!(second.fields[POWER], json!(false));
    assert_eq!(
        second.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn a_grant_counts_up_to_and_including_its_last_day() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let last_day = apply_and_decide(&mut cell, APPLICANT, true, "2031-10-19");
    assert_eq!(last_day.fields[POWER], json!(false));
    assert_eq!(
        last_day.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    let expired = apply_and_decide(&mut cell, APPLICANT, true, "2031-10-20");
    assert_eq!(expired.fields[POWER], json!(true));
    assert_eq!(
        expired.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(expired.outputs["kaart_geldig_tot"], json!("2036-10-19"));
}

#[test]
fn a_refusal_does_not_count() {
    // A refusal has no last day of validity (`kaart_geldig_tot` is null);
    // only a card that was granted counts. Without the reduction's
    // `verstrekt` filter this test fails through the engine (DATE_DIFF on an
    // absent operand), not on the assertion below: the filter guards that
    // comparison as well.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let gram = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-27");
    assert_eq!(gram.fields[POWER], json!(true));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_grant_to_another_subject_does_not_count() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "999990032", true, "2026-10-20");
    let gram = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-27");
    assert_eq!(gram.fields[POWER], json!(true));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_cell_lists_the_answers_it_gives() {
    // The answers of the municipality's reduction: whether the applicant
    // holds a valid card, and whether the college issued a card before
    // (Regeling art. 2 lid 2). The examination is the GGD's, in a cell of its
    // own. The catalogue also gives the count of the bedrijfsparkeervergunning:
    // a cell's answers are the cell's, whatever case asks. The service here
    // loads none of the rulework that binds it, so it fills no term.
    let cell = cell();
    let answers: Vec<(&str, &str)> = cell
        .answers()
        .iter()
        .map(|(answer, reduction)| (answer.as_str(), reduction.as_str()))
        .collect();
    assert_eq!(
        answers,
        [
            (PERMITS, PERMITS_REDUCTION),
            (EARLIER, REDUCTION),
            (VALID, REDUCTION)
        ]
    );
}
