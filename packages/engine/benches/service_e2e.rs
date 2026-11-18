// Allowed crate-wide: benchmark setup outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{Criterion, criterion_group, criterion_main};
use regelrecht_engine::{LawExecutionService, Value};
use std::collections::BTreeMap;
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

fn load_law_file(service: &mut LawExecutionService, relative_path: &str) {
    let path = corpus_path().join(relative_path);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    service
        .load_law(&content)
        .unwrap_or_else(|e| panic!("Failed to load {}: {}", path.display(), e));
}

fn load_all_laws(service: &mut LawExecutionService) {
    for entry in walkdir::WalkDir::new(corpus_path())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            let content = std::fs::read_to_string(path).unwrap();
            let _ = service.load_law(&content); // ignore errors for unsupported files
        }
    }
}

fn make_record(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn register_zorgtoeslag_data(service: &mut LawExecutionService) {
    let bsn = Value::String("999993653".to_string());

    // RVIG personal_data
    service
        .register_dict_source(
            "personal_data",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("geboortedatum", Value::String("2005-01-01".to_string())),
                ("verblijfsadres", Value::String("Amsterdam".to_string())),
                ("land_verblijf", Value::String("NEDERLAND".to_string())),
            ])],
        )
        .unwrap();

    // RVIG relationship_data
    service
        .register_dict_source(
            "relationship_data",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("partnerschap_type", Value::String("GEEN".to_string())),
                ("partner_bsn", Value::Null),
            ])],
        )
        .unwrap();

    // RVZ insurance
    service
        .register_dict_source(
            "insurance",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("polis_status", Value::String("ACTIEF".to_string())),
                ("verdragsinschrijving", Value::Bool(false)),
            ])],
        )
        .unwrap();

    // BELASTINGDIENST box1
    service
        .register_dict_source(
            "box1",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("loon_uit_dienstbetrekking", Value::Int(79547)),
                ("uitkeringen_en_pensioenen", Value::Int(0)),
                ("winst_uit_onderneming", Value::Int(0)),
                ("resultaat_overige_werkzaamheden", Value::Int(0)),
                ("eigen_woning", Value::Int(0)),
            ])],
        )
        .unwrap();

    // BELASTINGDIENST inkomensgegevens (AWR art. 21, Awir art. 8 lid 2)
    service
        .register_dict_source(
            "inkomensgegevens",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                (
                    "aanslag_of_navorderingsaanslag_vastgesteld",
                    Value::Bool(true),
                ),
                ("belastbaar_loon", Value::Int(79547)),
                ("niet_in_nederland_belastbaar_inkomen", Value::Int(0)),
            ])],
        )
        .unwrap();

    // BELASTINGDIENST box2
    service
        .register_dict_source(
            "box2",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("reguliere_voordelen", Value::Int(0)),
                ("vervreemdingsvoordelen", Value::Int(0)),
            ])],
        )
        .unwrap();

    // BELASTINGDIENST box3
    service
        .register_dict_source(
            "box3",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("spaargeld", Value::Int(0)),
                ("beleggingen", Value::Int(0)),
                ("onroerend_goed", Value::Int(0)),
                ("schulden", Value::Int(0)),
            ])],
        )
        .unwrap();

    // DJI detenties
    service
        .register_dict_source(
            "detenties",
            "bsn",
            vec![make_record(&[
                ("bsn", bsn.clone()),
                ("detentiestatus", Value::Null),
                ("inrichting_type", Value::Null),
                ("zorgtype", Value::Null),
                ("juridische_grondslag", Value::Null),
            ])],
        )
        .unwrap();
}

fn bench_simple_law(c: &mut Criterion) {
    let mut service = LawExecutionService::new();
    load_law_file(
        &mut service,
        "ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml",
    );
    // Standaardpremie also needs the parent law for IoC
    load_law_file(&mut service, "wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");

    let mut group = c.benchmark_group("service_e2e_simple");

    group.bench_function("standaardpremie", |b| {
        b.iter(|| {
            service.evaluate_law_output(
                black_box("regeling_standaardpremie"),
                black_box("standaardpremie"),
                BTreeMap::new(),
                "2025-01-01",
            )
        })
    });

    group.bench_function("standaardpremie_with_trace", |b| {
        b.iter(|| {
            service.evaluate_law_output_with_trace(
                black_box("regeling_standaardpremie"),
                black_box("standaardpremie"),
                BTreeMap::new(),
                "2025-01-01",
            )
        })
    });

    group.finish();
}

fn bench_complex_law(c: &mut Criterion) {
    let mut service = LawExecutionService::new();
    load_all_laws(&mut service);
    register_zorgtoeslag_data(&mut service);

    let mut params = BTreeMap::new();
    params.insert("bsn".to_string(), Value::String("999993653".to_string()));

    let mut group = c.benchmark_group("service_e2e_complex");

    // Verify the execution actually succeeds before benchmarking
    let verify = service.evaluate_law_output(
        "wet_op_de_zorgtoeslag",
        "hoogte_zorgtoeslag",
        params.clone(),
        "2025-01-01",
    );
    assert!(
        verify.is_ok(),
        "zorgtoeslag execution failed: {:?}",
        verify.err()
    );

    group.bench_function("zorgtoeslag_full", |b| {
        b.iter(|| {
            service
                .evaluate_law_output(
                    black_box("wet_op_de_zorgtoeslag"),
                    black_box("hoogte_zorgtoeslag"),
                    params.clone(),
                    "2025-01-01",
                )
                .unwrap()
        })
    });

    group.bench_function("zorgtoeslag_with_trace", |b| {
        b.iter(|| {
            service
                .evaluate_law_output_with_trace(
                    black_box("wet_op_de_zorgtoeslag"),
                    black_box("hoogte_zorgtoeslag"),
                    params.clone(),
                    "2025-01-01",
                )
                .unwrap()
        })
    });

    group.finish();
}

criterion_group!(benches, bench_simple_law, bench_complex_law);
criterion_main!(benches);
