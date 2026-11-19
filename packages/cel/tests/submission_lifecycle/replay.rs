//! Replaying a decision: it runs as the decision ran or fails, and a sandbox
//! leaves the cell alone.

use super::*;

/// A replay runs as the decision ran, or fails: read back with an outcome
/// the chronicle did not get from the law, the replay is not that decision.
#[test]
fn a_replay_that_does_not_run_as_the_decision_ran_fails() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join("kroniek.jsonl");
    let b = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
        apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2026-10-27")
    };
    assert_eq!(b.outputs["verlof_verleend"], json!(false));
    let with = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    let found = with.findings(&mut service(), &[], &b.id).unwrap();
    assert!(!found.contains(&ONCE.to_string()), "{found:?}");

    // The last line is B; it now says leave was granted.
    let text = std::fs::read_to_string(&path).unwrap();
    let (rest, last) = text.trim_end().rsplit_once('\n').unwrap();
    assert!(last.contains(&b.id));
    let tampered = last.replace("\"verlof_verleend\":false", "\"verlof_verleend\":true");
    assert_ne!(tampered, last);
    std::fs::write(&path, format!("{rest}\n{tampered}\n")).unwrap();
    let cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    let e = cell.explain(&mut service(), &[], &b.id).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("the replay is not this decision")
            && m.contains("verlof_verleend")),
        "{e}"
    );
    // Nor does the cell derive findings from it.
    assert!(cell.findings(&mut service(), &[], &b.id).is_err());
}

#[test]
fn a_decision_in_a_sandbox_stays_out_of_the_cell_and_its_file() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("kroniek.jsonl");
    let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let before = std::fs::read_to_string(&path).unwrap();

    let mut sandbox = cell.sandbox();
    assert_eq!(sandbox.answers(), cell.answers());
    let gram = decide(&mut sandbox, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    assert_eq!(sandbox.grams().len(), 2);

    assert_eq!(cell.grams(), std::slice::from_ref(&app));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    // The cell can still take that decision itself.
    decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
}
