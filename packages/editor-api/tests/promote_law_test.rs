//! Integration tests for `corpus_handlers::promote_corpus_law`: een wet uit
//! de centrale-corpus-seed van het traject kopiëren naar de traject-repo —
//! alle versie-YAML's plus de `scenarios/*.feature`-bestanden, en een 409
//! wanneer de wet al in het traject staat (geen dubbele bestanden).
//!
//! Zelfde hermetische opzet als `traject_reads_test.rs`: lokale sources, geen
//! GitHub/netwerk, handlers direct aangeroepen met inline axum-extractors.
//! Het traject heeft twee sources, zoals in productie: een read-seed die het
//! centrale corpus speelt en een (lege) writable-own repo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, empty_state, insert_traject, seed_account, session_for, traject_ref,
};

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::promote_corpus_law;
use regelrecht_editor_api::state::AppState;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_voorbeeld_promotie";

/// Traject met twee lokale sources, zoals in productie: een read-seed op
/// `seed_dir` (het "centrale corpus", priority 2) en een writable-own op
/// `own_dir` (priority 0). Saves op seed-wetten routeren naar de
/// writable-own via `write_target_for_source` — hetzelfde pad dat promote
/// gebruikt.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    own_dir: &std::path::Path,
    seed_dir: &std::path::Path,
) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'local', 'Local', 'local'::corpus_source_type, $2,
                 0, '[]'::jsonb, TRUE)",
    )
    .bind(traject_id)
    .bind(own_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();
    add_central_seed(pool, traject_id, seed_dir).await;
    traject_id
}

fn test_account() -> AccountRecord {
    AccountRecord {
        id: Uuid::new_v4(),
        person_sub: "test-sub".to_string(),
        email: "test@example.gov".to_string(),
        name: "Test User".to_string(),
    }
}

/// Schrijf de volledige wet-map in het centrale corpus: twee versies (met
/// `machine_readable`-marker in de nieuwste) en één scenario-file.
fn write_central_law(central_dir: &std::path::Path) {
    let law_dir = central_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(law_dir.join("scenarios")).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Versie 2025\nmachine_readable: MARKER_2025\n"),
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

async fn promote(
    state: AppState,
    session: Session,
    tref: &str,
    law_id: &str,
) -> Result<axum::Json<regelrecht_editor_api::corpus_handlers::PromoteResponse>, (StatusCode, String)>
{
    promote_corpus_law(
        State(state),
        Extension(test_account()),
        session,
        Path((tref.to_string(), law_id.to_string())),
        HeaderMap::new(),
    )
    .await
}

#[tokio::test]
async fn promote_copies_all_versions_and_scenarios_to_the_traject_repo() {
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let response = promote(state.clone(), session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect("promote must succeed");
    assert_eq!(response.0.law_id, LAW_ID);
    assert_eq!(response.0.copied_files, 3, "2 versies + 1 scenario");

    // De volledige wet-map staat nu in de traject-repo, byte-voor-byte,
    // inclusief machine_readable en het scenario.
    let base = traject_dir.path().join("wet").join(LAW_ID);
    let newest = std::fs::read_to_string(base.join("2025-01-01.yaml")).unwrap();
    assert!(newest.contains("MARKER_2025"), "got: {newest}");
    assert_eq!(
        newest,
        std::fs::read_to_string(
            central
                .path()
                .join("wet")
                .join(LAW_ID)
                .join("2025-01-01.yaml")
        )
        .unwrap()
    );
    assert!(base.join("2024-01-01.yaml").exists());
    assert_eq!(
        std::fs::read_to_string(base.join("scenarios").join("basis.feature")).unwrap(),
        "Feature: basis\n"
    );
}

#[tokio::test]
async fn promote_refuses_a_law_already_in_the_traject_repo() {
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let _ = promote(state.clone(), session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect("first promote must succeed");

    // Tweede promote: de wet staat al in de traject-repo → 409, en er
    // veranderen geen bestanden (geen dubbele/overschreven bestanden).
    let before = std::fs::read_to_string(
        traject_dir
            .path()
            .join("wet")
            .join(LAW_ID)
            .join("2025-01-01.yaml"),
    )
    .unwrap();
    let err = promote(state.clone(), session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect_err("second promote must be refused");
    assert_eq!(err.0, StatusCode::CONFLICT);
    let after = std::fs::read_to_string(
        traject_dir
            .path()
            .join("wet")
            .join(LAW_ID)
            .join("2025-01-01.yaml"),
    )
    .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn promote_keeps_a_traject_edited_scenario_of_a_federated_law() {
    // `save_scenario` routeert scenario's van gefedereerde wetten naar de
    // writable-own zonder dat er een wet-YAML in de traject-repo staat. Zo'n
    // traject-edit mag een promote niet stilletjes overschrijven: de
    // traject-versie wint, de wet-YAML's worden gewoon gekopieerd.
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    write_central_law(central.path());
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let scenario_dir = traject_dir
        .path()
        .join("wet")
        .join(LAW_ID)
        .join("scenarios");
    std::fs::create_dir_all(&scenario_dir).unwrap();
    std::fs::write(
        scenario_dir.join("basis.feature"),
        "Feature: basis (traject-edit)\n",
    )
    .unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let response = promote(state, session_for(&sub).await, &tref, LAW_ID)
        .await
        .expect("promote must succeed despite the scenario edit");
    assert_eq!(
        response.0.copied_files, 2,
        "2 versies; scenario overgeslagen"
    );

    let base = traject_dir.path().join("wet").join(LAW_ID);
    assert!(base.join("2025-01-01.yaml").exists());
    assert!(base.join("2024-01-01.yaml").exists());
    assert_eq!(
        std::fs::read_to_string(base.join("scenarios").join("basis.feature")).unwrap(),
        "Feature: basis (traject-edit)\n",
        "traject-edit mag niet overschreven worden"
    );
}

#[tokio::test]
async fn promote_404s_for_a_law_missing_from_the_central_corpus() {
    let db = TestDb::new().await;
    let central = tempfile::tempdir().unwrap();
    let state = empty_state(db.pool.clone());

    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let traject_dir = tempfile::tempdir().unwrap();
    let traject_id = seeded_traject(&db.pool, owner, traject_dir.path(), central.path()).await;
    let tref = traject_ref(traject_id);

    let err = promote(state, session_for(&sub).await, &tref, "wet_bestaat_niet")
        .await
        .expect_err("unknown law must 404");
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}
