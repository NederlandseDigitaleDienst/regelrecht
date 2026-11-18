//! An answer a cell gives from a fact, without a reduction written for it
//! (`cel.yaml`, `answers`, [`regelrecht_cel::standard`]): the standard
//! reduction is the written-out copy reduction, structurally and in what it
//! answers, also after a revocation and on a day before a record.

#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use regelrecht_cel::Cell;
use regelrecht_cel::reading::Reader;
use regelrecht_engine::{AnswerRequest, LawExecutionService, OpenTermAnswerer, Value};
use serde_json::{Map, json};

use common::{cell_tempdir, day, keep_reduction};

const ACTOR: &str = "testcel";

/// The fictional register of the cell's BDD suite: article 1 records where
/// a resident lives, article 2 corrects the postcode of such a record, and
/// article 3 records that one was wrong from the start, which revokes it.
fn register() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("bdd/fixtures/regulation/test_chronicle_corrections/2025-01-01.yaml"),
    )
    .unwrap()
}

/// The fictional register of the PoC's test source for prefilling, and its
/// reduction, which copies each fact into its aggregates by hand.
fn poc_source(path: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../poc-chronolexografie/tests/bron-voorinvullen")
            .join(path),
    )
    .unwrap()
}

/// The reduction written out by hand that the short form
/// `inschrijvingen` stands for: one aggregate per registration with every
/// field of the fact, a correction that sets the postcode it holds, and a
/// read model that gives the aggregates of the resident as rows.
const BY_HAND: &str = r"---
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json
$id: test_reductie_inschrijvingen
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
name: Inschrijvingen van een bewoner (testreductie)
articles:
  - number: inschrijving
    text: FICTIEF. Per inschrijving de velden, met de correcties erop.
    machine_readable:
      projection:
        per: test_chronicle_corrections#1#ingeschreven
        state:
          - name: bewoner
            type: string
          - name: postcode
            type: string
          - name: huisnummer
            type: number
        steps:
          - record: test_chronicle_corrections#1#ingeschreven
            actions:
              - output: bewoner
                value: $record.bewoner
              - output: postcode
                value: $record.postcode
              - output: huisnummer
                value: $record.huisnummer
          - record: test_chronicle_corrections#2#postcode_gecorrigeerd
            actions:
              - output: bewoner
                value: $record.bewoner
              - output: postcode
                value: $record.postcode
  - number: inschrijvingen_met_de_hand
    text: FICTIEF. De inschrijvingen van de bewoner, als rijen.
    machine_readable:
      execution:
        parameters:
          - name: bewoner
            type: string
        input:
          - name: inschrijvingen
            type: array
            source:
              projections: test_reductie_inschrijvingen#inschrijving
              where:
                bewoner: $bewoner
        output:
          - name: inschrijvingen_met_de_hand
            type: array
        actions:
          - output: inschrijvingen_met_de_hand
            value: $inschrijvingen
";

/// The same answer in the short form.
const SHORT: &str = "  inschrijvingen:
    fact: test_chronicle_corrections#1#ingeschreven
    referred_by: [test_chronicle_corrections#2#postcode_gecorrigeerd]
    where: [bewoner]
";

/// A cell that answers `inschrijvingen_met_de_hand` from the reduction
/// written out by hand and `inschrijvingen` from the fact, with the
/// register and both reductions in the service.
fn cell_and_service() -> (Cell, LawExecutionService, tempfile::TempDir) {
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        format!(
            "name: Testcel\nanswers:\n  inschrijvingen_met_de_hand: \
             test_reductie_inschrijvingen\n{SHORT}"
        ),
    )
    .unwrap();
    keep_reduction(dir.path(), "test_reductie_inschrijvingen", BY_HAND);
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let mut service = LawExecutionService::new();
    service.load_law(&register()).unwrap();
    service.load_law(BY_HAND).unwrap();
    cell.load_standard_reductions(&mut service).unwrap();
    (cell, service, dir)
}

fn fields(pairs: &[(&str, serde_json::Value)]) -> Map<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

/// What the cell answers for `answer` about `bewoner` on `on`.
fn answer(
    cell: &Cell,
    service: &LawExecutionService,
    answer: &str,
    bewoner: &str,
    on: &str,
) -> Value {
    let reader = Reader::new(
        cell.recording_actor(),
        cell.authority(),
        cell.answers(),
        cell.reductions(),
        cell.grams().to_vec(),
        None,
    );
    let parameters: BTreeMap<String, Value> =
        [("bewoner".to_string(), Value::String(bewoner.to_string()))].into();
    let request = AnswerRequest {
        name: answer,
        parameters: &parameters,
        calculation_date: on,
        trace: false,
    };
    reader
        .answer(service, &request)
        .unwrap()
        .map_or(Value::Null, |a| a.value)
}

/// The aggregates a row gives, by the postcode of each, in order.
fn postcodes(rows: &Value) -> Vec<Value> {
    let Value::Array(rows) = rows else {
        panic!("no rows: {rows:?}");
    };
    rows.iter()
        .map(|row| match row {
            Value::Object(row) => row.get("postcode").cloned().unwrap_or(Value::Null),
            other => panic!("no row: {other:?}"),
        })
        .collect()
}

/// The aggregate article of the reduction `id` in force on `on`.
fn projection_of(
    service: &LawExecutionService,
    id: &str,
    article: &str,
    on: &str,
) -> regelrecht_law_model::Projection {
    service
        .resolver()
        .get_law_for_date(id, Some(day(on)))
        .unwrap_or_else(|| panic!("no '{id}' on {on}"))
        .find_article_by_number(article)
        .and_then(|a| a.machine_readable.as_ref()?.projection.clone())
        .unwrap_or_else(|| panic!("'{id}' article {article} has no projection"))
}

/// The standard reduction of a fact is the copy reduction the PoC's test
/// source wrote by hand, one aggregate article at a time: the same `per`,
/// the same state with its types and the columns of a table, the same
/// steps.
#[test]
fn the_standard_reduction_is_the_copy_reduction_written_out() {
    for (fact, by_hand) in [
        (
            "voorbeeld_register#1#vereniging_ingeschreven",
            "inschrijving",
        ),
        ("voorbeeld_register#2#bestuur_vastgesteld", "bestuur"),
    ] {
        let dir = cell_tempdir();
        std::fs::write(
            dir.path().join("cel.yaml"),
            format!("name: Testcel\nanswers:\n  rijen: {fact}\n"),
        )
        .unwrap();
        let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
        let mut service = LawExecutionService::new();
        for path in [
            "regulation/nl/uitvoeringsbeleid/voorbeeld_register/2026-01-01.yaml",
            "cells/voorbeeld_register/reductions/reductie_voorbeeld_register/2026-01-01.yaml",
        ] {
            service.load_law(&poc_source(path)).unwrap();
        }
        cell.load_standard_reductions(&mut service).unwrap();
        let (_, id) = cell.standard_answers().next().unwrap();
        assert_eq!(
            projection_of(
                &service,
                &id,
                regelrecht_cel::standard::AGGREGATE,
                "2026-06-01"
            ),
            projection_of(
                &service,
                "reductie_voorbeeld_register",
                by_hand,
                "2026-06-01"
            ),
            "{fact}"
        );
    }
}

/// Three registrations, two of resident `b1`: the postcode of the first
/// is corrected, the second is revoked as wrong from the start.
fn record_registrations(cell: &mut Cell, service: &LawExecutionService) {
    let fact =
        |c: &mut Cell, fact: &str, f: &[(&str, serde_json::Value)], to: Option<&str>, on: &str| {
            c.record_executogram_referring(service, fact, &fields(f), to, day(on))
                .unwrap()
                .id
        };
    let a = fact(
        cell,
        "test_chronicle_corrections#1#ingeschreven",
        &[
            ("bewoner", json!("b1")),
            ("postcode", json!("1111AA")),
            ("huisnummer", json!(1)),
        ],
        None,
        "2025-02-01",
    );
    let b = fact(
        cell,
        "test_chronicle_corrections#1#ingeschreven",
        &[
            ("bewoner", json!("b1")),
            ("postcode", json!("2222BB")),
            ("huisnummer", json!(2)),
        ],
        None,
        "2025-03-01",
    );
    fact(
        cell,
        "test_chronicle_corrections#1#ingeschreven",
        &[
            ("bewoner", json!("b2")),
            ("postcode", json!("3333CC")),
            ("huisnummer", json!(3)),
        ],
        None,
        "2025-03-01",
    );
    fact(
        cell,
        "test_chronicle_corrections#2#postcode_gecorrigeerd",
        &[("bewoner", json!("b1")), ("postcode", json!("1111AB"))],
        Some(&a),
        "2025-04-01",
    );
    fact(
        cell,
        "test_chronicle_corrections#3#inschrijving_onjuist",
        &[("reden", json!("verkeerd adres"))],
        Some(&b),
        "2025-05-01",
    );
}

/// With a fact that refers to the root, the standard reduction answers what
/// the reduction written out by hand answers: before any record, after a
/// correction that sets the postcode it holds, and after a fact that
/// revokes a registration.
#[test]
fn a_standard_answer_answers_what_the_written_out_reduction_answers() {
    let (mut cell, service, _dir) = cell_and_service();
    record_registrations(&mut cell, &service);

    for on in [
        "2025-01-15",
        "2025-02-15",
        "2025-03-15",
        "2025-04-15",
        "2025-06-01",
    ] {
        for bewoner in ["b1", "b2", "b3"] {
            assert_eq!(
                answer(&cell, &service, "inschrijvingen", bewoner, on),
                answer(&cell, &service, "inschrijvingen_met_de_hand", bewoner, on),
                "{bewoner} on {on}"
            );
        }
    }
    let s = |p: &str| Value::String(p.to_string());
    assert_eq!(
        postcodes(&answer(
            &cell,
            &service,
            "inschrijvingen",
            "b1",
            "2025-01-15"
        )),
        Vec::<Value>::new()
    );
    assert_eq!(
        postcodes(&answer(
            &cell,
            &service,
            "inschrijvingen",
            "b1",
            "2025-03-15"
        )),
        vec![s("1111AA"), s("2222BB")]
    );
    // The correction sets the postcode it holds under the same name.
    assert_eq!(
        postcodes(&answer(
            &cell,
            &service,
            "inschrijvingen",
            "b1",
            "2025-04-15"
        )),
        vec![s("1111AB"), s("2222BB")]
    );
    // A revoked registration plays no step: its aggregate holds the start
    // values, as the reduction written out by hand gives it.
    assert_eq!(
        postcodes(&answer(
            &cell,
            &service,
            "inschrijvingen",
            "b1",
            "2025-06-01"
        )),
        vec![s("1111AB"), Value::Null]
    );
}

/// An answer that selects on a field the fact does not hold, one that
/// names the same fact twice, one named like the aggregate article, and one
/// whose standard reduction has the `$id` of a reduction the cell keeps, are
/// refused.
#[test]
fn a_standard_answer_that_cannot_be_written_out_is_refused() {
    let dir = cell_tempdir();
    let write = |answers: &str| {
        std::fs::write(
            dir.path().join("cel.yaml"),
            format!("name: Testcel\nanswers:\n{answers}"),
        )
        .unwrap();
    };
    write("  rijen:\n    fact: test_chronicle_corrections#1#ingeschreven\n    where: [kenteken]\n");
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let mut service = LawExecutionService::new();
    service.load_law(&register()).unwrap();
    let err = cell.load_standard_reductions(&mut service).unwrap_err();
    assert!(err.to_string().contains("kenteken"), "{err}");

    for (answers, says) in [
        (
            "  rijen:\n    fact: test_chronicle_corrections#1#ingeschreven\n    referred_by: [test_chronicle_corrections#1#ingeschreven]\n",
            "twice",
        ),
        (
            "  aggregaat: test_chronicle_corrections#1#ingeschreven\n",
            "its own article",
        ),
    ] {
        write(answers);
        let err = Cell::in_memory(ACTOR, dir.path()).unwrap_err();
        assert!(err.to_string().contains(says), "{err}");
    }

    write("  rijen: test_chronicle_corrections#1#ingeschreven\n");
    keep_reduction(
        dir.path(),
        "standaard_testcel_rijen",
        &BY_HAND.replace("test_reductie_inschrijvingen", "standaard_testcel_rijen"),
    );
    let err = Cell::in_memory(ACTOR, dir.path()).unwrap_err();
    assert!(err.to_string().contains("keeps a reduction"), "{err}");
}

/// A service that already holds another rulework under the `$id` of a
/// standard reduction refuses to take the standard reduction in its place,
/// as when two cells' names give the same `$id`.
#[test]
fn a_standard_reduction_does_not_pass_for_another_rulework() {
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        "name: Testcel\nanswers:\n  rijen: test_chronicle_corrections#1#ingeschreven\n",
    )
    .unwrap();
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let mut service = LawExecutionService::new();
    service.load_law(&register()).unwrap();
    service
        .load_law(&BY_HAND.replace("test_reductie_inschrijvingen", "standaard_testcel_rijen"))
        .unwrap();
    let err = cell.load_standard_reductions(&mut service).unwrap_err();
    assert!(err.to_string().contains("another rulework"), "{err}");
}

/// A misspelled key, a name with `#` that is no fact, and an answer name
/// that cannot be part of a `$id` are refused when the cell opens, with
/// the answer they belong to; a misspelled referring fact when the
/// reduction is written out.
#[test]
fn a_malformed_answer_from_a_fact_is_refused_by_name() {
    let dir = cell_tempdir();
    for (answers, says) in [
        (
            "  rijen:\n    fact: test_chronicle_corrections#1#ingeschreven\n    refered_by: []\n",
            "refered_by",
        ),
        ("  rijen: Test_register#1#ingeschreven\n", "neither the $id"),
        (
            "  Rijen: test_chronicle_corrections#1#ingeschreven\n",
            "lowercase",
        ),
    ] {
        std::fs::write(
            dir.path().join("cel.yaml"),
            format!("name: Testcel\nanswers:\n{answers}"),
        )
        .unwrap();
        let err = Cell::in_memory(ACTOR, dir.path()).unwrap_err();
        assert!(err.to_string().contains(says), "{answers}: {err}");
    }
    // A referring fact the regulation does not require is a misspelling,
    // refused when the reduction is written out.
    std::fs::write(
        dir.path().join("cel.yaml"),
        "name: Testcel\nanswers:\n  rijen:\n    fact: test_chronicle_corrections#1#ingeschreven\n    referred_by: [test_chronicle_corrections#2#postcode_gecorigeerd]\n",
    )
    .unwrap();
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let mut service = LawExecutionService::new();
    service.load_law(&register()).unwrap();
    let err = cell.load_standard_reductions(&mut service).unwrap_err();
    assert!(
        err.to_string().contains("postcode_gecorigeerd")
            && err.to_string().contains("no loaded version"),
        "{err}"
    );
}

/// A fictional register in two versions, and a fictional regulation of
/// corrections that takes effect between them, from 2025-06-01.
fn versioned(id: &str, from: &str, articles: &str) -> String {
    format!(
        "---\n$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json\n\
         $id: {id}\nregulatory_layer: WET\npublication_date: '{from}'\nvalid_from: '{from}'\n\
         name: FICTIEF {id}\narticles:\n{articles}"
    )
}

const REGISTERED: &str = "  - number: '1'
    text: FICTIEF. Het register schrijft in waar een bewoner woont.
    machine_readable:
      requires:
        - fact: ingeschreven
          parameters: [bewoner, postcode]
      execution:
        parameters:
          - {name: bewoner, type: string}
          - {name: postcode, type: string}
";

const CORRECTED: &str = "  - number: '1'
    text: FICTIEF. Het register verbetert de postcode, of laat hem leeg.
    machine_readable:
      requires:
        - fact: postcode_gecorrigeerd
          parameters: [postcode]
      execution:
        parameters:
          - {name: postcode, type: string, nullable: true}
";

/// A standard reduction has a version per day a version of one of its
/// regulations takes effect, from the day the root's regulation requires
/// the fact. A referring fact not yet in force has no step in a version;
/// a field the referring fact may leave null may hold null. Loading it
/// twice keeps the one loaded first.
#[test]
fn a_standard_reduction_follows_the_versions_of_its_regulations() {
    let mut service = LawExecutionService::new();
    let laws = [
        versioned(
            "test_standaard_register",
            "2024-01-01",
            "  - number: '9'\n    text: FICTIEF. Nog niets.\n",
        ),
        versioned("test_standaard_register", "2025-01-01", REGISTERED),
        versioned("test_standaard_correcties", "2025-06-01", CORRECTED),
    ];
    for law in &laws {
        service.load_law(law).unwrap();
    }
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        "name: Testcel\nanswers:\n  rijen:\n    fact: test_standaard_register#1#ingeschreven\n    referred_by: [test_standaard_correcties#1#postcode_gecorrigeerd]\n",
    )
    .unwrap();
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    cell.load_standard_reductions(&mut service).unwrap();
    cell.load_standard_reductions(&mut service).unwrap();
    let (_, id) = cell.standard_answers().next().unwrap();
    let aggregate =
        |on: &str| projection_of(&service, &id, regelrecht_cel::standard::AGGREGATE, on);
    assert!(
        service
            .resolver()
            .get_law_for_date(&id, Some(day("2024-06-01")))
            .is_none()
    );
    assert_eq!(aggregate("2025-02-01").steps.len(), 1);
    let corrected = aggregate("2025-07-01");
    assert_eq!(corrected.steps.len(), 2);
    let postcode = corrected
        .state
        .iter()
        .find(|p| p.name == "postcode")
        .unwrap();
    assert_eq!(postcode.nullable, Some(true));
}

/// Two facts that hold a field of the same name with another type cannot
/// share one property of the state, so the standard reduction is refused.
#[test]
fn facts_that_hold_one_name_with_two_types_are_refused() {
    let mut service = LawExecutionService::new();
    service
        .load_law(&versioned(
            "test_standaard_register",
            "2025-01-01",
            REGISTERED,
        ))
        .unwrap();
    let as_number = CORRECTED.replace(
        "{name: postcode, type: string, nullable: true}",
        "{name: postcode, type: number}",
    );
    service
        .load_law(&versioned(
            "test_standaard_correcties",
            "2025-01-01",
            &as_number,
        ))
        .unwrap();
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        "name: Testcel\nanswers:\n  rijen:\n    fact: test_standaard_register#1#ingeschreven\n    referred_by: [test_standaard_correcties#1#postcode_gecorrigeerd]\n",
    )
    .unwrap();
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let err = cell.load_standard_reductions(&mut service).unwrap_err();
    assert!(err.to_string().contains("another type"), "{err}");
}
