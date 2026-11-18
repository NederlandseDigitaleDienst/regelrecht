// Allowed crate-wide: benchmark setup outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{Criterion, criterion_group, criterion_main};
use regelrecht_engine::{ArticleBasedLaw, ArticleEngine, LawLoad, Value};
use std::collections::BTreeMap;
use std::hint::black_box;

const SIMPLE_LAW_YAML: &str = r#"
$id: bench_simple
name: Benchmark simple law
regulatory_layer: WET
publication_date: "2025-01-01"
valid_from: "2025-01-01"
articles:
  - number: "1"
    text: "Benchmark article"
    machine_readable:
      execution:
        parameters:
          - name: inkomen
            type: number
          - name: drempel
            type: number
        outputs:
          - name: boven_drempel
            type: boolean
          - name: verschil
            type: number
        definitions:
          factor:
            value: 1345
        actions:
          - output: boven_drempel
            value:
              operation: GREATER_THAN
              subject: "$inkomen"
              value: "$drempel"
          - output: verschil
            value:
              operation: IF
              when:
                operation: GREATER_THAN
                subject: "$inkomen"
                value: "$drempel"
              then:
                operation: SUBTRACT
                values:
                  - "$inkomen"
                  - "$drempel"
              else: 0
"#;

const ARITHMETIC_LAW_YAML: &str = r#"
$id: bench_arithmetic
name: Benchmark arithmetic law
regulatory_layer: WET
publication_date: "2025-01-01"
valid_from: "2025-01-01"
articles:
  - number: "1"
    text: "Benchmark article"
    machine_readable:
      execution:
        parameters:
          - name: a
            type: number
          - name: b
            type: number
          - name: c
            type: number
        outputs:
          - name: sum
            type: number
          - name: product
            type: number
          - name: result
            type: number
        actions:
          - output: sum
            value:
              operation: ADD
              values:
                - "$a"
                - "$b"
                - "$c"
          - output: product
            value:
              operation: MULTIPLY
              values:
                - "$a"
                - "$b"
          - output: result
            value:
              operation: SUBTRACT
              values:
                - operation: MULTIPLY
                  values:
                    - "$a"
                    - "$b"
                - operation: ADD
                  values:
                    - "$b"
                    - "$c"
"#;

fn bench_article_evaluate(c: &mut Criterion) {
    let simple_law = ArticleBasedLaw::from_yaml_str(SIMPLE_LAW_YAML).unwrap();
    let arithmetic_law = ArticleBasedLaw::from_yaml_str(ARITHMETIC_LAW_YAML).unwrap();

    let mut group = c.benchmark_group("article_evaluation");

    // Simple: one comparison + one conditional
    group.bench_function("simple_conditional", |b| {
        let article = &simple_law.articles[0];
        let engine = ArticleEngine::new(article, &simple_law);
        let mut params = BTreeMap::new();
        params.insert("inkomen".to_string(), Value::Int(35000));
        params.insert("drempel".to_string(), Value::Int(25000));
        b.iter(|| engine.evaluate(black_box(params.clone()), "2025-01-01"))
    });

    // Arithmetic: multiple nested operations
    group.bench_function("nested_arithmetic", |b| {
        let article = &arithmetic_law.articles[0];
        let engine = ArticleEngine::new(article, &arithmetic_law);
        let mut params = BTreeMap::new();
        params.insert("a".to_string(), Value::Int(100));
        params.insert("b".to_string(), Value::Int(200));
        params.insert("c".to_string(), Value::Int(300));
        b.iter(|| engine.evaluate(black_box(params.clone()), "2025-01-01"))
    });

    // With tracing enabled
    group.bench_function("simple_with_trace", |b| {
        let article = &simple_law.articles[0];
        let engine = ArticleEngine::new(article, &simple_law);
        let mut params = BTreeMap::new();
        params.insert("inkomen".to_string(), Value::Int(35000));
        params.insert("drempel".to_string(), Value::Int(25000));
        b.iter(|| {
            engine.evaluate_with_trace(
                black_box(params.clone()),
                "2025-01-01",
                None,
                std::rc::Rc::new(std::cell::RefCell::new(
                    regelrecht_engine::TraceBuilder::new(),
                )),
            )
        })
    });

    group.finish();
}

criterion_group!(benches, bench_article_evaluate);
criterion_main!(benches);
