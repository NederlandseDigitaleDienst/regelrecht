//! An answer a cell gives from a fact, without a reduction written for it
//! (`cel.yaml`, `answers`, [`regelrecht_cel::standard`]): the standard
//! reduction is the written-out copy reduction, structurally and in what it
//! answers, also after a revocation and on a day before a record. A record
//! of another fact that refers to a root and has no effect, such as a
//! correction, leaves its row as it is.

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
/// field of the fact, and a read model that gives the aggregates of the
/// resident as rows.
const BY_HAND: &str = r"---
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json
$id: test_reductie_inschrijvingen
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
name: Inschrijvingen van een bewoner (testreductie)
articles:
  - number: inschrijving
    text: FICTIEF. Per inschrijving de velden.
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

/// Three registrations, two of resident `b1`: a correction of the postcode
/// refers to the first, and the second is revoked as wrong from the start.
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

/// The standard reduction answers what the reduction written out by hand
/// answers: before any record, between records, and after a fact that
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
        vec![s("1111AA"), Value::Null]
    );
}

/// A record of another fact that refers to a root, such as a correction of
/// its postcode, leaves the row of the standard reduction as it is: what
/// such a record does with the row is a reading, written out in a reduction.
#[test]
fn a_record_of_another_fact_that_refers_to_the_root_leaves_its_row() {
    let (mut cell, service, _dir) = cell_and_service();
    record_registrations(&mut cell, &service);
    let before = answer(&cell, &service, "inschrijvingen", "b1", "2025-03-15");
    let after = answer(&cell, &service, "inschrijvingen", "b1", "2025-04-15");
    assert_eq!(after, before);
    let s = |p: &str| Value::String(p.to_string());
    assert_eq!(postcodes(&after), vec![s("1111AA"), s("2222BB")]);
}

/// An answer that selects on a field the fact does not hold, one that
/// selects on a field twice, one named like the aggregate article, and one
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
            "  rijen:\n    fact: test_chronicle_corrections#1#ingeschreven\n    where: [bewoner, bewoner]\n",
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
/// the answer they belong to.
#[test]
fn a_malformed_answer_from_a_fact_is_refused_by_name() {
    let dir = cell_tempdir();
    for (answers, says) in [
        (
            "  rijen:\n    fact: test_chronicle_corrections#1#ingeschreven\n    wehre: [bewoner]\n",
            "wehre",
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
}

/// A fictional regulation `id`, valid from `from`, with `articles`.
fn versioned(id: &str, from: &str, articles: &str) -> String {
    format!(
        "---\n$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json\n\
         $id: {id}\nregulatory_layer: WET\npublication_date: '{from}'\nvalid_from: '{from}'\n\
         name: FICTIEF {id}\narticles:\n{articles}"
    )
}

const REGISTERED: &str = "  - number: '1'
    text: FICTIEF. Het register schrijft in waar een bewoner woont, en welk kenteken hij opgaf.
    machine_readable:
      requires:
        - fact: ingeschreven
          parameters: [bewoner, kenteken]
      execution:
        parameters:
          - {name: bewoner, type: string}
          - {name: kenteken, type: string, nullable: true}
";

/// A cell of `ACTOR` whose catalogue holds `answers`, and a service with
/// the versions `laws` and the cell's standard reductions.
fn cell_on(answers: &str, laws: &[String]) -> (Cell, LawExecutionService, tempfile::TempDir) {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        format!("name: Testcel\nanswers:\n{answers}"),
    )
    .unwrap();
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    cell.load_standard_reductions(&mut service).unwrap();
    (cell, service, dir)
}

/// A standard reduction has a version per version of the fact's regulation
/// that requires the fact, and none before. Loading it twice keeps the one
/// loaded first. An answer name may hold digits and _.
#[test]
fn a_standard_reduction_follows_the_versions_of_its_regulation() {
    let laws = [
        versioned(
            "test_standaard_register",
            "2024-01-01",
            "  - number: '9'\n    text: FICTIEF. Nog niets.\n",
        ),
        versioned("test_standaard_register", "2025-01-01", REGISTERED),
    ];
    let (cell, mut service, _dir) =
        cell_on("  rijen_2: test_standaard_register#1#ingeschreven\n", &laws);
    cell.load_standard_reductions(&mut service).unwrap();
    let (_, id) = cell.standard_answers().next().unwrap();
    assert!(
        service
            .resolver()
            .get_law_for_date(&id, Some(day("2024-06-01")))
            .is_none()
    );
    let aggregate = projection_of(
        &service,
        &id,
        regelrecht_cel::standard::AGGREGATE,
        "2025-02-01",
    );
    assert_eq!(aggregate.steps.len(), 1);
    assert_eq!(aggregate.state.len(), 2);
}

/// The rows of the standard answer `answer` of `cell` on `on`, with
/// `parameters`.
fn rows(
    cell: &Cell,
    service: &LawExecutionService,
    answer: &str,
    parameters: &BTreeMap<String, Value>,
    on: &str,
) -> Vec<Value> {
    let reader = Reader::new(
        cell.recording_actor(),
        cell.authority(),
        cell.answers(),
        cell.reductions(),
        cell.grams().to_vec(),
        None,
    );
    let request = AnswerRequest {
        name: answer,
        parameters,
        calculation_date: on,
        trace: false,
    };
    match reader.answer(service, &request).unwrap() {
        Some(a) => match a.value {
            Value::Array(rows) => rows,
            other => panic!("no rows: {other:?}"),
        },
        None => panic!("the cell gave no answer"),
    }
}

/// A fact without parameters never reaches a standard reduction: the
/// service refuses the regulation that requires it (RFC-051). The test
/// guards that assumption, which an empty state would otherwise meet.
#[test]
fn a_fact_without_parameters_is_refused_before_a_standard_reduction() {
    let without = REGISTERED.replace(
        "          parameters: [bewoner, kenteken]\n",
        "          parameters: [bewoner, kenteken]\n        - fact: gemeld\n",
    );
    let err = LawExecutionService::new()
        .load_law(&versioned(
            "test_standaard_register",
            "2025-01-01",
            &without,
        ))
        .unwrap_err();
    assert!(err.to_string().contains("holds no parameters"), "{err}");
}

/// `where` on a field the fact may leave null makes it a nullable
/// parameter. A binding that passes null selects the roots that hold null
/// there, as `where` compares with EQUALS.
#[test]
fn where_on_a_nullable_field_with_null_selects_the_roots_that_hold_null() {
    let laws = [versioned(
        "test_standaard_register",
        "2025-01-01",
        REGISTERED,
    )];
    let (mut cell, service, _dir) = cell_on(
        "  op_kenteken:\n    fact: test_standaard_register#1#ingeschreven\n    where: [kenteken]\n",
        &laws,
    );
    for (bewoner, kenteken) in [("b1", json!("AB-12-CD")), ("b2", serde_json::Value::Null)] {
        cell.record_executogram(
            &service,
            "test_standaard_register#1#ingeschreven",
            &fields(&[("bewoner", json!(bewoner)), ("kenteken", kenteken)]),
            day("2025-02-01"),
        )
        .unwrap();
    }
    let bewoners = |kenteken: Value| -> Vec<Value> {
        let parameters = [("kenteken".to_string(), kenteken)].into();
        rows(&cell, &service, "op_kenteken", &parameters, "2025-03-01")
            .into_iter()
            .map(|row| match row {
                Value::Object(row) => row.get("bewoner").cloned().unwrap_or(Value::Null),
                other => panic!("no row: {other:?}"),
            })
            .collect()
    };
    let s = |p: &str| Value::String(p.to_string());
    assert_eq!(bewoners(s("AB-12-CD")), vec![s("b1")]);
    assert_eq!(bewoners(Value::Null), vec![s("b2")]);
}
