// Allowed crate-wide: benchmark setup outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{Criterion, criterion_group, criterion_main};
use regelrecht_engine::{ArticleBasedLaw, LawLoad};
use std::hint::black_box;
use std::path::PathBuf;

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("regulation")
        .join("nl")
}

fn read_law(relative_path: &str) -> String {
    let path = corpus_path().join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
}

fn bench_yaml_parsing(c: &mut Criterion) {
    let simple = read_law("ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml");
    let medium = read_law("wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
    let large = read_law("wet/zorgverzekeringswet/2025-01-01.yaml");

    let mut group = c.benchmark_group("law_loading");

    group.bench_function("small_standaardpremie", |b| {
        b.iter(|| ArticleBasedLaw::from_yaml_str(black_box(&simple)))
    });

    group.bench_function("medium_zorgtoeslag", |b| {
        b.iter(|| ArticleBasedLaw::from_yaml_str(black_box(&medium)))
    });

    group.bench_function("large_zorgverzekeringswet", |b| {
        b.iter(|| ArticleBasedLaw::from_yaml_str(black_box(&large)))
    });

    group.finish();
}

criterion_group!(benches, bench_yaml_parsing);
criterion_main!(benches);
