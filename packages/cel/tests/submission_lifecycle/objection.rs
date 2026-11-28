//! The objection, the reconsideration and the decision on the objection
//! (RFC-045 §7).

use super::*;

#[test]
fn the_shape_of_the_objection_comes_from_its_article() {
    let shape = shape::derive(&service(), AWB, OBJECTION, day("2026-11-01")).unwrap();
    assert_eq!(shape.kind, "BEZWAAR");
    assert_eq!(shape.stage.as_deref(), Some("BEZWAAR"));
    let fields: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    assert_eq!(
        fields,
        vec![
            ("bestreden_besluit", "test_general_administrative_act#7:11"),
            (
                "bezwaar_of_beroepschrift_ondertekend",
                "test_general_administrative_act#6:5"
            ),
            ("naam_indiener", "test_general_administrative_act#6:5"),
            ("adres_indiener", "test_general_administrative_act#6:5"),
            (
                "dagtekening_bezwaar_of_beroepschrift",
                "test_general_administrative_act#6:5"
            ),
            (
                "omschrijving_bestreden_besluit",
                "test_general_administrative_act#6:5"
            ),
            (
                "gronden_bezwaar_of_beroep",
                "test_general_administrative_act#6:5"
            ),
            ("termijn_einddatum", "test_general_administrative_act#6:9"),
        ]
    );
    // What 6:9 gives is no field, though 7:11 asks it.
    assert!(shape.given.iter().any(|g| g == TIMELY));
    // The objection asks for the decision of 7:11 (`requests`).
    assert_eq!(
        shape.requests.as_deref(),
        Some("test_general_administrative_act#7:11")
    );
    let moment = shape.moment.unwrap();
    assert_eq!(
        moment.parameter.as_deref(),
        Some("datum_ontvangst_bezwaar_of_beroepschrift")
    );
    assert_eq!(moment.declared_by, "test_general_administrative_act#6:9");
}

#[test]
fn the_decision_on_the_objection_asks_the_contested_decision() {
    let shape = decision::derive(&service(), AWB, OBJECTION, day("2026-11-15")).unwrap();
    assert_eq!(shape.kind, "BESCHIKKING");
    assert_eq!(shape.submission_kind.as_deref(), Some("BEZWAAR"));
    assert_eq!(shape.outputs, [ADMISSIBLE, CAUSE, REVOKED, NEW]);
    let source = |name: &str| shape.input(name).map(|i| &i.source);
    assert_eq!(source("bestreden_besluit"), Some(&Source::Application));
    assert_eq!(source(TIMELY), Some(&Source::Application));
    // What the heroverweging asks of art. 13a is no input of 7:11: the head
    // gives it among the findings.
    assert_eq!(source(PROFESSION), None);
}

#[test]
fn an_objection_refers_to_the_decision_and_takes_its_period_from_it() {
    let mut cell = cell();
    // Twelve school days, refused on 20 October; the period ends 1 December.
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    assert_eq!(refused.outputs["verlofdagen"], json!(12));
    let gram = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();

    assert_eq!(gram.kind, "BEZWAAR");
    assert_eq!(gram.basis, "test_general_administrative_act#7:11");
    assert_eq!(gram.refers_to.as_deref(), Some(refused.id.as_str()));
    // The contested decision, by its id.
    assert_eq!(gram.fields["bestreden_besluit"], json!(refused.id));
    // The period comes from the contested decision.
    assert_eq!(gram.fields["termijn_einddatum"], json!("2026-12-01"));
    assert_eq!(
        gram.fields["termijn_einddatum"],
        refused.outputs["termijn_einddatum"]
    );
    assert_eq!(gram.moment.as_ref().unwrap().on, day("2026-11-01"));
    assert_eq!(given(&cell, &gram)[TIMELY], json!(true));
    assert_eq!(
        given(&cell, &gram)["bezwaar_of_beroepschrift_bevat_wat_artikel_6_5_eist"],
        json!(true)
    );
    // Awb 7:10: six weeks from the day after the period ends.
    assert_eq!(
        given(&cell, &gram)["beslistermijn_bezwaar_einddatum"],
        json!("2027-01-12")
    );
    // The hooks on the decision do not fire on the objection.
    assert!(!gram.outputs.contains_key("motivering_vereist"));
}

#[test]
fn the_objector_does_not_supply_what_the_decision_holds_or_the_law_does_not_ask() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    for (name, value, says) in [
        (
            "termijn_einddatum",
            "2027-01-01",
            "comes from the decision it refers to",
        ),
        (
            "bestreden_besluit",
            "elders",
            "comes from the decision it refers to",
        ),
        ("bestemming", "Lissabon", "does not ask 'bestemming'"),
        (
            "datum_ontvangst_bezwaar_of_beroepschrift",
            "2026-10-25",
            "moment that counts",
        ),
        // An objection does not change the application (Awb 6:5, 7:11
        // lid 1), nor what a hook on it gave.
        ("laatste_verlofdag", "2027-02-12", "does not ask"),
        (
            "aanvraag_bevat_wat_artikel_4_2_eist",
            "true",
            "does not ask",
        ),
        ("bezwaar_of_beroepschrift_ondertekend", "ja", "does not fit"),
    ] {
        let e = object(
            &mut cell,
            &refused.id,
            &objection(&[(name, value)]),
            "2026-11-01",
        )
        .unwrap_err();
        assert!(matches!(&e, Error::Refused(m) if m.contains(says)), "{e}");
    }
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn an_objection_against_what_is_no_decision_is_refused() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = object(&mut cell, &app.id, &objection(&[]), "2026-11-01").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("not a decision")),
        "{e}"
    );
    // Nor is an objection a decision to object to.
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let e = object(&mut cell, &bezwaar.id, &objection(&[]), "2026-11-02").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("BEZWAAR under test_general_administrative_act#7:11, not a decision")),
        "{e}"
    );
}

#[test]
fn an_objection_without_the_contested_decision_is_recorded_and_not_decided_on() {
    // An objection that refers to no decision lacks the contested decision
    // and the end of the period; it is recorded (Awb 4:5 does not stop
    // that), but the decision on it needs the contested decision.
    let mut cell = cell();
    let bezwaar = cell
        .record_submission(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            None,
            &objection(&[]),
            day("2026-11-01"),
        )
        .unwrap();
    assert_eq!(bezwaar.refers_to, None);
    assert!(bezwaar.missing.iter().any(|m| m == "bestreden_besluit"));
    let e = decide_objection(&mut cell, &bezwaar.id, "2026-11-15").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("lacks 'bestreden_besluit'")),
        "{e}"
    );
}

#[test]
fn a_refusal_the_reconsideration_repeats_stands() {
    // Ongegrond: nothing changed, so 13a refuses again and the decision stands.
    let mut cell = cell();
    apply_and_decide(&mut cell, "2026-10-05", "2026-10-09", "2026-09-21");
    let second = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(second.outputs["verlof_verleend"], json!(false));
    let bezwaar = object(&mut cell, &second.id, &objection(&[]), "2027-03-10").unwrap();
    let decided = decide_objection(&mut cell, &bezwaar.id, "2027-04-01").unwrap();
    assert_eq!(decided.len(), 1);

    let decision = &decided[0];
    assert_eq!(decision.kind, "BESCHIKKING");
    assert_eq!(decision.basis, "test_general_administrative_act#7:11");
    assert_eq!(decision.refers_to.as_deref(), Some(bezwaar.id.as_str()));
    // The contested decision by its id; whether the objection came in on
    // time comes from the objection (Awb 6:9).
    assert_eq!(decision.fields["bestreden_besluit"], json!(second.id));
    assert_eq!(decision.fields[TIMELY], json!(true));
    assert_eq!(decision.fields[TIMELY], given(&cell, &bezwaar)[TIMELY]);
    assert!(!decision.fields.contains_key(PROFESSION));
    // The heroverweging gives no cause.
    assert_eq!(decision.outputs[ADMISSIBLE], json!(true));
    assert_eq!(decision.outputs[CAUSE], json!(false));
    assert_eq!(decision.outputs[REVOKED], json!(false));
    assert_eq!(decision.outputs[NEW], Value::Null);
    // The decision on the objection is a beschikking: motivated (3:46), and
    // open to appeal within six weeks (6:7, 6:8).
    assert_eq!(decision.outputs["motivering_vereist"], json!(true));
    assert_eq!(decision.outputs["termijn_einddatum"], json!("2027-05-13"));
    assert_eq!(cell.grams().len(), 6);

    // Decided once.
    let e = decide_objection(&mut cell, &bezwaar.id, "2027-04-02").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("already decided on")),
        "{e}"
    );
}

#[test]
fn a_finding_anew_at_the_reconsideration_revokes_the_refusal() {
    // Gegrond: refused because the head judged the profession of a parent
    // not of the kind art. 11 onder f asks. In the objection the parent
    // gives grounds, a statement of the employer; at the heroverweging the
    // head judges it anew, and art. 13a grants.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    // One act, one record: the decision on the objection holds the new one.
    assert_eq!(decided.len(), 1);
    let decision = &decided[0];
    let new = common::in_turn(decision);

    // What the head found anew is recorded with the decision on it.
    assert_eq!(decision.fields[PROFESSION], json!(true));
    assert_eq!(decision.outputs[CAUSE], json!(true));
    assert_eq!(decision.outputs[REVOKED], json!(true));
    // The decision on the objection holds the new decision as its value,
    // taken on the objection.
    assert_eq!(decision.outputs[NEW]["taken_on"], json!(bezwaar.basis));
    assert_eq!(new.basis, "test_school_leave_act#13a");
    // The application's own fields.
    assert_eq!(new.fields["eerste_verlofdag"], json!("2027-02-01"));
    assert_eq!(new.fields["laatste_verlofdag"], json!("2027-02-05"));
    // The judgement given anew replaces that one; the other stands from
    // the contested decision.
    assert_eq!(new.fields[PROFESSION], json!(true));
    assert_eq!(new.fields[OUTSIDE_HOLIDAYS], json!(true));
    assert_eq!(new.outputs["verlofdagen"], json!(5));
    assert_eq!(new.outputs["verlof_verleend"], json!(true));
    // The new decision has its own period, from its own announcement (Awb
    // 6:7, 6:8), not that of the revoked one.
    assert_eq!(new.outputs["termijn_einddatum"], json!("2026-12-27"));
    assert_ne!(
        new.outputs["termijn_einddatum"],
        refused.outputs["termijn_einddatum"]
    );
    assert_eq!(cell.grams().len(), 4);

    // The new decision counts: a later application this school year is refused.
    let later = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(later.fields[ONCE], json!(true));
    assert_eq!(later.outputs["verlof_verleend"], json!(false));
}

#[test]
fn an_objection_against_a_decision_on_objection_is_not_admissible() {
    // Against a decision on objection beroep is open, not bezwaar (Awb 7:1
    // lid 1 onder a). Art. 7:11 says so in its admissibility: the cell
    // records the objection and the decision on it, without a heroverweging.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let upheld = decide_objection(&mut cell, &bezwaar.id, "2026-11-15")
        .unwrap()
        .remove(0);
    assert_eq!(upheld.outputs[REVOKED], json!(false));

    let again = object(&mut cell, &upheld.id, &objection(&[]), "2026-11-20").unwrap();
    assert_eq!(again.fields["bestreden_besluit"], json!(upheld.id));
    // Its period comes from the decision on the objection.
    assert_eq!(given(&cell, &again)[TIMELY], json!(true));
    let decided = decide_objection(&mut cell, &again.id, "2026-11-25").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
}

#[test]
fn a_new_decision_changed_in_the_chronicle_does_not_replay() {
    // The new decision is part of the decision on the objection, which holds
    // it with every output its stages gave. A chronicle in which those
    // outputs were changed afterwards no longer holds what the decision
    // gave: taken again, the decision refuses it.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-15").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    cell.explain(&mut service(), &[], &decided[0].id).unwrap();
    let mut grams = cell.grams().to_vec();
    let held = grams.last_mut().unwrap().outputs.get_mut(NEW).unwrap();
    assert_eq!(held["outputs"]["verlof_verleend"], json!(true));
    held["outputs"]["verlof_verleend"] = json!(false);
    let dir = lid_2_dir();
    let changed = Cell::restore(&Disk, RECORDING_ACTOR, dir.path(), grams).unwrap();
    let e = changed
        .explain(&mut service(), &[], &decided[0].id)
        .unwrap_err();
    assert!(
        e.to_string().contains("the replay is not this decision"),
        "{e}"
    );
}

#[test]
fn a_chronicle_with_a_decision_recorded_in_turn_does_not_open() {
    // The older form: the new decision a decision on an objection took was
    // a chronolexogram of its own, which the decision held by its id. The
    // cell does not convert such a chronicle; it refuses it and says why.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-15").unwrap();
    reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    let mut grams = cell.grams().to_vec();
    let mut new = common::in_turn(grams.last().unwrap());
    new.id = "nieuw-besluit".into();
    new.refers_to = Some(bezwaar.id.clone());
    new.kind = "BESCHIKKING".into();
    grams
        .last_mut()
        .unwrap()
        .outputs
        .insert(NEW.into(), json!(new.id));
    grams.push(new);
    let dir = lid_2_dir();
    let Err(e) = Cell::restore(&Disk, RECORDING_ACTOR, dir.path(), grams) else {
        panic!("an older chronicle opens");
    };
    assert!(
        e.to_string().contains("is in an older form")
            && e.to_string()
                .contains("holds 'nieuw-besluit' as 'nieuw_besluit'"),
        "{e}"
    );
}

#[test]
fn a_decision_already_replaced_on_objection_is_not_replaced_again() {
    // Two objections against one grant, both on time. On the first, the
    // head judges the profession anew and a refusal takes the grant's place.
    // The second, decided after it, would revoke the grant again and take a
    // second refusal in its place: the cell refuses that decision, and
    // appends nothing.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(granted.outputs["verlof_verleend"], json!(true));
    let first = object(&mut cell, &granted.id, &objection(&[]), "2026-11-01").unwrap();
    let second = object(&mut cell, &granted.id, &objection(&[]), "2026-11-02").unwrap();
    let decided = reconsider(&mut cell, &first.id, &profession(false), "2026-11-15").unwrap();
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    let replaced_by = &decided[0].id;
    let recorded = cell.grams().len();

    let e = reconsider(&mut cell, &second.id, &profession(false), "2026-11-16").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("already replaces") && m.contains(replaced_by.as_str()) && m.contains(&granted.id)),
        "{e}"
    );
    assert_eq!(cell.grams().len(), recorded);
    // Without cause, the second is decided on: the grant is not revoked again.
    let decided = decide_objection(&mut cell, &second.id, "2026-11-16").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
}

#[test]
fn a_finding_anew_on_an_objection_not_admissible_is_refused() {
    // The head gives a finding anew on a late objection. There is no
    // heroverweging (Awb 7:11 lid 1), so no article asks it: an error that
    // names it, and nothing is recorded.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let late = object(&mut cell, &refused.id, &objection(&[]), "2026-12-02").unwrap();
    let recorded = cell.grams().len();
    let e = reconsider(&mut cell, &late.id, &profession(true), "2026-12-10").unwrap_err();
    assert!(e.to_string().contains(&format!("'{PROFESSION}'")), "{e}");
    assert_eq!(cell.grams().len(), recorded);
    // Without it, the objection is decided on and not admissible.
    let decided = decide_objection(&mut cell, &late.id, "2026-12-10").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn a_refusal_on_another_ground_on_objection_stands() {
    // The head now judges the profession of the kind 11 onder f asks, but
    // the leave within the school holidays: still refused, on another
    // ground. No outcome of art. 13a differs, so the heroverweging gives no
    // cause and the refusal stands.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(
        &mut cell,
        &bezwaar.id,
        &judgement(true, false),
        "2026-11-15",
    )
    .unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn a_smaller_request_after_a_refusal_is_a_new_application() {
    // Twelve school days are refused. Ten is a new application, not an
    // objection (Awb 6:5, 7:11 lid 1); the refusal does not count as leave
    // granted, so it is granted.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    assert_eq!(refused.outputs["verlof_verleend"], json!(false));
    let smaller = apply_and_decide(&mut cell, "2027-02-01", "2027-02-12", "2026-10-27");
    assert_eq!(smaller.outputs["verlofdagen"], json!(10));
    assert_eq!(smaller.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_grant_revoked_on_objection_does_not_count() {
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(granted.outputs["verlof_verleend"], json!(true));
    // Constructed: at the heroverweging the head judges the profession
    // anew, not of the kind art. 11 onder f asks, so 13a refuses anew, the
    // grant is revoked and a refusal takes its place.
    let bezwaar = object(&mut cell, &granted.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(false), "2026-11-15").unwrap();
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    let new = common::in_turn(&decided[0]);
    assert_eq!(new.outputs["verlof_verleend"], json!(false));

    // The revoked grant stays in the chronicle, and its row is as it was.
    // The row of the decision on the objection names it by id, says it
    // revoked it, and holds the refusal in its place as a row in turn; the
    // reduction leaves the grant out (RFC-048 §6).
    let rows = |basis: &str| {
        serde_json::to_value(
            regelrecht_cel::reading::rows(&service(), cell.grams(), basis, "BESCHIKKING").unwrap(),
        )
        .unwrap()
    };
    let granted_rows: Vec<Value> = rows("test_school_leave_act#13a")
        .as_array()
        .unwrap()
        .iter()
        .map(|row| json!([row["id"], row["verlof_verleend"]]))
        .collect();
    assert_eq!(granted_rows, [json!([granted.id, true])]);
    let on_objection = rows("test_general_administrative_act#7:11");
    assert_eq!(on_objection[0]["bestreden_besluit"], json!(granted.id));
    assert_eq!(on_objection[0][REVOKED], json!(true));
    assert_eq!(on_objection[0][NEW]["verlof_verleend"], json!(false));

    // Neither the revoked grant nor the refusal in its place counts.
    let later = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(later.fields[ONCE], json!(false));
    assert_eq!(later.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_late_objection_is_recorded_and_not_admissible() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    // On the last day of the period it is on time.
    let on_time = object(&mut cell, &refused.id, &objection(&[]), "2026-12-01").unwrap();
    assert_eq!(given(&cell, &on_time)[TIMELY], json!(true));
    // The day after it is late, and still recorded.
    let late = object(&mut cell, &refused.id, &objection(&[]), "2026-12-02").unwrap();
    assert_eq!(given(&cell, &late)[TIMELY], json!(false));

    // A late objection is not admissible (Awb 7:11 lid 1): it is not
    // reconsidered, so there is no judgement on cause, and the decision
    // stands.
    let decided = decide_objection(&mut cell, &late.id, "2026-12-10").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn an_objection_is_decided_with_decide_and_reads_back_from_the_file() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("kroniek.jsonl");
    let recorded = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        let refused = refused_on_profession(&mut cell);
        let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-15").unwrap();
        reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
        cell.grams().to_vec()
    };
    assert_eq!(recorded.len(), 4);
    let cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    assert_eq!(cell.grams(), recorded);
}

#[test]
fn a_chronicle_appends_a_batch_whole_or_not_at_all() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let mut chronicle = regelrecht_cel::Chronicle::in_memory();
    let mut later = app.clone();
    later.id = "later".into();
    let mut earlier = app.clone();
    earlier.id = "earlier".into();
    earlier.recorded_on = day("2026-10-01");
    let e = chronicle.append_all(vec![later, earlier]).unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );
    assert!(chronicle.grams().is_empty());
}

#[test]
fn the_reconsideration_does_not_count_the_contested_grant() {
    // Read with the contested grant, art. 13a would refuse once per school
    // year and revoke the grant. Without it, it grants anew, and the grant
    // stands.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let bezwaar = object(&mut cell, &granted.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = decide_objection(&mut cell, &bezwaar.id, "2026-11-15").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
}

#[test]
fn a_late_objection_is_not_admissible_where_the_reconsideration_would_fail() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    let late = object(&mut cell, &refused.id, &objection(&[]), "2026-12-02").unwrap();
    assert_eq!(given(&cell, &late)[TIMELY], json!(false));
    // Without the school's policy on the day of the decision, art. 13a anew
    // cannot be applied; the objection is not reconsidered, so that does not
    // stand in the way (Awb 7:11 lid 1).
    let mut without_policy = service_with_policy_lapsed();
    let decided = cell
        .decide(
            &mut without_policy,
            &[],
            AWB,
            OBJECTION,
            &late.id,
            &Map::new(),
            &common::on_objection(&Map::new()),
            day("2026-12-10"),
        )
        .unwrap();
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided.len(), 1);
    // An objection on time is reconsidered, and there it fails.
    let mut timely = self::cell();
    let refused = apply_and_decide(&mut timely, "2027-02-01", "2027-02-16", "2026-10-20");
    let on_time = object(&mut timely, &refused.id, &objection(&[]), "2026-12-01").unwrap();
    let e = timely
        .decide(
            &mut service_with_policy_lapsed(),
            &[],
            AWB,
            OBJECTION,
            &on_time.id,
            &Map::new(),
            &common::on_objection(&Map::new()),
            day("2026-12-10"),
        )
        .unwrap_err();
    // It fails in the call to art. 13a, on the first lesdag the policy no
    // longer gives.
    assert!(e.to_string().contains("eerste_lesdag_schooljaar"), "{e}");
}

/// Lazy evaluation keeps the heroverweging from running for an objection
/// that is not admissible (Awb 7:11 lid 1): the trace of the decision on a
/// late objection does not reach art. 13a, that on a timely one does.
#[test]
fn the_reconsideration_runs_only_for_an_admissible_objection() {
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    // The head's findings per case under art. 13a are no fields.
    let (findings, fields): (Map<String, Value>, Map<String, Value>) =
        refused.fields.clone().into_iter().partition(|(name, _)| {
            [
                PROFESSION,
                OUTSIDE_HOLIDAYS,
                OUTSIDE_HOLIDAYS_HEAD,
                ONCE,
                ONCE_HEAD,
            ]
            .contains(&name.as_str())
        });
    let contested = json!({
        "basis": refused.basis,
        "taken_on": "test_school_leave_act#13a",
        "fields": fields,
        "findings": findings,
        "outputs": {
            "verlofdagen": refused.outputs["verlofdagen"],
            "verlof_raakt_eerste_twee_lesweken": refused.outputs["verlof_raakt_eerste_twee_lesweken"],
            "verlof_verleend": refused.outputs["verlof_verleend"],
        },
    });
    let steps = |timely: bool| {
        let parameters = [
            ("bestreden_besluit", contested.clone()),
            (TIMELY, json!(timely)),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_string(), regelrecht_engine::Value::from(value)))
        .collect();
        // The head judges the objector an interested party, and at fault
        // for being late (Awb 6:11).
        let findings = common::on_objection(&Map::new())
            .into_iter()
            .map(|(name, value)| (name, regelrecht_engine::Value::from(value)))
            .collect();
        let result = service()
            .evaluate_law_with_trace_and_findings(
                AWB,
                &[REVOKED],
                parameters,
                findings,
                "2026-11-15",
            )
            .unwrap();
        assert_eq!(
            result.outputs[REVOKED],
            regelrecht_engine::Value::from(json!(false))
        );
        // The first line echoes the parameters, the basis among them.
        let trace = result.trace.as_ref().unwrap().render_box_drawing();
        trace.lines().skip(1).collect::<Vec<_>>().join("\n")
    };
    let late = steps(false);
    assert!(
        !late.contains("test_school_leave_act#13a"),
        "the heroverweging must not run:\n{late}"
    );
    let timely = steps(true);
    assert!(timely.contains("test_school_leave_act#13a"), "{timely}");
}

#[test]
fn the_authority_gives_at_the_decision_on_objection_only_findings_an_article_asks() {
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    for (given, says) in [
        // The contested decision comes from the objection: it is no finding.
        (
            json!({"bestreden_besluit": refused.id}),
            "'bestreden_besluit' of the decision of test_general_administrative_act#7:11 is no finding",
        ),
        // What no article in the decision asks, misspelt or not, is an
        // error that names it: it is not quietly dropped.
        (
            json!({"specifieke_aard_beroep": true}),
            "'specifieke_aard_beroep'",
        ),
        (json!({ CAUSE: true }), "'heroverweging_geeft_aanleiding'"),
        // A finding of another type than its term.
        (
            json!({ PROFESSION: "ja" }),
            "a boolean for the finding 'specifieke_aard_beroep_ouder'",
        ),
    ] {
        let given = object_map(given);
        let e = reconsider(&mut cell, &bezwaar.id, &given, "2026-11-15").unwrap_err();
        assert!(e.to_string().contains(says), "{says}: {e}");
    }
    assert_eq!(cell.grams().len(), 3);
}

#[test]
fn a_reduction_reads_the_decisions_on_objection_with_their_own_outcome() {
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    let rows = serde_json::to_value(
        regelrecht_cel::reading::rows(
            &service(),
            cell.grams(),
            "test_general_administrative_act#7:11",
            "BESCHIKKING",
        )
        .unwrap(),
    )
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    // Its own outcome: it revoked the refusal and took a grant in its place.
    assert_eq!(rows[0]["id"], json!(decided[0].id));
    assert_eq!(rows[0]["bestreden_besluit"], json!(refused.id));
    assert_eq!(rows[0][REVOKED], json!(true));
    assert_eq!(rows[0][ADMISSIBLE], json!(true));
    assert_eq!(rows[0][NEW]["verlof_verleend"], json!(true));
}

/// The owner's case (slice 4c): leave A is granted, leave B in the same
/// school year is refused because A counts, an objection against A revokes
/// A, and on the objection against B art. 13a called anew grants B. The
/// cell answers the term there; the engine records who answered. Why the
/// cell answered so is its own account: its reduction left A out, because
/// a decision on objection revoked it.
#[test]
fn a_grant_revoked_on_objection_no_longer_counts_on_the_objection_against_the_refusal() {
    let mut cell = cell();
    let a = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(a.outputs["verlof_verleend"], json!(true));
    let b = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2026-10-27");
    assert_eq!(b.fields[ONCE], json!(true));
    assert_eq!(b.outputs["verlof_verleend"], json!(false));

    // At the heroverweging of A the head judges the profession anew: A is
    // revoked and a refusal takes its place.
    let against_a = object(&mut cell, &a.id, &objection(&[]), "2026-11-01").unwrap();
    let on_a = reconsider(&mut cell, &against_a.id, &profession(false), "2026-11-15").unwrap();
    assert_eq!(on_a[0].outputs[REVOKED], json!(true));
    assert_eq!(
        common::in_turn(&on_a[0]).outputs["verlof_verleend"],
        json!(false)
    );

    // On the objection against B, art. 13a anew no longer counts A.
    let against_b = object(&mut cell, &b.id, &objection(&[]), "2026-11-20").unwrap();
    let on_b = decide_objection(&mut cell, &against_b.id, "2026-12-01").unwrap();
    assert_eq!(on_b[0].outputs[CAUSE], json!(true));
    assert_eq!(on_b[0].outputs[REVOKED], json!(true));
    let new_b = common::in_turn(&on_b[0]);
    assert_eq!(new_b.outputs["verlof_verleend"], json!(true));
    // The cell filled the term, so the new decision has no finding on it;
    // its value is the cell's answer.
    assert_eq!(new_b.fields[ONCE], json!(false));
    let found = &on_b[0].outputs[NEW]["findings"];
    assert!(found.get(ONCE).is_none(), "{found}");

    // Step 1: the trace of the decision on B's objection holds one node for
    // the term, within art. 13a called anew: the cell's answer, with the
    // reduction, its version and how far it read the chronicle. It read
    // everything before this decision but B, the contested decision.
    let result = cell.explain(&mut service(), &[], &on_b[0].id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let nodes = answered(root);
    assert_eq!(nodes.len(), 1, "{}", root.render_box_drawing());
    let node = nodes[0];
    assert_eq!(node.name, ONCE);
    assert_eq!(
        node.result,
        Some(regelrecht_engine::Value::from(json!(false)))
    );
    // Under it, how the cell came to the answer: the run of its reduction.
    assert_eq!(node.children.len(), 1, "{node:?}");
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, RECORDING_ACTOR);
    assert_eq!(answer.law_id, REDUCTION);
    assert_eq!(answer.valid_from, "2026-08-01");
    let before: Vec<&str> = cell
        .grams()
        .iter()
        .take_while(|g| g.id != on_b[0].id)
        .filter(|g| g.id != b.id)
        .map(|g| g.id.as_str())
        .collect();
    assert_eq!(answer.records_read, before.len());
    assert_eq!(answer.last_record.as_deref(), Some(against_b.id.as_str()));
    // The reduction's FOREACH hangs under the answer in the decision's
    // trace.
    assert!(node.render_box_drawing().contains("FOREACH"), "{node:?}");
    assert_eq!(
        result.outputs[REVOKED],
        regelrecht_engine::Value::from(json!(true))
    );
    // The report says the same: the cell answered the term.
    assert!(
        result.open_terms.iter().any(|f| f.term == ONCE
            && matches!(&f.filled_by, regelrecht_engine::FilledBy::Answer { law_id, article, answers, .. }
                if law_id == LEAVE_POLICY && article == "2" && answers == std::slice::from_ref(answer))),
        "{:?}",
        result.open_terms
    );

    // Step 2: asked why, the cell runs its reduction for that term again,
    // with its own trace: in the aggregate of A, A does not count, because
    // the decision on the objection against it revoked it (Awb 7:11), and
    // the refusal it took in A's place grants no leave. B itself is not
    // read.
    let explained = cell
        .explain_answer(&mut service(), &[], &on_b[0].id, ONCE)
        .unwrap();
    assert_eq!(explained.len(), 1);
    assert_eq!(
        explained[0].outputs[ONCE],
        regelrecht_engine::Value::from(json!(false))
    );
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(
        why.contains(&format!("the aggregate of {}", a.id))
            && why.contains(&format!("{}: revoked by {}", a.id, on_a[0].id)),
        "A is not revoked in its aggregate:\n{why}"
    );
    assert!(
        why.contains(&format!(
            "{}: fictief_beleid_verleend_verlof_obs_de_klimop#3: eerste_verlofdag: null -> \
             2027-02-01, verlof_verleend: false -> false",
            on_a[0].id
        )),
        "the refusal in A's place does not land in A's aggregate:\n{why}"
    );
    assert!(!why.contains(&b.id), "B was read:\n{why}");
    // A term the decision did not ask the cell is refused.
    let e = cell
        .explain_answer(&mut service(), &[], &on_b[0].id, "verlofdagen")
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("did not ask")),
        "{e}"
    );

    // Nothing stays behind in the service.
    let mut kept = service();
    let laws = kept.law_count();
    cell.explain(&mut kept, &[], &b.id).unwrap();
    cell.explain_answer(&mut kept, &[], &b.id, ONCE).unwrap();
    assert_eq!(kept.law_count(), laws);
    assert!(kept.data_registry().list_sources().is_empty());
}

/// The decision on B itself, explained: the cell answered that leave was
/// already granted, and its own account shows the grant A it counted.
#[test]
fn the_cell_explains_the_grant_its_reduction_counted_for_a_refusal() {
    let mut cell = cell();
    let a = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let b = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2026-10-27");
    let result = cell.explain(&mut service(), &[], &b.id).unwrap();
    let trace = result.trace.as_ref().unwrap().render_box_drawing();
    assert!(trace.contains(REDUCTION), "{trace}");
    // The projection under the answer shows the grant A it counted.
    assert!(trace.contains(&a.id), "{trace}");
    assert_eq!(
        result.outputs["verlof_verleend"],
        regelrecht_engine::Value::from(json!(false))
    );
    let explained = cell
        .explain_answer(&mut service(), &[], &b.id, ONCE)
        .unwrap();
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&a.id), "{why}");
    assert_eq!(
        explained[0].outputs[ONCE],
        regelrecht_engine::Value::from(json!(true))
    );
}
