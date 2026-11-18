//! A fact an article requires (schema v0.8.7, RFC-051): `requires` with
//! `fact` and the parameters it holds, and `source.facts` that names it as
//! `<regulation>#<article>#<fact>`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_law_model::{ArticleBasedLaw, Source, is_fact_reference};

#[test]
fn a_fact_reference_names_an_article_and_a_fact() {
    assert!(is_fact_reference("regeling_x#3#geneeskundig_onderzoek"));
    assert!(is_fact_reference(
        "algemene_wet_bestuursrecht#7:11#ontvangst"
    ));
    // An article alone, a fact name that is no name, and no article.
    assert!(!is_fact_reference("regeling_x#3"));
    assert!(!is_fact_reference("regeling_x#3#Onderzoek"));
    assert!(!is_fact_reference("regeling_x#3#onderzoek-1"));
    assert!(!is_fact_reference("regeling_x#3#"));
    assert!(!is_fact_reference("regeling_x##onderzoek"));
    assert!(!is_fact_reference("onderzoek"));
}

#[test]
fn source_facts_takes_only_a_fact_reference() {
    let source: Source = serde_yaml_ng::from_str("facts: regeling_x#3#onderzoek").unwrap();
    assert_eq!(source.facts.as_deref(), Some("regeling_x#3#onderzoek"));
    let e = serde_yaml_ng::from_str::<Source>("facts: regeling_x#3").unwrap_err();
    assert!(
        e.to_string().contains("<regulation>#<article>#<fact>"),
        "{e}"
    );
}

#[test]
fn an_article_requires_a_fact_with_its_parameters() {
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(
        r#"
$id: regeling_x
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2013-01-01'
articles:
  - number: '3'
    text: Het onderzoek wordt verricht door de GGD.
    machine_readable:
      requires:
        - law: andere_wet
          values: [iets]
        - fact: onderzoek
          description: Het onderzoek.
          parameters: [bsn, datum]
      execution:
        parameters:
          - {name: bsn, type: string}
          - {name: datum, type: date}
"#,
    )
    .unwrap();
    let article = &law.articles[0];
    let fact = article.required_fact("onderzoek").unwrap();
    assert_eq!(
        fact.parameters.as_deref(),
        Some(&["bsn".to_string(), "datum".to_string()][..])
    );
    assert_eq!(fact.description.as_deref(), Some("Het onderzoek."));
    // A dependency on another provision is no fact.
    assert!(article.required_fact("iets").is_none());
    assert!(article.required_fact("andere_wet").is_none());
}
