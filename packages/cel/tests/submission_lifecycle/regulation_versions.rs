//! The regulation versions a record was taken under, and the replay that
//! names what changed since.

use super::*;

#[test]
fn a_submission_and_a_decision_keep_the_regulation_versions_they_were_taken_under() {
    let mut cell = cell();
    let decision = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let application = &cell.grams()[0];
    let ids = |gram: &Gram| -> Vec<String> {
        gram.law_versions.iter().map(|v| v.law_id.clone()).collect()
    };
    // The application ran art. 13a and the Awb hooks on it.
    assert!(ids(application).contains(&LAW.to_string()));
    assert!(ids(application).contains(&AWB.to_string()));
    // The decision ran the law, the Awb and the policies that fill its
    // terms, and took the answer of the school's reduction.
    for id in [LAW, AWB, POLICY, LEAVE_POLICY, REDUCTION] {
        assert!(
            ids(&decision).contains(&id.to_string()),
            "{id}: {:?}",
            ids(&decision)
        );
    }
    let year = decision
        .law_versions
        .iter()
        .find(|v| v.law_id == POLICY)
        .unwrap();
    assert_eq!(year.valid_from.as_deref(), Some("2026-08-01"));
    assert!(
        year.hash
            .as_deref()
            .is_some_and(|h| h.starts_with("sha256:"))
    );
    // A reference, once each: no version twice.
    let mut unique = decision.law_versions.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), decision.law_versions.len());
    assert_eq!(
        cell.law_since(&service(), &decision.id).unwrap(),
        LawSince::Unchanged
    );
}

#[test]
fn a_regelwerk_corrected_since_the_decision_is_named_while_the_outcome_holds() {
    // A correction that leaves the outcome as it was: the replay takes the
    // decision again without complaint, and the versions say that the school
    // year is no longer what the decision was taken under.
    let mut cell = cell();
    let decision = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let corrected = format!(
        "{}\n# Gecorrigeerd: een typefout in de toelichting.\n",
        school_year()
    );
    let now = service_with_school_year(&[corrected]);
    let LawSince::Changed(changes) = cell.law_since(&now, &decision.id).unwrap() else {
        panic!("the school year changed");
    };
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].recorded.law_id, POLICY);
    let changed = changes[0].now.as_ref().unwrap();
    assert_eq!(changed.valid_from.as_deref(), Some("2026-08-01"));
    assert_ne!(changed.hash, changes[0].recorded.hash);
    assert!(
        changes[0].to_string().contains("has other content"),
        "{}",
        changes[0]
    );
    let mut now = now;
    cell.explain(&mut now, &[], &decision.id).unwrap();
}

#[test]
fn a_regelwerk_gone_from_the_corpus_since_the_decision_is_named() {
    let mut cell = cell();
    let decision = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let without = service_without(&[LEAVE_POLICY]);
    let LawSince::Changed(changes) = cell.law_since(&without, &decision.id).unwrap() else {
        panic!("the leave policy is gone");
    };
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].recorded.law_id, LEAVE_POLICY);
    assert_eq!(changes[0].now, None);
    assert_eq!(
        changes[0].to_string(),
        format!("{LEAVE_POLICY} no longer has version 2026-08-01")
    );
}

#[test]
fn a_version_added_since_the_decision_that_changes_the_outcome_is_named_in_the_replay() {
    // A version of the leave policy added with effect from before the
    // decision, under which leave counts as granted in the school year
    // already: taken again, the decision refuses, and the replay says both
    // that the outcome differs and which regelwerk changed since.
    let mut cell = cell();
    let decision = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(decision.outputs["verlof_verleend"], json!(true));
    let path = common::corpus().join(format!(
        "regulation/nl/uitvoeringsbeleid/{LEAVE_POLICY}/2026-08-01.yaml"
    ));
    let policy = std::fs::read_to_string(path).unwrap();
    let retroactive = policy
        .replace("valid_from: '2026-08-01'", "valid_from: '2026-09-01'")
        .replace("default: $antwoord_verlof_al_verleend", "default: true");
    assert_eq!(retroactive.matches("default: true").count(), 1);
    let mut now = service_without(&[LEAVE_POLICY]);
    for version in [&policy, &retroactive] {
        now.load_law(version).unwrap();
    }
    let LawSince::Changed(changes) = cell.law_since(&now, &decision.id).unwrap() else {
        panic!("the leave policy changed");
    };
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].now.as_ref().unwrap().valid_from.as_deref(),
        Some("2026-09-01")
    );
    let e = cell
        .explain(&mut now, &[], &decision.id)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("the replay is not this decision")
            && e.contains(&format!(
                "{LEAVE_POLICY} is in force in version 2026-09-01 that day, not version \
                 2026-08-01"
            )),
        "{e}"
    );
}
