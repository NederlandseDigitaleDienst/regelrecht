//! What the crates that keep their data in Postgres share: where the
//! database is, and running a crate's migrations once while other instances
//! wait.
//!
//! Each crate keeps its own migrations and its own advisory lock key; this
//! module only holds the mechanics, so they are written once.

#[cfg(feature = "test-utils")]
pub mod test_utils;

use sqlx::PgPool;
use sqlx::migrate::{MigrateError, Migrator};

/// The variables that name the database, in the order they are read:
/// `DATABASE_URL` for a local run, `DATABASE_SERVER_FULL` as ZAD sets it.
pub const DATABASE_URL_VARS: [&str; 2] = ["DATABASE_URL", "DATABASE_SERVER_FULL"];

/// The database URL in the environment `env`, a lookup by name such as
/// `|k| std::env::var(k).ok()`: the first of [`DATABASE_URL_VARS`] that is
/// set and not empty.
pub fn database_url(env: impl Fn(&str) -> Option<String>) -> Option<String> {
    DATABASE_URL_VARS
        .iter()
        .find_map(|name| env(name).filter(|v| !v.trim().is_empty()))
}

/// Run `migrator` on `pool` under the advisory lock `lock_key`, so that of
/// several instances starting at once one migrates and the others wait and
/// then find nothing left to do.
///
/// With `schema`, that schema is created first if it does not exist, under
/// the same lock. The pool decides where the tables land, the migration table
/// included: a crate with its own schema connects with that schema first in
/// its `search_path`.
///
/// # Errors
///
/// When the lock cannot be taken, the schema not created, or a migration
/// fails.
pub async fn migrate_locked(
    pool: &PgPool,
    lock_key: i64,
    migrator: &Migrator,
    schema: Option<&str>,
) -> Result<(), MigrateError> {
    // Advisory locks belong to a session: take and release this one on one
    // dedicated connection, not on whichever the pool hands out.
    let mut conn = pool.acquire().await.map_err(MigrateError::Execute)?;
    tracing::info!(lock_key, "acquiring migration lock...");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(lock_key)
        .execute(conn.as_mut())
        .await
        .map_err(MigrateError::Execute)?;

    let result = migrate(pool, &mut conn, migrator, schema).await;

    // Release the lock on the same connection, also after a failure.
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(lock_key)
        .execute(conn.as_mut())
        .await
    {
        tracing::warn!(error = %e, "failed to release migration lock");
    }
    result
}

async fn migrate(
    pool: &PgPool,
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    migrator: &Migrator,
    schema: Option<&str>,
) -> Result<(), MigrateError> {
    if let Some(schema) = schema {
        sqlx::query(&format!(
            "CREATE SCHEMA IF NOT EXISTS {}",
            quote_identifier(schema)
        ))
        .execute(conn.as_mut())
        .await
        .map_err(MigrateError::Execute)?;
    }
    tracing::info!("running database migrations...");
    migrator.run(pool).await?;
    tracing::info!("migrations completed");
    Ok(())
}

/// `name` as a quoted SQL identifier: in double quotes, with a double quote
/// in it doubled.
pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_url_prefers_database_url_and_skips_empty_values() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(database_url(env(&[])), None);
        assert_eq!(
            database_url(env(&[("DATABASE_SERVER_FULL", "postgres://zad")])),
            Some("postgres://zad".into())
        );
        assert_eq!(
            database_url(env(&[
                ("DATABASE_URL", "postgres://local"),
                ("DATABASE_SERVER_FULL", "postgres://zad"),
            ])),
            Some("postgres://local".into())
        );
        assert_eq!(
            database_url(env(&[
                ("DATABASE_URL", " "),
                ("DATABASE_SERVER_FULL", "postgres://zad"),
            ])),
            Some("postgres://zad".into())
        );
    }

    #[test]
    fn an_identifier_is_quoted() {
        assert_eq!(quote_identifier("wetsproces"), "\"wetsproces\"");
        assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
    }
}
