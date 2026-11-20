//! Test helpers exposed via the `test-utils` Cargo feature.
//!
//! These are used by this crate's integration tests and by downstream
//! crates (e.g. admin) that need a real Postgres container with the
//! pipeline schema applied. Where the database comes from is
//! `regelrecht_shared::postgres::test_utils`.

use regelrecht_shared::postgres::test_utils::TestDatabase;
use sqlx::PgPool;

use crate::config::PipelineConfig;
use crate::db;

/// Postgres database with the pipeline schema applied and seed rows truncated,
/// ready for integration tests. Every `TestDb` gets its own database inside the
/// shared container, so tests stay isolated from each other.
pub struct TestDb {
    pub pool: PgPool,
    _database: TestDatabase,
}

impl TestDb {
    #[allow(clippy::unwrap_used)]
    pub async fn new() -> Self {
        let database = TestDatabase::create().await;
        let config = PipelineConfig::new(database.url());
        let pool = db::create_pool(&config).await.unwrap();
        // ensure_schema takes a pg_advisory_lock, which is scoped to the
        // database, so parallel tests do not queue behind each other.
        db::ensure_schema(&pool).await.unwrap();

        // Clear seed data from migrations so tests start with empty tables.
        sqlx::query("TRUNCATE jobs, law_entries CASCADE")
            .execute(&pool)
            .await
            .unwrap();

        Self {
            pool,
            _database: database,
        }
    }
}
