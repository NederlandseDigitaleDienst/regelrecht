//! Outcomes of art. 13a of the fictional school leave act, which follows
//! art. 13a of the Leerplichtwet 1969. These check values of one regelwerk,
//! not a construct of the cell. The same outcomes of the Leerplichtwet
//! itself belong in a bucket-A scenario next to it; these stay here until the
//! cell's steps run there.

use super::*;

#[test]
fn eleven_school_days_are_refused() {
    let mut cell = cell();
    // Monday 1 up to and including Monday 15 February.
    let app = applied(&mut cell, "2027-02-01", "2027-02-15", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlofdagen"], json!(11));
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    // A refusal is a decision too: it is motivated and open to objection.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(gram.outputs["termijn_weken"], json!(6));
}

#[test]
fn two_weeks_with_the_weekend_between_are_ten_school_days() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-12", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlofdagen"], json!(10));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn leave_in_the_first_two_weeks_of_the_school_year_is_refused() {
    let mut cell = cell();
    // The school year starts on Monday 31 August; the second week ends on
    // Sunday 13 September.
    let app = applied(&mut cell, "2026-09-10", "2026-09-11", "2026-08-20");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-25").unwrap();
    assert_eq!(gram.outputs["verlofdagen"], json!(2));
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(true)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));

    // The week after is fine.
    let app = applied(&mut cell, "2026-09-14", "2026-09-18", "2026-08-25");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-25").unwrap();
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn without_a_profession_of_a_specific_nature_leave_is_refused() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(false, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    assert_eq!(gram.fields[PROFESSION], json!(false));
}

#[test]
fn leave_within_the_school_holidays_is_refused() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, false), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
}

#[test]
fn leave_that_ends_on_the_first_lesdag_touches_the_first_two_weeks() {
    let mut cell = cell();
    // Ending on Monday 31 August, the first lesdag: refused.
    let app = applied(&mut cell, "2026-08-24", "2026-08-31", "2026-08-03");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-03").unwrap();
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(true)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));

    // Ending the day before: it does not touch them.
    let app = applied(&mut cell, "2026-08-24", "2026-08-30", "2026-08-03");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-03").unwrap();
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
}
