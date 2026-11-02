//! An application names the decision it asks for with `requests` (schema
//! v0.8.1, RFC-045). The engine follows the reference both ways: from an
//! application to its decision, and from a decision to the applications that
//! ask for it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::LawExecutionService;

/// Article 1 establishes the application and decides on it (list form).
/// Article 2 decides in the object form, establishing its own application.
/// Article 3 establishes an application without a decision (object form).
/// Article 4 decides, and nothing asks for its decision.
const DECIDING_LAW: &str = r#"
$id: wet_verlof
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent op aanvraag verlof.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof#1
            parameters: [dagen]
          - type: BESCHIKKING
            outputs: [verleend]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: verleend, type: boolean}
        actions:
          - {output: verleend, value: true}
  - number: '2'
    text: Het hoofd verleent op aanvraag vrijstelling.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          submission: {kind: AANVRAAG}
        output:
          - {name: vrijgesteld, type: boolean}
        actions:
          - {output: vrijgesteld, value: true}
  - number: '3'
    text: Een melding wordt gedaan bij het hoofd.
    machine_readable:
      execution:
        produces:
          submission: {kind: AANVRAAG}
        output:
          - {name: gemeld, type: boolean}
        actions:
          - {output: gemeld, value: true}
  - number: '4'
    text: Het hoofd stelt het rooster vast.
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
        output:
          - {name: rooster, type: boolean}
        actions:
          - {output: rooster, value: true}
"#;

/// Another law whose article 1 asks for the decision of wet_verlof
/// article 1, and whose article 2 is an object-form decision with its own
/// application, numbered like the one in wet_verlof.
const ASKING_LAW: &str = r#"
$id: wet_aanvraag
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Verlof wordt bij het hoofd aangevraagd.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof#1
            outputs: [aangevraagd]
        output:
          - {name: aangevraagd, type: boolean}
        actions:
          - {output: aangevraagd, value: true}
  - number: '2'
    text: Het hoofd verleent op aanvraag vrijstelling.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          submission: {kind: AANVRAAG}
        output:
          - {name: vrijgesteld, type: boolean}
        actions:
          - {output: vrijgesteld, value: true}
"#;

const DAY: &str = "2026-10-14";

fn service(laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
}

#[test]
fn an_application_names_the_decision_it_requests() {
    let service = service(&[DECIDING_LAW, ASKING_LAW]);
    let requested =
        |law: &str, article: &str| service.requested_decision(law, article, DAY).unwrap();
    assert_eq!(
        requested("wet_verlof", "1").as_deref(),
        Some("wet_verlof#1")
    );
    assert_eq!(
        requested("wet_aanvraag", "1").as_deref(),
        Some("wet_verlof#1")
    );
    // The object form with a decision asks for its own.
    assert_eq!(
        requested("wet_verlof", "2").as_deref(),
        Some("wet_verlof#2")
    );
    // An application without a decision, and a decision without an
    // application, request nothing.
    assert_eq!(requested("wet_verlof", "3"), None);
    assert_eq!(requested("wet_verlof", "4"), None);
}

#[test]
fn a_decision_finds_the_applications_that_request_it() {
    let service = service(&[DECIDING_LAW, ASKING_LAW]);
    let applications = |article: &str| {
        service
            .applications_for("wet_verlof", article, DAY)
            .unwrap()
    };
    // The deciding law first, then the others.
    assert_eq!(applications("1"), ["wet_verlof#1", "wet_aanvraag#1"]);
    assert_eq!(applications("2"), ["wet_verlof#2"]);
    assert!(applications("3").is_empty());
    assert!(applications("4").is_empty());
}

#[test]
fn a_request_for_a_decision_that_is_not_there_is_an_error_when_used() {
    let elsewhere = ASKING_LAW.replace("requests: wet_verlof#1", "requests: wet_elders#7");
    let service = service(&[DECIDING_LAW, &elsewhere]);
    let e = service
        .requested_decision("wet_aanvraag", "1", DAY)
        .unwrap_err()
        .to_string();
    assert!(e.contains("requests the decision of wet_elders#7"), "{e}");

    let no_decision = ASKING_LAW.replace("requests: wet_verlof#1", "requests: wet_verlof#3");
    let service = self::service(&[DECIDING_LAW, &no_decision]);
    let e = service
        .requested_decision("wet_aanvraag", "1", DAY)
        .unwrap_err()
        .to_string();
    assert!(e.contains("which takes no decision"), "{e}");
}
