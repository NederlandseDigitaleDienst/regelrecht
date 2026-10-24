//! The appeal to the administrative court across two cells (RFC-053): the
//! school decides on an objection, the parent appeals to the court with a
//! copy of that decision, the court judges, and the school receives a copy
//! of the judgment and decides on the objection again.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use regelrecht_cel::{Afschrift, Cell, Error, Gram, Received, Reference};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const LAW: &str = "leerplichtwet_1969";
const ARTICLE: &str = "13a";
const AWB: &str = "algemene_wet_bestuursrecht";
const OBJECTION: &str = "7:11";
const APPEAL: &str = "8:70";
/// A fictional school and a fictional court.
const SCHOOL: &str = "obs_de_klimop";
const COURT: &str = "rechtbank_wilgendaal";
/// The fact that a party holds the copy of a judgment (Awb 8:79), which the
/// school records when it receives one.
const JUDGMENTS: &str = "algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak";

const POLICY: &str = "fictief_beleid_schooljaar_obs_de_klimop";
const SCHOOL_YEAR: &str = "fictief_beleid_schooljaargrenzen_obs_de_klimop";
/// The head's policy on vakantieverlof: the norms the reduction applies, and
/// the binding of the open term of art. 13a lid 2 to the school's answer
/// (RFC-048).
const LEAVE_POLICY: &str = "fictief_beleid_vakantieverlof_obs_de_klimop";
/// The school's reduction for lid 2, and the answer it gives.
const REDUCTION: &str = "reductie_verlof_al_verleend_in_schooljaar";
const ONCE: &str = "verlof_al_verleend_in_schooljaar";

const PROFESSION: &str = "specifieke_aard_beroep_ouder";
const OUTSIDE_HOLIDAYS: &str = "verlof_valt_buiten_schoolvakanties";

std::thread_local! {
    /// Whether the school's cell opens without its answer for lid 2, for a
    /// test in which the head judges that term himself. Each test runs on a
    /// thread of its own.
    static WITHOUT_REDUCTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The Leerplichtwet, the Awb, the school's policies and its reduction for
/// lid 2 from the corpus.
fn service() -> LawExecutionService {
    let regulation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/regulation/nl");
    let mut service = LawExecutionService::new();
    for law in [
        format!("wet/{LAW}"),
        format!("wet/{AWB}"),
        format!("uitvoeringsbeleid/{POLICY}"),
        format!("uitvoeringsbeleid/{SCHOOL_YEAR}"),
        format!("uitvoeringsbeleid/{LEAVE_POLICY}"),
        format!("reducties/{REDUCTION}"),
        // Which decisions still stand after objection and appeal: the
        // reduction of the Awb, which the school's reduction calls.
        "reducties/reductie_algemene_wet_bestuursrecht".to_string(),
    ] {
        for entry in std::fs::read_dir(regulation.join(law)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|x| x == "yaml") {
                service
                    .load_law(&std::fs::read_to_string(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
    }
    service
}

fn cells() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/cells")
}

/// A directory of the school's cell that gives only the answer for art. 13a
/// lid 2, or none when the test left it out ([`without_reduction`]). The
/// head judges art. 11, onder f, here, as in `tests/vakantieverlof.rs`.
fn school_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut yaml = "name: Het hoofd van OBS De Klimop\ncategory: het hoofd\n".to_string();
    if !WITHOUT_REDUCTION.get() {
        yaml.push_str(&format!("answers:\n  {ONCE}: {REDUCTION}\n"));
    }
    std::fs::write(dir.path().join("cel.yaml"), yaml).unwrap();
    dir
}

/// The school's cell with an empty chronicle. The cell reads its catalogue
/// when it opens, so the directory may go after.
fn school() -> Cell {
    Cell::in_memory(SCHOOL, school_dir().path()).unwrap()
}

/// The school's cell with its chronicle in the file at `path`.
fn school_at(path: &Path) -> Result<Cell, Error> {
    Cell::open(SCHOOL, path, school_dir().path())
}

fn court() -> Cell {
    Cell::in_memory(COURT, cells().join(COURT)).unwrap()
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn map(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

/// The refusal of 14 October: leave from 1 to 5 February 2027, refused
/// because the head judged that the profession of a parent is not of the
/// kind art. 11, onder f, asks.
fn refused(school: &mut Cell) -> Gram {
    refused_with(school, json!({ PROFESSION: false, OUTSIDE_HOLIDAYS: true }))
}

/// [`refused`] with what the head `findings`.
fn refused_with(school: &mut Cell, findings: Value) -> Gram {
    let application = map(json!({
        "bsn_jongere": "999990019",
        "eerste_verlofdag": "2027-02-01",
        "laatste_verlofdag": "2027-02-05",
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "J. Jansen",
        "adres_aanvrager": "Voorbeeldstraat 1",
        "dagtekening": "2026-10-12",
        "gevraagde_beschikking": "vakantieverlof",
    }));
    let applied = school
        .record_submission(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            None,
            &application,
            day("2026-10-12"),
        )
        .unwrap();
    let findings = map(findings);
    let refused = school
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &applied.id,
            &Map::new(),
            &findings,
            day("2026-10-14"),
        )
        .unwrap()
        .remove(0);
    assert_eq!(refused.outputs["verlof_verleend"], json!(false));
    refused
}

/// What a bezwaar- or beroepschrift holds (Awb 6:5).
fn grounds(dated: &str, against: &str, grounds: &str) -> Map<String, Value> {
    map(json!({
        "bezwaar_of_beroepschrift_ondertekend": true,
        "naam_indiener": "J. Jansen",
        "adres_indiener": "Voorbeeldstraat 1",
        "dagtekening_bezwaar_of_beroepschrift": dated,
        "omschrijving_bestreden_besluit": against,
        "gronden_bezwaar_of_beroep": grounds,
    }))
}

/// The objection of 1 November against `refused`, and the decision on it
/// of 15 November with what the head `findings` anew.
fn objected(school: &mut Cell, refused: &Gram, findings: Value) -> (Gram, Vec<Gram>) {
    let supplied = grounds(
        "2026-11-01",
        "weigering vakantieverlof",
        "verklaring van de werkgever",
    );
    let objection = school
        .record_submission(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            Some(Reference::Local(&refused.id)),
            &supplied,
            day("2026-11-01"),
        )
        .unwrap();
    let decided = school
        .decide(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &objection.id,
            &Map::new(),
            &map(findings),
            day("2026-11-15"),
        )
        .unwrap();
    (objection, decided)
}

/// The appeal of 1 December against the decision on the objection, with
/// the copy the school made of it.
fn appealed(court: &mut Cell, copy: &Afschrift) -> Result<Gram, Error> {
    appealed_on(court, copy, ["2026-11-30", "2026-12-01"])
}

/// [`appealed`], dated and received on other days.
fn appealed_on(court: &mut Cell, copy: &Afschrift, [dated, on]: [&str; 2]) -> Result<Gram, Error> {
    let supplied = grounds(
        dated,
        "beslissing op bezwaar tegen de weigering van vakantieverlof",
        "de beslissing is niet deugdelijk gemotiveerd",
    );
    court.record_submission(
        &mut service(),
        &[],
        AWB,
        APPEAL,
        Some(Reference::Received(copy)),
        &supplied,
        day(on),
    )
}

/// The court's judgement on a well-founded appeal: Awb 7:12 lid 1 is
/// breached, the legal effects do not stay, and the school gets two weeks.
fn well_founded() -> Map<String, Value> {
    map(json!({
        "indiener_is_belanghebbende": true,
        "rechtsregel_geschonden": true,
        "geschonden_rechtsregel": "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht",
        "rechtsgevolgen_in_stand": false,
        "uitspraak_treedt_in_de_plaats": false,
        "termijn_nieuw_besluit": "2027-01-25",
    }))
}

fn judged(court: &mut Cell, appeal: &Gram, findings: &Map<String, Value>) -> Gram {
    judged_on(court, appeal, findings, "2027-01-11")
}

/// [`judged`] on the day `on`.
fn judged_on(court: &mut Cell, appeal: &Gram, findings: &Map<String, Value>, on: &str) -> Gram {
    let mut grams = court
        .decide(
            &mut service(),
            &[],
            AWB,
            APPEAL,
            &appeal.id,
            &Map::new(),
            findings,
            day(on),
        )
        .unwrap();
    assert_eq!(grams.len(), 1);
    grams.remove(0)
}

/// Everything up to the copy of the judgment: the refusal, the objection
/// declared unfounded, the appeal and the judgment.
struct Case {
    school: Cell,
    court: Cell,
    refused: Gram,
    objection: Gram,
    on_objection: Gram,
    appeal: Gram,
    judgment: Gram,
}

fn case(findings: &Map<String, Value>) -> Case {
    case_with(school(), findings)
}

/// [`case`] in the cell `school`.
fn case_with(mut school: Cell, findings: &Map<String, Value>) -> Case {
    let mut court = court();
    let refused = refused(&mut school);
    let (objection, mut decided) = objected(&mut school, &refused, json!({}));
    let on_objection = decided.remove(0);
    assert_eq!(
        on_objection.outputs["bestreden_besluit_herroepen"],
        json!(false)
    );
    let copy = school
        .afschrift(&mut service(), &[], &on_objection.id)
        .unwrap();
    let appeal = appealed(&mut court, &copy).unwrap();
    let judgment = judged(&mut court, &appeal, findings);
    Case {
        school,
        court,
        refused,
        objection,
        on_objection,
        appeal,
        judgment,
    }
}

#[test]
fn the_copy_of_a_decision_holds_its_value_from_the_cell_that_took_it() {
    let mut school = school();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({}));
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    assert_eq!(copy.from, SCHOOL);
    assert_eq!(copy.gram, decided[0]);
    assert_eq!(
        copy.value["grondslag"],
        json!("algemene_wet_bestuursrecht#7:11")
    );
    assert_eq!(
        copy.value["taken_on"],
        json!("algemene_wet_bestuursrecht#7:11")
    );
    assert_eq!(
        copy.value["outputs"]["bestreden_besluit_herroepen"],
        json!(false)
    );
    // The objection contested a decision of the school itself.
    assert_eq!(copy.about, None);
}

#[test]
fn the_court_records_the_appeal_with_the_copy_it_received() {
    let mut school = school();
    let mut court = court();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({}));
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    let appeal = appealed(&mut court, &copy).unwrap();

    assert_eq!(appeal.kind, "BEROEP");
    assert_eq!(appeal.grondslag, "algemene_wet_bestuursrecht#8:70");
    assert_eq!(appeal.refers_to, None);
    assert_eq!(
        appeal.received,
        Some(Received {
            from: SCHOOL.into(),
            id: decided[0].id.clone()
        })
    );
    // The court keeps the decision value as it received it, not an id: it
    // cannot replay the school's decision.
    assert_eq!(appeal.fields["bestreden_besluit"], copy.value);
    // The period for appeal comes from the decision on the objection (Awb
    // 6:7 and 6:8 on that beschikking), and 6:9 says the appeal is on time.
    assert_eq!(appeal.fields["termijn_einddatum"], json!("2026-12-27"));
    assert_eq!(
        appeal.outputs["bezwaar_of_beroepschrift_tijdig_ingediend"],
        json!(true)
    );
    assert_eq!(
        appeal.outputs["bezwaar_of_beroepschrift_bevat_wat_artikel_6_5_eist"],
        json!(true)
    );
}

#[test]
fn the_court_annuls_the_decision_and_orders_a_new_one() {
    let case = case(&well_founded());
    let judgment = &case.judgment;
    assert_eq!(judgment.kind, "UITSPRAAK");
    assert_eq!(judgment.refers_to.as_deref(), Some(case.appeal.id.as_str()));
    assert_eq!(judgment.outputs["beroep_ontvankelijk"], json!(true));
    assert_eq!(judgment.outputs["beroep_gegrond"], json!(true));
    assert_eq!(
        judgment.outputs["uitspraak_strekt_tot"],
        json!("gegrondverklaring van het beroep")
    );
    assert_eq!(
        judgment.outputs["bestreden_besluit_vernietigd"],
        json!(true)
    );
    assert_eq!(judgment.outputs["rechtsgevolgen_vernietigd"], json!(true));
    assert_eq!(judgment.outputs["opdracht_nieuw_besluit"], json!(true));
    assert_eq!(
        judgment.outputs["termijn_nieuw_besluit_einddatum"],
        json!("2027-01-25")
    );
    // The stages of the procedure beroep, on the day of the judgment.
    assert_eq!(judgment.fields["uitspraak_datum"], json!("2027-01-11"));
    assert_eq!(judgment.fields["toezending_datum"], json!("2027-01-11"));
    assert_eq!(case.court.grams().len(), 2);
}

#[test]
fn the_court_explains_its_judgment_from_the_copy_it_keeps() {
    let case = case(&well_founded());
    let explained = case
        .court
        .explain(&mut service(), &[], &case.judgment.id)
        .unwrap();
    assert_eq!(
        explained.outputs["uitspraak_strekt_tot"].to_string(),
        "gegrondverklaring van het beroep"
    );
    let findings = case
        .court
        .findings(&mut service(), &[], &case.judgment.id)
        .unwrap();
    assert!(findings.contains(&"rechtsregel_geschonden".to_string()));
    assert!(findings.contains(&"rechtsgevolgen_in_stand".to_string()));
}

#[test]
fn the_copy_of_the_judgment_names_the_decision_it_concerns() {
    let case = case(&well_founded());
    let copy = case
        .court
        .afschrift(&mut service(), &[], &case.judgment.id)
        .unwrap();
    assert_eq!(copy.from, COURT);
    assert_eq!(
        copy.about,
        Some(Received {
            from: SCHOOL.into(),
            id: case.on_objection.id.clone()
        })
    );
}

/// The receipt of the copy of the judgment by the school, on 12 January.
fn received(case: &mut Case) -> Result<Gram, Error> {
    let copy = case
        .court
        .afschrift(&mut service(), &[], &case.judgment.id)
        .unwrap();
    case.school.record_receipt(
        &service(),
        JUDGMENTS,
        &copy,
        &case.on_objection.id,
        day("2027-01-12"),
    )
}

#[test]
fn the_school_records_the_receipt_of_the_judgment() {
    let mut case = case(&well_founded());
    let receipt = received(&mut case).unwrap();
    assert_eq!(receipt.kind, "EXECUTOGRAM");
    assert_eq!(receipt.fact.as_deref(), Some("afschrift_van_de_uitspraak"));
    assert_eq!(receipt.grondslag, "algemene_wet_bestuursrecht#8:79");
    assert_eq!(
        receipt.refers_to.as_deref(),
        Some(case.on_objection.id.as_str())
    );
    assert_eq!(
        receipt.received,
        Some(Received {
            from: COURT.into(),
            id: case.judgment.id.clone()
        })
    );
    assert_eq!(receipt.fields["bestreden_besluit_vernietigd"], json!(true));
    assert_eq!(receipt.fields["opdracht_nieuw_besluit"], json!(true));
    assert_eq!(
        receipt.fields["termijn_nieuw_besluit_einddatum"],
        json!("2027-01-25")
    );
    assert_eq!(
        receipt.fields["geschonden_geoordeelde_rechtsregel"],
        json!("artikel 7:12, eerste lid, van de Algemene wet bestuursrecht")
    );
    assert!(receipt.outputs.is_empty());
}

#[test]
fn after_the_judgment_the_school_decides_on_the_objection_again() {
    // The head decides on 18 January, within the term, and judges the
    // profession anew on the employer's statement: well founded, the
    // refusal is revoked and leave is granted.
    let mut case = case(&well_founded());
    let receipt = received(&mut case).unwrap();
    let decided = case
        .school
        .decide_after(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &case.objection.id,
            &receipt.id,
            &Map::new(),
            &map(json!({ PROFESSION: true })),
            day("2027-01-18"),
        )
        .unwrap();
    assert_eq!(decided.len(), 2);
    let (again, new) = (&decided[0], &decided[1]);
    assert_eq!(again.refers_to.as_deref(), Some(case.objection.id.as_str()));
    assert_eq!(again.after.as_deref(), Some(receipt.id.as_str()));
    assert_eq!(again.fields["bestreden_besluit"], json!(case.refused.id));
    assert_eq!(again.outputs["bestreden_besluit_herroepen"], json!(true));
    assert_eq!(again.outputs["nieuw_besluit"], json!(new.id));
    assert_eq!(new.grondslag, "leerplichtwet_1969#13a");
    assert_eq!(new.outputs["verlof_verleend"], json!(true));
    // The school's chronicle: the application, the refusal, the objection,
    // the decision on it, the receipt, and the two decisions taken again.
    assert_eq!(case.school.grams().len(), 7);
    // Both decisions on the objection replay as they were taken.
    case.school
        .explain(&mut service(), &[], &case.on_objection.id)
        .unwrap();
    // The replay takes the decision taken again, not the first decision on
    // the objection, which revoked nothing.
    let replayed = case.school.explain(&mut service(), &[], &again.id).unwrap();
    assert_eq!(
        replayed.outputs["bestreden_besluit_herroepen"],
        regelrecht_engine::Value::Bool(true)
    );
    case.school.explain(&mut service(), &[], &new.id).unwrap();
}

#[test]
fn a_chronicle_with_a_receipt_and_a_decision_taken_again_reads_back() {
    let file = tempfile::tempdir().unwrap();
    let path = file.path().join("kroniek.jsonl");
    let opened = school_at(&path).unwrap();
    let mut case = case_with(opened, &well_founded());
    let receipt = received(&mut case).unwrap();
    case.school
        .decide_after(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &case.objection.id,
            &receipt.id,
            &Map::new(),
            &map(json!({ PROFESSION: true })),
            day("2027-01-18"),
        )
        .unwrap();
    let written = case.school.grams().to_vec();
    let reopened = school_at(&path).unwrap();
    assert_eq!(reopened.grams(), written.as_slice());
}

#[test]
fn a_copy_of_what_is_no_decision_is_refused() {
    let mut school = school();
    let refused = refused(&mut school);
    let (objection, _) = objected(&mut school, &refused, json!({}));
    let err = school
        .afschrift(&mut service(), &[], &objection.id)
        .unwrap_err();
    assert!(err.to_string().contains("not a decision"), "{err}");
}

#[test]
fn a_cell_does_not_take_a_copy_of_its_own_decision() {
    let mut school = school();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({}));
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    let err = school
        .record_submission(
            &mut service(),
            &[],
            AWB,
            APPEAL,
            Some(Reference::Received(&copy)),
            &Map::new(),
            day("2026-12-01"),
        )
        .unwrap_err();
    assert!(err.to_string().contains("another cell"), "{err}");
}

#[test]
fn a_receipt_for_another_decision_is_refused() {
    let mut case = case(&well_founded());
    let copy = case
        .court
        .afschrift(&mut service(), &[], &case.judgment.id)
        .unwrap();
    // The judgment concerns the decision on the objection, not the refusal.
    let err = case
        .school
        .record_receipt(
            &service(),
            JUDGMENTS,
            &copy,
            &case.refused.id,
            day("2027-01-12"),
        )
        .unwrap_err();
    assert!(err.to_string().contains("not the decision"), "{err}");
}

#[test]
fn a_receipt_before_the_judgment_is_refused() {
    let mut case = case(&well_founded());
    let copy = case
        .court
        .afschrift(&mut service(), &[], &case.judgment.id)
        .unwrap();
    let err = case
        .school
        .record_receipt(
            &service(),
            JUDGMENTS,
            &copy,
            &case.on_objection.id,
            day("2027-01-10"),
        )
        .unwrap_err();
    assert!(err.to_string().contains("cannot be received"), "{err}");
    // On the day of the judgment it can.
    case.school
        .record_receipt(
            &service(),
            JUDGMENTS,
            &copy,
            &case.on_objection.id,
            day("2027-01-11"),
        )
        .unwrap();
}

#[test]
fn a_decided_objection_is_not_decided_again_without_a_later_chronolexogram() {
    let mut case = case(&well_founded());
    let err = case
        .school
        .decide(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &case.objection.id,
            &Map::new(),
            &Map::new(),
            day("2027-01-18"),
        )
        .unwrap_err();
    assert!(err.to_string().contains("already decided on"), "{err}");
    // Nor after a chronolexogram that does not refer to the decision on it.
    let refused = case.refused.id.clone();
    let err = case
        .school
        .decide_after(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &case.objection.id,
            &refused,
            &Map::new(),
            &Map::new(),
            day("2027-01-18"),
        )
        .unwrap_err();
    assert!(err.to_string().contains("not to"), "{err}");
}

#[test]
fn a_submission_not_decided_on_is_not_decided_again() {
    let mut school = school();
    let refused = refused(&mut school);
    let objection = school
        .record_submission(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            Some(Reference::Local(&refused.id)),
            &grounds("2026-11-01", "weigering", "gronden"),
            day("2026-11-01"),
        )
        .unwrap();
    let err = school
        .decide_after(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &objection.id,
            &refused.id,
            &Map::new(),
            &Map::new(),
            day("2026-11-15"),
        )
        .unwrap_err();
    assert!(err.to_string().contains("not decided on yet"), "{err}");
}

#[test]
fn a_judgment_that_keeps_the_legal_effects_orders_nothing() {
    let mut findings = well_founded();
    findings.insert("rechtsgevolgen_in_stand".into(), json!(true));
    findings.remove("termijn_nieuw_besluit");
    let mut case = case(&findings);
    assert_eq!(
        case.judgment.outputs["bestreden_besluit_vernietigd"],
        json!(true)
    );
    assert_eq!(
        case.judgment.outputs["rechtsgevolgen_vernietigd"],
        json!(false)
    );
    assert_eq!(
        case.judgment.outputs["opdracht_nieuw_besluit"],
        json!(false)
    );
    let receipt = received(&mut case).unwrap();
    assert_eq!(
        receipt.fields["termijn_nieuw_besluit_einddatum"],
        json!(null)
    );
}

#[test]
fn an_unfounded_appeal_is_received_too() {
    let findings = map(json!({
        "indiener_is_belanghebbende": true,
        "rechtsregel_geschonden": false,
    }));
    let mut case = case(&findings);
    assert_eq!(
        case.judgment.outputs["uitspraak_strekt_tot"],
        json!("ongegrondverklaring van het beroep")
    );
    let receipt = received(&mut case).unwrap();
    assert_eq!(receipt.fields["bestreden_besluit_vernietigd"], json!(false));
}

#[test]
fn a_grant_on_objection_the_court_annulled_no_longer_counts() {
    // The objection is well founded: the refusal is revoked and leave is
    // granted in its place. The court annuls that decision on the objection,
    // with its legal effects (Awb 8:72 lid 2), so the grant it took goes too
    // and the refusal is no longer revoked. The school's reduction for lid 2
    // reads the judgment the school received and asks the head's policy what
    // no longer counts: decided again, the head grants again, and the
    // annulled grant does not count as leave already granted.
    let mut school = school();
    let mut court = court();
    let refused = refused(&mut school);
    let (objection, decided) = objected(&mut school, &refused, json!({ PROFESSION: true }));
    assert_eq!(decided.len(), 2);
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    let appeal = appealed(&mut court, &copy).unwrap();
    let judgment = judged(&mut court, &appeal, &well_founded());
    let copy = court.afschrift(&mut service(), &[], &judgment.id).unwrap();
    let receipt = school
        .record_receipt(
            &service(),
            JUDGMENTS,
            &copy,
            &decided[0].id,
            day("2027-01-12"),
        )
        .unwrap();
    let again = school
        .decide_after(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            &objection.id,
            &receipt.id,
            &Map::new(),
            &map(json!({ PROFESSION: true })),
            day("2027-01-18"),
        )
        .unwrap();
    // The replacement the annulled decision took does not count against
    // this one: the cell lets it replace the refusal again.
    assert_eq!(again.len(), 2);
    assert_eq!(
        again[1].fields["verlof_al_verleend_in_schooljaar"],
        json!(false)
    );
    assert_eq!(again[1].outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_grant_whose_legal_effects_the_court_kept_still_counts() {
    // Annulled, but the legal effects stay (Awb 8:72 lid 3 onder a): the
    // grant on objection still counts for a later application.
    let mut school = school();
    let mut court = court();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({ PROFESSION: true }));
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    let appeal = appealed(&mut court, &copy).unwrap();
    let mut kept = well_founded();
    kept.insert("rechtsgevolgen_in_stand".into(), json!(true));
    kept.remove("termijn_nieuw_besluit");
    let judgment = judged(&mut court, &appeal, &kept);
    let copy = court.afschrift(&mut service(), &[], &judgment.id).unwrap();
    school
        .record_receipt(
            &service(),
            JUDGMENTS,
            &copy,
            &decided[0].id,
            day("2027-01-12"),
        )
        .unwrap();
    let application = map(json!({
        "bsn_jongere": "999990019",
        "eerste_verlofdag": "2027-05-10",
        "laatste_verlofdag": "2027-05-14",
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "J. Jansen",
        "adres_aanvrager": "Voorbeeldstraat 1",
        "dagtekening": "2027-02-01",
        "gevraagde_beschikking": "vakantieverlof",
    }));
    let later = school
        .record_submission(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            None,
            &application,
            day("2027-02-01"),
        )
        .unwrap();
    let decided = school
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &later.id,
            &Map::new(),
            &map(json!({ PROFESSION: true, OUTSIDE_HOLIDAYS: true })),
            day("2027-02-02"),
        )
        .unwrap();
    assert_eq!(
        decided[0].fields["verlof_al_verleend_in_schooljaar"],
        json!(true)
    );
    assert_eq!(decided[0].outputs["verlof_verleend"], json!(false));
}

#[test]
fn a_grant_on_objection_annulled_itself_no_longer_counts() {
    // The appeal is against the new decision the decision on objection took
    // (its `taken_on` is 7:11, so 8:70 finds it admissible). The court
    // annuls that grant with its legal effects: the school's reduction
    // leaves the grant itself out, by its own id.
    let mut school = school();
    let mut court = court();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({ PROFESSION: true }));
    let grant = &decided[1];
    let copy = school.afschrift(&mut service(), &[], &grant.id).unwrap();
    let appeal = appealed(&mut court, &copy).unwrap();
    let judgment = judged(&mut court, &appeal, &well_founded());
    assert_eq!(judgment.outputs["beroep_ontvankelijk"], json!(true));
    assert_eq!(judgment.outputs["rechtsgevolgen_vernietigd"], json!(true));
    let copy = court.afschrift(&mut service(), &[], &judgment.id).unwrap();
    school
        .record_receipt(&service(), JUDGMENTS, &copy, &grant.id, day("2027-01-12"))
        .unwrap();
    let application = map(json!({
        "bsn_jongere": "999990019",
        "eerste_verlofdag": "2027-05-10",
        "laatste_verlofdag": "2027-05-14",
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "J. Jansen",
        "adres_aanvrager": "Voorbeeldstraat 1",
        "dagtekening": "2027-02-01",
        "gevraagde_beschikking": "vakantieverlof",
    }));
    let later = school
        .record_submission(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            None,
            &application,
            day("2027-02-01"),
        )
        .unwrap();
    let decided = school
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &later.id,
            &Map::new(),
            &map(json!({ PROFESSION: true, OUTSIDE_HOLIDAYS: true })),
            day("2027-02-02"),
        )
        .unwrap();
    assert_eq!(
        decided[0].fields["verlof_al_verleend_in_schooljaar"],
        json!(false)
    );
}

/// Open the school's cell without its answer for art. 13a lid 2 for the
/// rest of the test: the head judges whether leave was already granted this
/// school year, so a grant the court annulled does not count against a
/// grant taken again.
fn without_reduction() {
    WITHOUT_REDUCTION.set(true);
}

/// The copy of `decision` appealed to `court` on the first day, judged well
/// founded on the second, and the copy of the judgment received by the
/// school on the third.
fn annulled(school: &mut Cell, court: &mut Cell, decision: &Gram, days: [&str; 3]) -> Gram {
    let copy = school.afschrift(&mut service(), &[], &decision.id).unwrap();
    let appeal = appealed_on(court, &copy, [days[0], days[0]]).unwrap();
    let judgment = judged_on(court, &appeal, &well_founded(), days[1]);
    let copy = court.afschrift(&mut service(), &[], &judgment.id).unwrap();
    school
        .record_receipt(&service(), JUDGMENTS, &copy, &decision.id, day(days[2]))
        .unwrap()
}

/// What the head finds when he grants the leave: without the reduction, he
/// also judges that no leave was granted this school year.
fn grant() -> Value {
    json!({
        PROFESSION: true,
        OUTSIDE_HOLIDAYS: true,
        "verlof_al_verleend_in_schooljaar": false,
    })
}

/// In `school`, the refusal, an objection that grants the leave in its
/// place, and two rounds in which the court annuls the decision on the
/// objection and the head grants again. Returns the receipts and the
/// decisions on the objection, in the order of the chronicle.
fn annulled_twice(school: &mut Cell) -> (Vec<Gram>, Vec<Gram>) {
    let mut court = court();
    let mut refusal = grant();
    refusal[PROFESSION] = json!(false);
    let refused = refused_with(school, refusal);
    let (objection, mut decided) = objected(school, &refused, grant());
    assert_eq!(decided.len(), 2);
    let (mut receipts, mut decisions) = (Vec::new(), vec![decided[0].clone()]);
    for days in [
        ["2026-12-01", "2027-01-11", "2027-01-12"],
        ["2027-02-01", "2027-03-01", "2027-03-02"],
    ] {
        let receipt = annulled(school, &mut court, &decided[0], days);
        decided = school
            .decide_after(
                &mut service(),
                &[],
                AWB,
                OBJECTION,
                &objection.id,
                &receipt.id,
                &Map::new(),
                &map(grant()),
                day(days[2]),
            )
            .unwrap();
        assert_eq!(decided.len(), 2);
        assert_eq!(decided[1].outputs["verlof_verleend"], json!(true));
        receipts.push(receipt);
        decisions.push(decided[0].clone());
    }
    (receipts, decisions)
}

#[test]
fn a_decision_annulled_twice_is_taken_again_twice() {
    // A replacement that an earlier decision on the objection took does not
    // count when the cell checks that a decision is replaced once, so the
    // objection can be decided on a third time with a new grant.
    without_reduction();
    let mut school = school();
    let (_, decisions) = annulled_twice(&mut school);
    assert_eq!(decisions.len(), 3);
}

#[test]
fn a_chronicle_whose_decision_taken_again_or_receipt_is_out_of_place_does_not_open() {
    without_reduction();
    let file = tempfile::tempdir().unwrap();
    let path = file.path().join("kroniek.jsonl");
    let mut school = school_at(&path).unwrap();
    let (receipts, decisions) = annulled_twice(&mut school);
    drop(school);
    let text = std::fs::read_to_string(&path).unwrap();
    let opens = |tampered: String| {
        assert_ne!(tampered, text);
        std::fs::write(&path, tampered).unwrap();
        school_at(&path).map(|_| ())
    };
    let (first, second) = (&receipts[0].id, &receipts[1].id);
    let objection = decisions[0].refers_to.as_deref().unwrap();
    let after = |id: &str| format!("\"after\":\"{id}\"");
    let refers_to = |id: &str| format!("\"refers_to\":\"{id}\"");
    for (tampered, says) in [
        // After a receipt for an older decision on the objection.
        (
            text.replace(&after(second), &after(first)),
            "is taken after",
        ),
        // After the objection itself, before any decision on it.
        (
            text.replace(
                &format!("\"id\":\"{}\"", decisions[0].id),
                &format!("\"id\":\"{}\",{}", decisions[0].id, after(objection)),
            ),
            "is taken after",
        ),
        // After a later gram.
        (
            text.replace(&after(first), &after(second)),
            "is taken after",
        ),
        // A receipt for an executogram.
        (
            text.replace(&refers_to(&decisions[1].id), &refers_to(first)),
            "or is an executogram",
        ),
        // A receipt taken after something, as only a decision is.
        (
            text.replace(
                &format!("\"id\":\"{first}\""),
                &format!("\"id\":\"{first}\",{}", after(objection)),
            ),
            "names `after`",
        ),
        // A receipt for nothing.
        (
            text.replace(&format!("{},", refers_to(&decisions[0].id)), ""),
            "refers to no decision",
        ),
    ] {
        let e = opens(tampered).unwrap_err();
        assert!(matches!(&e, Error::Setup(m) if m.contains(says)), "{e}");
    }
    std::fs::write(&path, &text).unwrap();
    school_at(&path).unwrap();
}

/// The court's judgement when it decides the case itself (Awb 8:72 lid 3
/// onder b): the decision on the objection taken anew by the court, with
/// its own judgement on the profession of the parent, and the fact the
/// school's reduction would otherwise give (8:69 lid 3).
fn decided_by_the_court() -> Map<String, Value> {
    let mut findings = well_founded();
    findings.remove("termijn_nieuw_besluit");
    findings.insert("uitspraak_treedt_in_de_plaats".into(), json!(true));
    findings.insert(PROFESSION.into(), json!(true));
    findings.insert("verlof_al_verleend_in_schooljaar".into(), json!(false));
    findings
}

#[test]
fn the_copy_of_a_decision_on_objection_holds_the_contested_decision_as_a_value() {
    let mut school = school();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({}));
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    let contested = &copy.value["fields"]["bestreden_besluit"];
    assert_eq!(contested["grondslag"], json!("leerplichtwet_1969#13a"));
    assert_eq!(contested["outputs"]["verlof_verleend"], json!(false));
    // In the school's own chronicle the decision on the objection keeps it
    // by its id.
    assert_eq!(decided[0].fields["bestreden_besluit"], json!(refused.id));
}

#[test]
fn the_court_decides_the_case_itself_and_the_school_counts_its_grant() {
    let mut case = case(&decided_by_the_court());
    let judgment = case.judgment.clone();
    assert_eq!(
        judgment.outputs["bestreden_besluit_vernietigd"],
        json!(true)
    );
    assert_eq!(judgment.outputs["rechtsgevolgen_vernietigd"], json!(true));
    assert_eq!(judgment.outputs["opdracht_nieuw_besluit"], json!(false));
    let instead = judgment.outputs["uitspraak_in_de_plaats"].clone();
    assert_eq!(
        instead["grondslag"],
        json!("algemene_wet_bestuursrecht#7:11")
    );
    assert_eq!(
        instead["outputs"]["bestreden_besluit_herroepen"],
        json!(true)
    );
    let grant = &instead["outputs"]["nieuw_besluit"];
    assert_eq!(grant["grondslag"], json!("leerplichtwet_1969#13a"));
    assert_eq!(grant["outputs"]["verlof_verleend"], json!(true));
    // The court takes no decision of the school in its own chronicle: the
    // judgment holds the decision it sets in place as its value.
    assert_eq!(case.court.grams().len(), 2);
    case.court
        .explain(&mut service(), &[], &judgment.id)
        .unwrap();

    let receipt = received(&mut case).unwrap();
    assert_eq!(receipt.fields["uitspraak_in_de_plaats"], instead);

    // The grant counts: a second application in the school year is refused.
    let application = map(json!({
        "bsn_jongere": "999990019",
        "eerste_verlofdag": "2027-05-10",
        "laatste_verlofdag": "2027-05-14",
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "J. Jansen",
        "adres_aanvrager": "Voorbeeldstraat 1",
        "dagtekening": "2027-02-01",
        "gevraagde_beschikking": "vakantieverlof",
    }));
    let later = case
        .school
        .record_submission(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            None,
            &application,
            day("2027-02-01"),
        )
        .unwrap();
    let decided = case
        .school
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &later.id,
            &Map::new(),
            &map(json!({ PROFESSION: true, OUTSIDE_HOLIDAYS: true })),
            day("2027-02-02"),
        )
        .unwrap();
    assert_eq!(
        decided[0].fields["verlof_al_verleend_in_schooljaar"],
        json!(true)
    );
    assert_eq!(decided[0].outputs["verlof_verleend"], json!(false));
}

#[test]
fn a_court_that_decides_itself_without_the_fact_the_school_would_give_fails() {
    let mut school = school();
    let mut court = court();
    let refused = refused(&mut school);
    let (_, decided) = objected(&mut school, &refused, json!({}));
    let copy = school
        .afschrift(&mut service(), &[], &decided[0].id)
        .unwrap();
    let appeal = appealed(&mut court, &copy).unwrap();
    let mut findings = decided_by_the_court();
    findings.remove("verlof_al_verleend_in_schooljaar");
    let err = court
        .decide(
            &mut service(),
            &[],
            AWB,
            APPEAL,
            &appeal.id,
            &Map::new(),
            &findings,
            day("2027-01-11"),
        )
        .unwrap_err();
    assert!(
        err.to_string().contains("verlof_al_verleend_in_schooljaar"),
        "{err}"
    );
    assert_eq!(court.grams().len(), 1);
}
