use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

use crate::config::PipelineConfig;
use crate::error::Result;

/// Advisory lock key for migration coordination. All components that call
/// `ensure_schema` use this same key so only one runs migrations at a time.
pub const MIGRATION_LOCK_KEY: i64 = 0x5245_4745_4C52_4543; // "REGELREC"

pub async fn create_pool(config: &PipelineConfig) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&config.database_url)
        .await?;

    Ok(pool)
}

/// The pipeline's migrations, embedded at build time.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Ensure the database schema is up to date.
///
/// Uses a PostgreSQL advisory lock so that whichever component starts first
/// runs migrations while the others block. Since sqlx migrations are
/// idempotent, the second caller safely no-ops after the lock is released.
pub async fn ensure_schema(pool: &PgPool) -> Result<()> {
    regelrecht_shared::postgres::migrate_locked(pool, MIGRATION_LOCK_KEY, &MIGRATOR, None).await?;
    Ok(())
}
