use criterion::{criterion_group, criterion_main, Criterion};
use regelrecht_engine::priority::{resolve_candidate, Candidate};
use regelrecht_engine::types::RegulatoryLayer;
use regelrecht_engine::ArticleBasedLaw;
use std::hint::black_box;

fn make_law(id: &str, layer: RegulatoryLayer, valid_from: &str) -> ArticleBasedLaw {
    ArticleBasedLaw {
        id: id.to_string(),
        name: Some(id.to_string()),
        regulatory_layer: layer,
        valid_from: Some(valid_from.to_string()),
        valid_to: None,
        publication_date: valid_from.to_string(),
        uuid: None,
        schema: None,
        competent_authority: None,
        bwb_id: None,
        url: None,
        identifiers: None,
        gemeente_code: None,
        provincie_code: None,
        waterschap_code: None,
        celex_nummer: None,
        eli: None,
        tractatenblad_id: None,
        unts_nummer: None,
        stcrt_id: None,
        organisation: None,
        officiele_titel: None,
        jaar: None,
        legal_basis: None,
        procedure: None,
        member_of: None,
        preamble: None,
        articles: vec![],
        content_hash: None,
    }
}

fn bench_priority_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_resolution");

    // Single candidate (trivial)
    let law1 = make_law("wet_a", RegulatoryLayer::Wet, "2025-01-01");
    let single = vec![Candidate {
        law: &law1,
        article_number: "1".to_string(),
    }];
    group.bench_function("single_candidate", |b| {
        b.iter(|| resolve_candidate(black_box(&single)))
    });

    // Two candidates, lex superior
    let law_wet = make_law("wet_a", RegulatoryLayer::Wet, "2025-01-01");
    let law_mr = make_law("mr_b", RegulatoryLayer::MinisterieleRegeling, "2025-01-01");
    let lex_superior = vec![
        Candidate {
            law: &law_mr,
            article_number: "1".to_string(),
        },
        Candidate {
            law: &law_wet,
            article_number: "2".to_string(),
        },
    ];
    group.bench_function("two_lex_superior", |b| {
        b.iter(|| resolve_candidate(black_box(&lex_superior)))
    });

    // Two candidates, lex posterior (same layer)
    let law_old = make_law("mr_a", RegulatoryLayer::MinisterieleRegeling, "2024-01-01");
    let law_new = make_law("mr_b", RegulatoryLayer::MinisterieleRegeling, "2025-01-01");
    let lex_posterior = vec![
        Candidate {
            law: &law_old,
            article_number: "1".to_string(),
        },
        Candidate {
            law: &law_new,
            article_number: "2".to_string(),
        },
    ];
    group.bench_function("two_lex_posterior", |b| {
        b.iter(|| resolve_candidate(black_box(&lex_posterior)))
    });

    // Five candidates, mixed layers
    let law_eu = make_law("eu_reg", RegulatoryLayer::EuVerordening, "2020-01-01");
    let law_wet = make_law("wet_c", RegulatoryLayer::Wet, "2024-06-01");
    let law_amvb = make_law("amvb_d", RegulatoryLayer::Amvb, "2025-01-01");
    let law_mr1 = make_law("mr_e", RegulatoryLayer::MinisterieleRegeling, "2024-01-01");
    let law_mr2 = make_law("mr_f", RegulatoryLayer::MinisterieleRegeling, "2025-01-01");
    let five_candidates = vec![
        Candidate {
            law: &law_mr1,
            article_number: "1".to_string(),
        },
        Candidate {
            law: &law_amvb,
            article_number: "2".to_string(),
        },
        Candidate {
            law: &law_wet,
            article_number: "3".to_string(),
        },
        Candidate {
            law: &law_mr2,
            article_number: "4".to_string(),
        },
        Candidate {
            law: &law_eu,
            article_number: "5".to_string(),
        },
    ];
    group.bench_function("five_mixed_layers", |b| {
        b.iter(|| resolve_candidate(black_box(&five_candidates)))
    });

    group.finish();
}

criterion_group!(benches, bench_priority_resolution);
criterion_main!(benches);
