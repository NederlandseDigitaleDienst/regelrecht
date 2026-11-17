// Allowed crate-wide: benchmark setup outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{Criterion, criterion_group, criterion_main};
use regelrecht_engine::{RuleContext, Value, ValueResolver};
use std::collections::BTreeMap;
use std::hint::black_box;

fn make_context() -> RuleContext<'static> {
    let mut parameters = BTreeMap::new();
    parameters.insert("bsn".to_string(), Value::String("999993653".to_string()));
    parameters.insert("inkomen".to_string(), Value::Int(35000));
    parameters.insert("leeftijd".to_string(), Value::Int(30));

    let mut ctx = RuleContext::new(parameters, "2025-01-01").unwrap();

    let mut definitions = BTreeMap::new();
    definitions.insert("drempelinkomen".to_string(), Value::Int(25000));
    definitions.insert("maximale_toeslag".to_string(), Value::Int(2112));
    ctx.set_definitions_raw(definitions);

    ctx.set_output("is_verzekerd", Value::Bool(true));
    ctx.set_output("toetsingsinkomen", Value::Int(35000));

    ctx.set_local("item", Value::String("test_item".to_string()));

    ctx
}

fn bench_resolve_variable(c: &mut Criterion) {
    let ctx = make_context();

    let mut group = c.benchmark_group("variable_resolution");

    // Context variable (referencedate) — highest priority
    group.bench_function("context_referencedate", |b| {
        b.iter(|| ctx.resolve(black_box("referencedate")))
    });

    // Dot notation (referencedate.year)
    group.bench_function("dot_notation_year", |b| {
        b.iter(|| ctx.resolve(black_box("referencedate.year")))
    });

    // Local scope (FOREACH variables)
    group.bench_function("local_scope", |b| b.iter(|| ctx.resolve(black_box("item"))));

    // Outputs
    group.bench_function("output_lookup", |b| {
        b.iter(|| ctx.resolve(black_box("is_verzekerd")))
    });

    // Definitions
    group.bench_function("definition_lookup", |b| {
        b.iter(|| ctx.resolve(black_box("drempelinkomen")))
    });

    // Parameters — lowest priority, most lookups
    group.bench_function("parameter_lookup", |b| {
        b.iter(|| ctx.resolve(black_box("bsn")))
    });

    // Variable not found (all scopes checked)
    group.bench_function("not_found", |b| {
        b.iter(|| ctx.resolve(black_box("nonexistent_variable")).ok())
    });

    group.finish();
}

criterion_group!(benches, bench_resolve_variable);
criterion_main!(benches);
