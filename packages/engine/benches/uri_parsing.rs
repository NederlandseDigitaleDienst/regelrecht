use criterion::{Criterion, criterion_group, criterion_main};
use regelrecht_engine::uri::{RegelrechtUri, RegelrechtUriBuilder};
use std::hint::black_box;

fn bench_parse_regelrecht_uri(c: &mut Criterion) {
    let mut group = c.benchmark_group("uri_parsing");

    group.bench_function("regelrecht_uri_basic", |b| {
        b.iter(|| RegelrechtUri::parse(black_box("regelrecht://zvw/is_verzekerd")))
    });

    group.bench_function("regelrecht_uri_with_field", |b| {
        b.iter(|| {
            RegelrechtUri::parse(black_box(
                "regelrecht://wet_op_de_zorgtoeslag/bereken_zorgtoeslag#heeft_recht_op_zorgtoeslag",
            ))
        })
    });

    group.bench_function("file_path_reference", |b| {
        b.iter(|| {
            RegelrechtUri::parse(black_box(
                "regulation/nl/ministeriele_regeling/regeling_standaardpremie#standaardpremie",
            ))
        })
    });

    group.bench_function("internal_reference", |b| {
        b.iter(|| RegelrechtUri::parse(black_box("#standaardpremie")))
    });

    group.finish();
}

fn bench_uri_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("uri_builder");

    group.bench_function("build_string", |b| {
        b.iter(|| {
            RegelrechtUriBuilder::new("wet_op_de_zorgtoeslag", "bereken_zorgtoeslag")
                .with_field("heeft_recht_op_zorgtoeslag")
                .build()
        })
    });

    group.bench_function("build_parsed", |b| {
        b.iter(|| {
            RegelrechtUriBuilder::new("wet_op_de_zorgtoeslag", "bereken_zorgtoeslag")
                .with_field("heeft_recht_op_zorgtoeslag")
                .build_parsed()
        })
    });

    group.finish();
}

criterion_group!(benches, bench_parse_regelrecht_uri, bench_uri_builder);
criterion_main!(benches);
