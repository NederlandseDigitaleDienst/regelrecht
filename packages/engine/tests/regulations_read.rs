//! An execution reports the regulation versions an article of which ran
//! (`ArticleResult::regulations_read`), without tracing: a record that keeps
//! them refers to the law it rests on without copying it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use regelrecht_engine::{LawExecutionService, RegulationRead, Value};

/// The register, in two versions with a different age.
fn register(valid_from: &str, age: i64) -> String {
    format!(
        r#"
$id: read_register
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '{valid_from}'
articles:
  - number: '1'
    text: De registratie kent de leeftijd.
    machine_readable:
      execution:
        output:
          - name: leeftijd
            type: number
        actions:
          - output: leeftijd
            value: {age}
"#
    )
}

const TOESLAG: &str = r#"
$id: read_toeslag
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Aanspraak bestaat voor een meerderjarige.
    machine_readable:
      execution:
        input:
          - name: leeftijd
            type: number
            source:
              regulation: read_register
              output: leeftijd
        output:
          - name: voldoet
            type: boolean
        actions:
          - output: voldoet
            value:
              operation: GREATER_THAN_OR_EQUAL
              subject: $leeftijd
              value: 18
"#;

const UNREAD: &str = r#"
$id: read_unread
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Niemand leest dit.
    machine_readable:
      execution:
        output:
          - name: niets
            type: number
        actions:
          - output: niets
            value: 0
"#;

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in [
        register("2025-01-01", 17),
        register("2026-01-01", 18),
        TOESLAG.to_string(),
        UNREAD.to_string(),
    ] {
        service.load_law(&law).unwrap();
    }
    service
}

fn read(service: &LawExecutionService, date: &str) -> Vec<RegulationRead> {
    service
        .evaluate_law("read_toeslag", &["voldoet"], BTreeMap::new(), date)
        .unwrap()
        .regulations_read
}

#[test]
fn an_execution_reports_each_version_it_ran_once_in_order() {
    let service = service();
    let read = read(&service, "2025-06-01");
    let ids: Vec<(&str, Option<&str>)> = read
        .iter()
        .map(|r| (r.law_id.as_str(), r.valid_from.as_deref()))
        .collect();
    assert_eq!(
        ids,
        [
            ("read_toeslag", Some("2025-01-01")),
            ("read_register", Some("2025-01-01"))
        ]
    );
    assert!(
        read.iter()
            .all(|r| r.hash.as_deref().is_some_and(|h| h.starts_with("sha256:")))
    );
}

#[test]
fn an_execution_on_another_day_reports_the_version_in_force_then() {
    let service = service();
    let before = read(&service, "2025-06-01");
    let after = read(&service, "2026-06-01");
    assert_eq!(after[1].valid_from.as_deref(), Some("2026-01-01"));
    // Another version is other content.
    assert_ne!(after[1].hash, before[1].hash);
    // The same law that ran is the same version and content.
    assert_eq!(after[0], before[0]);
}

#[test]
fn a_version_is_what_the_engine_loaded() {
    let service = service();
    let law = service
        .get_law_version("read_register", Some("2026-01-01"))
        .unwrap();
    let read = RegulationRead::of(law);
    assert_eq!(read.law_id, "read_register");
    assert_eq!(read.valid_from.as_deref(), Some("2026-01-01"));
    assert_eq!(read.hash, law.content_hash);
    let result = service
        .evaluate_law("read_unread", &["niets"], BTreeMap::new(), "2025-06-01")
        .unwrap();
    assert_eq!(result.outputs["niets"], Value::Int(0));
    assert_eq!(result.regulations_read.len(), 1);
}
