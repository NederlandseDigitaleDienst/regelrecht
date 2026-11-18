//! Shapes where the model used to be narrower than the schema.
//!
//! The conformance suite (`packages/engine/tests/conformance.rs`) is the
//! systematic gate; these are the close-up regression tests for the individual
//! shapes that were wrong, each one a document the schema accepts and the model
//! refused or lost.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_law_model::{ArticleBasedLaw, CompetentAuthority};

fn parse(yaml: &str) -> ArticleBasedLaw {
    serde_yaml_ng::from_str(yaml).expect("model should parse a schema-valid law")
}

/// `requires` is a list of objects, not a list of strings. A law written the way
/// the schema allows made the engine fail to load with "invalid type: map,
/// expected a string".
#[test]
fn requires_is_a_list_of_dependency_objects() {
    let law = parse(
        r#"
$id: test_requires
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een artikel met afhankelijkheden.
    machine_readable:
      requires:
        - article: '2'
        - law: hogere_wet
          values: [drempelbedrag, standaardpremie]
        - regeling: Testregeling
        - koninklijk_besluit: Testbesluit
"#,
    );

    let requires = law.articles[0].get_requires();
    assert_eq!(requires.len(), 4);
    assert_eq!(requires[0].article.as_deref(), Some("2"));
    assert_eq!(requires[1].law.as_deref(), Some("hogere_wet"));
    assert_eq!(
        requires[1].values.as_deref(),
        Some(["drempelbedrag".to_string(), "standaardpremie".to_string()].as_slice())
    );
    assert_eq!(requires[2].regeling.as_deref(), Some("Testregeling"));
    assert_eq!(
        requires[3].koninklijk_besluit.as_deref(),
        Some("Testbesluit")
    );
}

/// The schema does not require `$id`, so the model may not either. It used to
/// fail with "missing field `$id`" on a document the schema accepts.
#[test]
fn a_law_without_an_id_still_parses() {
    let law = parse(
        r#"
regulatory_layer: WET
publication_date: '2025-01-01'
articles: []
"#,
    );
    assert_eq!(law.id, "");
}

/// The aanhef carries the "Gelet op"-chain and is in use in the corpus; the
/// model dropped it entirely.
#[test]
fn the_preamble_survives_a_round_trip() {
    let law = parse(
        r#"
$id: test_preamble
regulatory_layer: WET
publication_date: '2025-01-01'
preamble:
  text: Gelet op artikel 4 van de hogere wet;
  url: https://example.invalid/law/aanhef
  machine_readable:
    endpoint: aanhef
articles: []
"#,
    );
    let preamble = law.preamble.as_ref().expect("preamble is parsed");
    assert_eq!(preamble.text, "Gelet op artikel 4 van de hogere wet;");
    assert_eq!(
        preamble
            .machine_readable
            .as_ref()
            .and_then(|mr| mr.endpoint.as_deref()),
        Some("aanhef")
    );
}

/// `CATEGORY` means the authority must be resolved per context. The model kept
/// only the name, which silently turned every category into an instance.
#[test]
fn a_category_authority_keeps_its_type() {
    let law = parse(
        r#"
$id: test_authority
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bevoegd gezag beslist.
    machine_readable:
      competent_authority:
        name: het college
        type: CATEGORY
"#,
    );
    let authority = law.articles[0]
        .machine_readable
        .as_ref()
        .and_then(|mr| mr.competent_authority.as_ref())
        .expect("authority is parsed");
    match authority {
        CompetentAuthority::Structured(regelrecht_law_model::NamedAuthority {
            name,
            authority_type,
        }) => {
            assert_eq!(name, "het college");
            assert_eq!(
                *authority_type,
                Some(regelrecht_law_model::AuthorityType::Category)
            );
        }
        other => panic!("expected a structured authority, got {other:?}"),
    }
}

/// `type_spec.min`/`max` are JSON numbers in the schema. `Decimal` serializes to
/// a string by default, which made the re-serialized document schema-invalid.
#[test]
fn numeric_bounds_serialize_as_numbers() {
    let law = parse(
        r#"
$id: test_bounds
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een artikel met grenzen.
    machine_readable:
      execution:
        input:
          - name: INKOMEN
            type: amount
            type_spec:
              min: 0
              max: 1000000
"#,
    );
    let json = serde_json::to_value(&law).expect("serialize");
    let bounds = &json["articles"][0]["machine_readable"]["execution"]["input"][0]["type_spec"];
    assert!(bounds["min"].is_number(), "min: {}", bounds["min"]);
    assert!(bounds["max"].is_number(), "max: {}", bounds["max"]);
}
