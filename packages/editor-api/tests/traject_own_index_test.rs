//! Reproductie + regressietests voor "traject-eigen corpus indexeert 0
//! wetten na promote" (tickets #6, vervolg op de promote-flow van #952).
//!
//! Twee scenario's rond dezelfde deployed-achtige federatie-config als
//! `promote_user_token_write_test.rs` — GitHub-backed writable-own zonder
//! server-side `CORPUS_AUTH_*`-token, lokale seed als centraal corpus,
//! schrijven via het gekoppelde user-token:
//!
//! 1. **Onleesbare traject-repo (het prod-symptoom).** De promote slaagt
//!    (write met user-token), maar de indexscan van de writable-own faalt
//!    — zoals op een privé-repo zonder geconfigureerd token:
//!    unauthenticated Trees-API geeft 404, en hier faalt ook het
//!    user-token (ongemockt trees-pad). Sinds de user-token-fallback op de
//!    scan valt de bibliotheek dan niet langer stil terug op de seed: de
//!    listing faalt luid (428 zonder gekoppeld token; mét token dat de repo
//!    alsnog niet kan lezen een geclassificeerde 404 die het repo-adres
//!    noemt). `GET .../sources` blijft de diagnose dragen: de writable-own
//!    toont `law_count: 0` mét een `index_error`.
//!    (De volledige user-token-scanflow — succes via het token, heling na
//!    een kapotte snapshot — staat in `traject_index_user_token_test.rs`.)
//! 2. **Happy path van #952.** Zodra de scan de traject-repo wél kan lezen
//!    (Trees-API geeft de gepromote bestanden terug), staat de wet in de
//!    traject-index op `source_priority: 0` en is de source gezond
//!    (`index_error: null`, `law_count: 1`).
//!
//! GitHub wordt gespeeld door wiremock via de proces-brede
//! `GITHUB_API_BASE`-seam — daarom staan beide scenario's in ÉÉN test
//! (zelfde afweging als `promote_user_token_write_test.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, insert_traject, seed_account, session_for, state_with_user_token_mode,
    traject_ref,
};

use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_corpus::dto::PaginationParams;
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{
    list_traject_corpus_laws, list_traject_sources, promote_corpus_law,
};
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_voorbeeld_promotie";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "corpus/regulation";

/// Traject zoals deployed: GitHub writable-own op een user-gekozen repo
/// (auth_ref zonder geconfigureerd `CORPUS_AUTH_*_TOKEN`-env → server-side
/// géén token) plus een lokale read-seed die het centrale corpus speelt.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    own_repo_owner: &str,
    own_repo_name: &str,
    seed_dir: &std::path::Path,
) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 $2, $3, $4, 'main', $5,
                 0, 'example-unset-token-ref', TRUE)",
    )
    .bind(traject_id)
    .bind(own_repo_owner)
    .bind(own_repo_name)
    .bind(OWN_BRANCH)
    .bind(OWN_SUBPATH)
    .execute(pool)
    .await
    .unwrap();
    add_central_seed(pool, traject_id, seed_dir).await;
    traject_id
}

/// Volledige wet-map in het centrale corpus: twee versies plus één
/// scenario-file — de bestanden die de promote kopieert.
fn write_central_law(central_dir: &std::path::Path) {
    let law_dir = central_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(law_dir.join("scenarios")).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2025\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("2024-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2024\n"),
    )
    .unwrap();
    std::fs::write(
        law_dir.join("scenarios").join("basis.feature"),
        "Feature: basis\n",
    )
    .unwrap();
}

/// Mocks voor de user-token-schrijfflow op één traject-repo: branch-check
/// (bestaat al), en de Contents-PUT's van de promote.
async fn mount_write_mocks(server: &MockServer, own_repo: &str) {
    // De read waarmee promote controleert of de wet er al staat, 404't.
    // Dat telt pas als afwezigheid zodra de repo zelf leesbaar blijkt —
    // anders zou een repo die dit token niet mag zien als "leeg" lezen.
    // Eenmalig, want daarna vertelt dit scenario juist het omgekeerde
    // verhaal: de repo staat er ná de promote niet (meer), wat de
    // diagnose van de mislukte indexscan hoort op te leveren.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{own_repo}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "full_name": own_repo,
        })))
        .up_to_n_times(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{own_repo}/git/ref/heads/{OWN_BRANCH}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": format!("refs/heads/{OWN_BRANCH}"),
            "object": { "sha": "branch-sha" },
        })))
        .mount(server)
        .await;
    for file in [
        format!("wet/{LAW_ID}/2025-01-01.yaml"),
        format!("wet/{LAW_ID}/2024-01-01.yaml"),
        format!("wet/{LAW_ID}/scenarios/basis.feature"),
    ] {
        Mock::given(method("PUT"))
            .and(path(format!(
                "/repos/{own_repo}/contents/{OWN_SUBPATH}/{file}"
            )))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "content": { "sha": format!("sha-{file}") },
            })))
            .mount(server)
            .await;
    }
}

/// De Trees-API-listing van een traject-repo waar de gepromote wet op de
/// branch staat — wat de indexscan ziet zodra hij de repo kán lezen.
async fn mount_tree_with_promoted_law(server: &MockServer, own_repo: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{own_repo}/git/trees/{OWN_BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                {
                    "path": format!("{OWN_SUBPATH}/wet/{LAW_ID}/2025-01-01.yaml"),
                    "type": "blob",
                    "sha": "blob-2025",
                },
                {
                    "path": format!("{OWN_SUBPATH}/wet/{LAW_ID}/2024-01-01.yaml"),
                    "type": "blob",
                    "sha": "blob-2024",
                },
                {
                    "path": format!("{OWN_SUBPATH}/wet/{LAW_ID}/scenarios/basis.feature"),
                    "type": "blob",
                    "sha": "blob-feature",
                },
            ],
        })))
        .mount(server)
        .await;
}

fn ids_query() -> Query<PaginationParams> {
    Query(PaginationParams {
        offset: 0,
        limit: None,
        q: None,
        ids: Some(LAW_ID.to_string()),
        source: None,
        enriched: false,
    })
}

/// Eén test voor beide scenario's: de `GITHUB_API_BASE`-override is
/// proces-breed, dus parallelle tests met elk een eigen mockserver zouden
/// elkaars fetchers verhangen.
#[tokio::test]
async fn traject_index_shows_promoted_law_and_surfaces_failed_own_scans() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());

    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());

    let oauth = GithubOAuth::for_tests(true); // user-token-schrijfmodus aan
    let state = state_with_user_token_mode(db.pool.clone(), oauth.clone());

    let (owner_id, sub) = seed_account(&db.pool, "alice@test.local").await;
    let account = AccountRecord {
        id: owner_id,
        person_sub: sub.clone(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            account.id,
            "user-token",
        ))
        .unwrap(),
    );

    // ------------------------------------------------------------------
    // Scenario 1: de indexscan kan de traject-repo NIET lezen (prod-
    // symptoom: privé-repo, geen server-side token → Trees-API 404, hier
    // wiremock's default voor het ongemockte trees-pad van repo A).
    // ------------------------------------------------------------------
    let repo_a = "example-org/example-unreadable-repo";
    let traject_a = seeded_traject(
        &db.pool,
        owner_id,
        "example-org",
        "example-unreadable-repo",
        central.path(),
    )
    .await;
    let tref_a = traject_ref(traject_a);
    mount_write_mocks(&server, repo_a).await;

    // De promote zelf slaagt: de write gaat met het user-token.
    let response = promote_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref_a.clone(), LAW_ID.to_string())),
        headers.clone(),
    )
    .await
    .expect("promote met gekoppeld token moet slagen");
    assert_eq!(response.0.copied_files, 3, "2 versies + 1 scenario");

    // …maar de herbouwde index (promote invalideert de cache) mist de
    // traject-repo. Sinds de user-token-fallback op de indexscan valt de
    // bibliotheek dan niet langer stil terug op de seed: een request
    // zónder gekoppeld token faalt luid met de 428-koppel-flow…
    let err = list_traject_corpus_laws(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref_a.clone()),
        ids_query(),
        HeaderMap::new(),
    )
    .await
    .expect_err("onleesbare traject-repo zonder token mag niet stil terugvallen op de seed");
    assert_eq!(
        err.0,
        axum::http::StatusCode::PRECONDITION_REQUIRED,
        "zonder gekoppeld token hoort de bibliotheek de 428-koppel-flow te geven, got: {}: {}",
        err.0,
        err.1
    );

    // …en een request mét gekoppeld token (dat deze repo alsnog niet kan
    // lezen — het trees-pad is ongemockt, dus ook met token 404) faalt
    // luid én geclassificeerd: de diagnose vraagt de repo-lookup na met
    // hetzelfde token, krijgt óók 404, en meldt dat de repo niet op dit
    // adres staat.
    let err = list_traject_corpus_laws(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref_a.clone()),
        ids_query(),
        headers.clone(),
    )
    .await
    .expect_err("scan-fout mét token → expliciete fout, geen stille fallback");
    assert_eq!(
        err.0,
        axum::http::StatusCode::NOT_FOUND,
        "got: {}: {}",
        err.0,
        err.1
    );
    assert!(
        err.1.contains(repo_a),
        "de melding moet het adres noemen waar de repo niet (meer) staat, got: {}",
        err.1
    );

    // Niet langer stil: /sources meldt per source waaróm de scan faalde.
    let sources = list_traject_sources(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref_a.clone()),
        HeaderMap::new(),
    )
    .await
    .expect("sources-listing moet slagen");
    let own = sources
        .0
        .iter()
        .find(|s| s.id == "traject-own-test")
        .expect("writable-own source ontbreekt in /sources");
    assert_eq!(own.law_count, 0);
    let err = own
        .index_error
        .as_deref()
        .expect("gefaalde scan moet een index_error op de source zetten");
    assert!(
        err.contains("404"),
        "index_error moet de onderliggende fout dragen, got: {err}"
    );
    let seed = sources.0.iter().find(|s| s.id == "central-seed").unwrap();
    assert_eq!(
        seed.index_error, None,
        "de gezonde seed-source mag geen fout dragen"
    );

    // ------------------------------------------------------------------
    // Scenario 2 (happy path #952): de scan kan de traject-repo WèL lezen
    // → de gepromote wet staat in de traject-index op priority 0.
    // ------------------------------------------------------------------
    let repo_b = "example-org/example-readable-repo";
    let traject_b = seeded_traject(
        &db.pool,
        owner_id,
        "example-org",
        "example-readable-repo",
        central.path(),
    )
    .await;
    let tref_b = traject_ref(traject_b);
    mount_write_mocks(&server, repo_b).await;

    let response = promote_corpus_law(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path((tref_b.clone(), LAW_ID.to_string())),
        headers.clone(),
    )
    .await
    .expect("promote met gekoppeld token moet slagen");
    assert_eq!(response.0.copied_files, 3);

    // Pas ná de promote de Trees-listing mounten: tijdens de promote-build
    // hoort de repo nog leeg/onbereikbaar te zijn (anders weigert de
    // promote met 409 "staat al in dit traject").
    mount_tree_with_promoted_law(&server, repo_b).await;

    let laws = list_traject_corpus_laws(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref_b.clone()),
        ids_query(),
        HeaderMap::new(),
    )
    .await
    .expect("laws-listing moet slagen");
    assert_eq!(laws.0.len(), 1, "één resolutie per wet, got: {:?}", laws.0);
    assert_eq!(
        laws.0[0].source_id, "traject-own-test",
        "de gepromote wet moet uit de traject-eigen source komen"
    );
    assert_eq!(
        laws.0[0].source_priority, 0,
        "traject-eigen source wint met priority 0"
    );

    let sources = list_traject_sources(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&sub).await,
        Path(tref_b.clone()),
        HeaderMap::new(),
    )
    .await
    .expect("sources-listing moet slagen");
    let own = sources
        .0
        .iter()
        .find(|s| s.id == "traject-own-test")
        .expect("writable-own source ontbreekt in /sources");
    assert_eq!(own.law_count, 1, "de gepromote wet telt mee in de index");
    assert_eq!(own.index_error, None, "gezonde scan → geen index_error");

    std::env::remove_var("GITHUB_API_BASE");
}
