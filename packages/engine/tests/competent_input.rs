//! An input whose source names who is competent by law to answer it from
//! its own records (`source.competent_authority`, schema v0.8.14, draft
//! RFC-056). The engine asks the one answerer of the call that the source
//! names (`OpenTermAnswerer::answer_input`). A value the caller supplies
//! still counts; supplied and answered is an error; neither leaves the input
//! as a `source: {}` input without data. The report says which happened
//! (`ArticleResult::competent_inputs`). The type check holds an answering
//! reduction's output to the input (T6, N5), and the loader holds
//! `implements` with `input` to a `REDUCTIE`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use regelrecht_engine::typecheck::{check_law, Rule};
use regelrecht_engine::{
    Answer, AnswerProvenance, AnswerRequest, ArticleBasedLaw, Authority, Call, InputRequest,
    InputResolvedBy, LawExecutionService, LawLoad, OpenTermAnswerer, Value,
};

const DATE: &str = "2026-10-20";

/// Art. 1 reads whether the inspecteur granted a permit, keyed on a value
/// another input of the article gives; art. 2 reads a nullable one.
const LAW: &str = r#"
$id: wet_vergunning_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/wet
articles:
  - number: '1'
    text: Een plaats mag alleen gebruikt worden met een vergunning van de inspecteur.
    machine_readable:
      execution:
        parameters:
          - {name: kvk_nummer, type: string}
        input:
          - name: vestiging
            type: string
            source: {regulation: wet_vergunning_fictief, output: vestiging_uit, parameters: {kvk_nummer: $kvk_nummer}}
          - name: heeft_vergunning
            type: boolean
            source:
              competent_authority: {name: inspecteur, type: CATEGORY}
              parameters: {vestiging: $vestiging}
        output:
          - {name: mag_gebruiken, type: boolean}
        actions:
          - {output: mag_gebruiken, value: $heeft_vergunning}
  - number: '2'
    text: Tekst.
    machine_readable:
      execution:
        input:
          - name: misschien
            type: boolean
            nullable: true
            source:
              competent_authority: {name: inspecteur, type: CATEGORY}
        output:
          - {name: misschien_uit, type: boolean, nullable: true}
        actions:
          - {output: misschien_uit, value: $misschien}
  - number: '3'
    text: Tekst.
    machine_readable:
      execution:
        parameters:
          - {name: kvk_nummer, type: string}
        output:
          - {name: vestiging_uit, type: string}
        actions:
          - {output: vestiging_uit, value: 'vestiging-1'}
"#;

/// An answerer that answers inputs by name, and nothing else.
struct Fake {
    authority: Authority,
    inputs: BTreeMap<String, Value>,
}

impl Fake {
    fn inspecteur(name: &str, inputs: &[(&str, Value)]) -> Self {
        Self {
            authority: Authority {
                name: name.to_string(),
                category: Some("inspecteur".to_string()),
            },
            inputs: inputs
                .iter()
                .map(|(n, v)| (n.to_string(), v.clone()))
                .collect(),
        }
    }
}

impl OpenTermAnswerer for Fake {
    fn authority(&self) -> &Authority {
        &self.authority
    }

    fn answer(
        &self,
        _service: &LawExecutionService,
        _request: &AnswerRequest<'_>,
    ) -> regelrecht_engine::Result<Option<Answer>> {
        Ok(None)
    }

    fn answer_input(
        &self,
        _service: &LawExecutionService,
        request: &InputRequest<'_>,
    ) -> regelrecht_engine::Result<Option<Answer>> {
        assert_eq!(request.law, "wet_vergunning_fictief");
        assert_eq!(request.calculation_date, DATE);
        if request.input == "heeft_vergunning" {
            // The source's parameter, read from another input of the article.
            assert_eq!(
                request.parameters.get("vestiging"),
                Some(&Value::String("vestiging-1".to_string()))
            );
        }
        Ok(self.inputs.get(request.input).map(|value| Answer {
            value: value.clone(),
            provenance: AnswerProvenance {
                answerer: self.authority.name.clone(),
                answer: request.input.to_string(),
                law_id: "reductie_fictief".to_string(),
                article: "1".to_string(),
                valid_from: "2026-01-01".to_string(),
                records_read: 0,
                last_record: None,
            },
        }))
    }
}

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(LAW).unwrap();
    service
}

fn parameters() -> BTreeMap<String, Value> {
    BTreeMap::from([("kvk_nummer".to_string(), Value::String("1".to_string()))])
}

fn evaluate(
    service: &LawExecutionService,
    output: &str,
    parameters: BTreeMap<String, Value>,
    cells: Vec<&dyn OpenTermAnswerer>,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let call = Call {
        sphere: cells,
        ..Call::default()
    };
    service.evaluate_law_in("wet_vergunning_fictief", &[output], parameters, call, DATE)
}

fn resolved_by<'r>(
    result: &'r regelrecht_engine::ArticleResult,
    input: &str,
) -> &'r InputResolvedBy {
    let entries: Vec<_> = result
        .competent_inputs
        .iter()
        .filter(|c| c.input == input)
        .collect();
    let [entry] = entries.as_slice() else {
        panic!("one entry for {input}: {:?}", result.competent_inputs)
    };
    &entry.resolved_by
}

fn supply(service: &mut LawExecutionService, value: bool) {
    let record = BTreeMap::from([
        ("kvk_nummer".to_string(), Value::String("1".to_string())),
        ("heeft_vergunning".to_string(), Value::Bool(value)),
    ]);
    service
        .register_dict_source_for_law(
            "wet_vergunning_fictief",
            "DOUANE",
            "kvk_nummer",
            vec![record],
            10,
        )
        .unwrap();
}

#[test]
fn the_competent_cell_answers() {
    let service = service();
    let cell = Fake::inspecteur("inspecteur_a", &[("heeft_vergunning", Value::Bool(true))]);
    let result = evaluate(&service, "mag_gebruiken", parameters(), vec![&cell]).unwrap();
    assert_eq!(result.outputs["mag_gebruiken"], Value::Bool(true));
    let InputResolvedBy::Answered(provenance) = resolved_by(&result, "heeft_vergunning") else {
        panic!("{:?}", result.competent_inputs)
    };
    assert_eq!(provenance.answerer, "inspecteur_a");
    // The receipt says who answered, without a trace.
    let receipt = service.build_receipt_with_outputs(&result, &parameters(), DATE, &[]);
    assert_eq!(receipt.results.competent_inputs, result.competent_inputs);
}

#[test]
fn a_supplied_value_counts_without_a_cell() {
    let mut service = service();
    supply(&mut service, false);
    let result = evaluate(&service, "mag_gebruiken", parameters(), Vec::new()).unwrap();
    assert_eq!(result.outputs["mag_gebruiken"], Value::Bool(false));
    assert_eq!(
        resolved_by(&result, "heeft_vergunning"),
        &InputResolvedBy::Supplied {
            provider: Some("DOUANE".to_string())
        }
    );
}

#[test]
fn supplied_and_answered_is_an_error() {
    let mut service = service();
    supply(&mut service, false);
    let cell = Fake::inspecteur("inspecteur_a", &[("heeft_vergunning", Value::Bool(true))]);
    let e = evaluate(&service, "mag_gebruiken", parameters(), vec![&cell]).unwrap_err();
    assert!(
        e.to_string()
            .contains("is supplied by DOUANE and answered by the cell inspecteur_a"),
        "{e}"
    );
}

#[test]
fn a_passed_value_counts_and_passed_and_answered_is_an_error() {
    let service = service();
    let mut passed = parameters();
    passed.insert("heeft_vergunning".to_string(), Value::Bool(true));
    let result = evaluate(&service, "mag_gebruiken", passed.clone(), Vec::new()).unwrap();
    assert_eq!(result.outputs["mag_gebruiken"], Value::Bool(true));
    assert_eq!(
        resolved_by(&result, "heeft_vergunning"),
        &InputResolvedBy::Supplied { provider: None }
    );
    let cell = Fake::inspecteur("inspecteur_a", &[("heeft_vergunning", Value::Bool(false))]);
    let e = evaluate(&service, "mag_gebruiken", passed, vec![&cell]).unwrap_err();
    assert!(
        e.to_string()
            .contains("is passed by the caller and answered by the cell inspecteur_a"),
        "{e}"
    );
}

#[test]
fn neither_leaves_the_input_unknown() {
    let service = service();
    let silent = Fake::inspecteur("inspecteur_a", &[]);
    let result = evaluate(&service, "mag_gebruiken", parameters(), vec![&silent]).unwrap();
    assert!(result.outputs["mag_gebruiken"].is_unknown());
    assert_eq!(
        resolved_by(&result, "heeft_vergunning"),
        &InputResolvedBy::Absent
    );
}

#[test]
fn two_competent_cells_are_an_error_and_another_authority_is_not_asked() {
    let service = service();
    let a = Fake::inspecteur("inspecteur_a", &[("heeft_vergunning", Value::Bool(true))]);
    let b = Fake::inspecteur("inspecteur_b", &[("heeft_vergunning", Value::Bool(true))]);
    let e = evaluate(&service, "mag_gebruiken", parameters(), vec![&a, &b]).unwrap_err();
    assert!(
        e.to_string()
            .contains("2 cells of the call are named by it: inspecteur_a, inspecteur_b"),
        "{e}"
    );
    let college = Fake {
        authority: Authority {
            name: "college".to_string(),
            category: Some("college".to_string()),
        },
        inputs: BTreeMap::from([("heeft_vergunning".to_string(), Value::Bool(true))]),
    };
    let result = evaluate(&service, "mag_gebruiken", parameters(), vec![&college]).unwrap();
    assert_eq!(
        resolved_by(&result, "heeft_vergunning"),
        &InputResolvedBy::Absent
    );
}

#[test]
fn a_null_answer_fits_only_a_nullable_input() {
    let service = service();
    let null = Fake::inspecteur(
        "inspecteur_a",
        &[
            ("heeft_vergunning", Value::Null),
            ("misschien", Value::Null),
        ],
    );
    let e = evaluate(&service, "mag_gebruiken", parameters(), vec![&null]).unwrap_err();
    assert!(
        matches!(
            &e,
            regelrecht_engine::EngineError::NullForNonNullable { field, origin, .. }
                if field == "heeft_vergunning" && origin == "the cell inspecteur_a"
        ),
        "{e}"
    );
    let result = evaluate(&service, "misschien_uit", BTreeMap::new(), vec![&null]).unwrap();
    assert_eq!(result.outputs["misschien_uit"], Value::Null);
}

/// A reduction answering art. 1's input with the output `output`.
fn reduction(output: &str) -> ArticleBasedLaw {
    ArticleBasedLaw::from_yaml_str(&format!(
        r#"
$id: reductie_fictief
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Geen regelgeving.
    machine_readable:
      implements:
        - {{law: wet_vergunning_fictief, article: '1', input: heeft_vergunning}}
      execution:
        output:
          - {output}
        actions:
          - {{output: heeft_vergunning, value: true}}
"#
    ))
    .unwrap()
}

/// The findings of the type check on `reduction`, which may consult `law`.
fn findings(reduction: &ArticleBasedLaw, law: &ArticleBasedLaw) -> Vec<(Rule, String)> {
    check_law(reduction, &|id| (id == law.id).then_some(law))
        .into_iter()
        .map(|f| (f.rule, f.message))
        .collect()
}

#[test]
fn the_type_check_holds_the_answer_to_the_input() {
    let law = ArticleBasedLaw::from_yaml_str(LAW).unwrap();
    let rules = |output: &str| findings(&reduction(output), &law);
    assert_eq!(rules("{name: heeft_vergunning, type: boolean}"), Vec::new());
    let found = rules("{name: heeft_vergunning, type: number}");
    assert!(
        found
            .iter()
            .any(|(rule, m)| *rule == Rule::T6
                && m.contains("which is boolean, but is declared number")),
        "{found:?}"
    );
    let found = rules("{name: heeft_vergunning, type: boolean, type_spec: {unit: eurocent}}");
    assert!(
        matches!(found.as_slice(), [(Rule::T6, m)] if m.contains("whose unit is no unit, but has eurocent")),
        "{found:?}"
    );
    let found = rules("{name: heeft_vergunning, type: boolean, nullable: true}");
    assert!(
        matches!(found.as_slice(), [(Rule::N5, m)] if m.contains("which is never absent")),
        "{found:?}"
    );
    // An output of another name answers nothing.
    let found = rules("{name: iets_anders, type: boolean}");
    assert!(
        found
            .iter()
            .any(|(rule, m)| *rule == Rule::T6
                && m.contains("declares no output 'heeft_vergunning'")),
        "{found:?}"
    );
    // A nullable input takes a nullable answer.
    let nullable = ArticleBasedLaw::from_yaml_str(
        r#"
$id: reductie_fictief
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Geen regelgeving.
    machine_readable:
      implements:
        - {law: wet_vergunning_fictief, article: '2', input: misschien}
      execution:
        output:
          - {name: misschien, type: boolean, nullable: true}
        actions:
          - {output: misschien, value: null}
"#,
    )
    .unwrap();
    assert_eq!(findings(&nullable, &law), Vec::new());
}

#[test]
fn the_loader_refuses_an_implements_entry_with_both_or_neither() {
    // In a WET, so the refusal of a term filled by a reduction does not
    // come first.
    let entry = |layer: &str, fields: &str| {
        format!(
            r#"
$id: reductie_fictief
regulatory_layer: {layer}
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Geen regelgeving.
    machine_readable:
      implements:
        - {{law: wet_vergunning_fictief, article: '1'{fields}}}
      execution:
        output:
          - {{name: heeft_vergunning, type: boolean}}
        actions:
          - {{output: heeft_vergunning, value: true}}
"#
        )
    };
    for (fields, says) in [
        (
            ", open_term: heeft_vergunning, input: heeft_vergunning",
            "implements both the open term 'heeft_vergunning' and the input 'heeft_vergunning'",
        ),
        ("", "without naming an open term or an input"),
    ] {
        let e = ArticleBasedLaw::from_yaml_str(&entry("WET", fields)).unwrap_err();
        assert!(e.to_string().contains(says), "{e}");
    }
    ArticleBasedLaw::from_yaml_str(&entry("REDUCTIE", ", input: heeft_vergunning")).unwrap();
}
