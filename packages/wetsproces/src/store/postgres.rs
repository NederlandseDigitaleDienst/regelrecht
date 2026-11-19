//! The chronicle store in PostgreSQL: chronicles that survive a restart.
//!
//! Everything lives in the schema [`SCHEMA`] of the database it is given,
//! the migration table included, next to whatever else that database holds
//! and apart from it. The migrations in `packages/wetsproces/migrations/`
//! make three tables:
//!
//! - `grams`: one row per record, keyed by cell and sequence number (1, 2,
//!   ...), with the record as JSON, the [`crate::store::FORMAT_VERSION`] it was written in
//!   and when it was stored. A trigger refuses every `UPDATE`, `DELETE` and
//!   `TRUNCATE`: a chronicle only grows;
//! - `heads`: per cell its head, the number of records in it. A commit locks
//!   the head rows of its cells, so commits on one cell take turns;
//! - `receipts`: per idempotency key what the request under it answered.
//!
//! A commit is one transaction. It first claims its key in `receipts`, so a
//! second request under the same key waits for the first and then replays
//! it; then it locks the heads of its cells in one fixed order (by name),
//! so two commits never wait for each other in a circle; then it checks
//! every head, appends the records and moves the heads.

use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use regelrecht_cel::Gram;
use regelrecht_shared::postgres::migrate_locked;
use serde_json::Value;
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::store::{
    Appended, CellAppend, ChronicleStore, Commit, FORMAT_VERSION, IdempotencyKey, Keyed, Receipt,
    StoreError, StoredChronicle,
};

/// The schema the store keeps its tables in.
pub const SCHEMA: &str = "wetsproces";

/// The advisory lock that keeps two instances from migrating at once.
/// Another key than the pipeline's, so neither holds the other up for the
/// whole of its run; sqlx's own lock per database inside `Migrator::run`
/// still makes the two runs take turns while they migrate.
const MIGRATION_LOCK_KEY: i64 = 0x5745_5453_5052_4F43; // "WETSPROC"

/// Connections per instance: a request holds one for a read or a commit.
const MAX_CONNECTIONS: u32 = 10;

/// How long a connection may take before the store gives up: long enough
/// for a database that is starting next to the server.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// The chronicles in PostgreSQL.
#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl std::fmt::Debug for PostgresStore {
    // The pool's options hold the password.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresStore")
            .field("schema", &SCHEMA)
            .finish_non_exhaustive()
    }
}

fn backend(context: &str) -> impl FnOnce(sqlx::Error) -> StoreError + '_ {
    move |e| StoreError::Backend(format!("{context}: {e}"))
}

impl PostgresStore {
    /// Connect to the database at `url` and bring the schema [`SCHEMA`] up
    /// to date.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the URL does not parse, the database
    /// cannot be reached, or a migration fails.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        Self::connect_within(url, CONNECT_TIMEOUT).await
    }

    /// [`PostgresStore::connect`], giving up on a connection after `timeout`.
    ///
    /// # Errors
    ///
    /// As [`PostgresStore::connect`].
    pub async fn connect_within(url: &str, timeout: Duration) -> Result<Self, StoreError> {
        let options: PgConnectOptions = url
            .parse()
            .map_err(|e| StoreError::Backend(format!("the database URL does not parse: {e}")))?;
        // Every connection looks in the schema first, so sqlx's own
        // migration table lands there too and not in `public`.
        let options = options.options([("search_path", SCHEMA)]);
        let pool = PgPoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .acquire_timeout(timeout)
            .connect_with(options)
            .await
            .map_err(backend("connecting to the database"))?;
        migrate_locked(&pool, MIGRATION_LOCK_KEY, &MIGRATOR, Some(SCHEMA))
            .await
            .map_err(|e| StoreError::Backend(format!("migrating the schema {SCHEMA}: {e}")))?;
        Ok(Self { pool })
    }

    /// Close the connections; for a test that starts over on the same
    /// database.
    #[cfg(test)]
    async fn close(&self) {
        self.pool.close().await;
    }

    #[cfg(test)]
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// A count or head as the database holds it.
fn to_db(n: u64) -> Result<i64, StoreError> {
    i64::try_from(n).map_err(|_| StoreError::Backend(format!("{n} does not fit the database")))
}

/// A count or head as the database gave it.
fn from_db(n: i64) -> Result<u64, StoreError> {
    u64::try_from(n)
        .map_err(|_| StoreError::Backend(format!("the database holds {n}, which is no count")))
}

/// One stored row, read back into a record.
fn read_gram(cell: &str, seq: u64, version: i32, json: Value) -> Result<Gram, StoreError> {
    if version != FORMAT_VERSION {
        return Err(StoreError::UnknownFormat {
            cell: cell.to_string(),
            seq,
            version,
        });
    }
    serde_json::from_value(json).map_err(|e| {
        StoreError::Backend(format!(
            "record {seq} of '{cell}' does not read as a gram: {e}"
        ))
    })
}

#[async_trait]
impl ChronicleStore for PostgresStore {
    async fn load(&self, cell: &str) -> Result<StoredChronicle, StoreError> {
        let mut all = self.load_all(&[cell]).await?;
        all.pop()
            .ok_or_else(|| StoreError::Backend("no chronicle for the one cell asked".into()))
    }

    async fn load_all(&self, cells: &[&str]) -> Result<Vec<StoredChronicle>, StoreError> {
        // One statement, so one snapshot: every cell as it stood at once.
        let rows = sqlx::query(
            "SELECT cell, seq, format_version, gram FROM wetsproces.grams \
             WHERE cell = ANY($1) ORDER BY cell, seq",
        )
        .bind(cells)
        .fetch_all(&self.pool)
        .await
        .map_err(backend("reading the chronicles"))?;
        let mut by_cell: BTreeMap<String, StoredChronicle> = BTreeMap::new();
        for row in &rows {
            let cell: String = column(row, "cell")?;
            let seq = from_db(column(row, "seq")?)?;
            let gram = read_gram(
                &cell,
                seq,
                column(row, "format_version")?,
                column(row, "gram")?,
            )?;
            let chronicle = by_cell.entry(cell.clone()).or_default();
            if seq != chronicle.head() + 1 {
                return Err(StoreError::Backend(format!(
                    "the chronicle of '{cell}' has record {seq} after {}",
                    chronicle.head()
                )));
            }
            chronicle.grams.push(gram);
        }
        Ok(cells
            .iter()
            .map(|cell| by_cell.get(*cell).cloned().unwrap_or_default())
            .collect())
    }

    async fn receipt(&self, key: &IdempotencyKey) -> Result<Option<Receipt>, StoreError> {
        let row = sqlx::query("SELECT request, response FROM wetsproces.receipts WHERE key = $1")
            .bind(key.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(backend("reading a receipt"))?;
        row.as_ref().map(read_receipt).transpose()
    }

    async fn append(&self, commit: Commit) -> Result<Appended, StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(backend("starting a commit"))?;
        if let Some(keyed) = &commit.keyed
            && let Some(receipt) = claim_key(&mut tx, keyed).await?
        {
            tx.rollback()
                .await
                .map_err(backend("rolling back a replay"))?;
            return Ok(Appended::Replayed(receipt));
        }
        let heads = lock_heads(&mut tx, &commit.appends).await?;
        check_heads(&heads, &commit.appends)?;
        write_appends(&mut tx, heads, commit.appends).await?;
        tx.commit().await.map_err(backend("committing"))?;
        Ok(Appended::Committed)
    }
}

/// The unique constraint on `grams.gram_id`, by the name Postgres gives it.
const GRAM_ID_CONSTRAINT: &str = "grams_gram_id_key";

/// Whether `e` is the refusal of a record whose id is already stored.
fn is_duplicate_gram_id(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .is_some_and(|db| db.is_unique_violation() && db.constraint() == Some(GRAM_ID_CONSTRAINT))
}

/// The column `name` of `row`, as a `T`.
fn column<'r, T>(row: &'r PgRow, name: &str) -> Result<T, StoreError>
where
    T: sqlx::Decode<'r, Postgres> + sqlx::Type<Postgres>,
{
    row.try_get(name)
        .map_err(|e| StoreError::Backend(format!("reading the column {name}: {e}")))
}

/// A row of `receipts` as a receipt.
fn read_receipt(row: &PgRow) -> Result<Receipt, StoreError> {
    Ok(Receipt {
        request: column(row, "request")?,
        response: column(row, "response")?,
    })
}

/// Claim `keyed`'s key for this commit by storing its receipt. When the key
/// was used before, nothing is stored and the earlier receipt comes back.
/// A commit under the same key still in progress elsewhere makes this wait
/// until it ends: committed, it is replayed; rolled back, this one claims
/// the key.
async fn claim_key(
    tx: &mut Transaction<'_, Postgres>,
    keyed: &Keyed,
) -> Result<Option<Receipt>, StoreError> {
    let claimed = sqlx::query(
        "INSERT INTO wetsproces.receipts (key, request, response) VALUES ($1, $2, $3) \
         ON CONFLICT (key) DO NOTHING",
    )
    .bind(keyed.key.as_str())
    .bind(&keyed.receipt.request)
    .bind(&keyed.receipt.response)
    .execute(&mut **tx)
    .await
    .map_err(backend("storing a receipt"))?
    .rows_affected();
    if claimed == 1 {
        return Ok(None);
    }
    let row = sqlx::query("SELECT request, response FROM wetsproces.receipts WHERE key = $1")
        .bind(keyed.key.as_str())
        .fetch_one(&mut **tx)
        .await
        .map_err(backend("reading the receipt of a used key"))?;
    read_receipt(&row).map(Some)
}

/// Lock the head of every cell in `appends`, in the order of their names,
/// and give each one's head. A cell without a row gets one at 0 first.
async fn lock_heads(
    tx: &mut Transaction<'_, Postgres>,
    appends: &[CellAppend],
) -> Result<BTreeMap<String, u64>, StoreError> {
    let mut cells: Vec<&str> = appends.iter().map(|a| a.cell.as_str()).collect();
    cells.sort_unstable();
    cells.dedup();
    if cells.is_empty() {
        return Ok(BTreeMap::new());
    }
    // Both statements go through the cells in the same order (byte order,
    // as `sort_unstable` above), so commits never wait in a circle.
    sqlx::query(
        "INSERT INTO wetsproces.heads (cell, head) \
         SELECT cell, 0 FROM unnest($1::text[]) AS cell ORDER BY cell COLLATE \"C\" \
         ON CONFLICT (cell) DO NOTHING",
    )
    .bind(&cells)
    .execute(&mut **tx)
    .await
    .map_err(backend("adding heads"))?;
    let rows = sqlx::query(
        "SELECT cell, head FROM wetsproces.heads WHERE cell = ANY($1) \
         ORDER BY cell COLLATE \"C\" FOR UPDATE",
    )
    .bind(&cells)
    .fetch_all(&mut **tx)
    .await
    .map_err(backend("locking heads"))?;
    rows.iter()
        .map(|row| Ok((column(row, "cell")?, from_db(column(row, "head")?)?)))
        .collect()
}

/// Refuse the commit when a head moved since it was read, at the first
/// append in the commit's order whose head did, as the store in memory does.
fn check_heads(heads: &BTreeMap<String, u64>, appends: &[CellAppend]) -> Result<(), StoreError> {
    for append in appends {
        let actual = heads.get(&append.cell).copied().unwrap_or(0);
        if actual != append.expected_head {
            return Err(StoreError::Conflict {
                cell: append.cell.clone(),
                expected: append.expected_head,
                actual,
            });
        }
    }
    Ok(())
}

/// Append every record after its cell's head and move the heads.
async fn write_appends(
    tx: &mut Transaction<'_, Postgres>,
    mut heads: BTreeMap<String, u64>,
    appends: Vec<CellAppend>,
) -> Result<(), StoreError> {
    let mut moved = Vec::new();
    for append in appends {
        let head = heads.entry(append.cell.clone()).or_insert(0);
        for gram in &append.grams {
            *head += 1;
            sqlx::query(
                "INSERT INTO wetsproces.grams (cell, seq, gram_id, format_version, gram) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(&append.cell)
            .bind(to_db(*head)?)
            .bind(&gram.id)
            .bind(FORMAT_VERSION)
            .bind(Json(gram))
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                if is_duplicate_gram_id(&e) {
                    StoreError::DuplicateGram {
                        id: gram.id.clone(),
                    }
                } else {
                    StoreError::Backend(format!(
                        "appending record {} '{}' to '{}': {e}",
                        *head, gram.id, append.cell
                    ))
                }
            })?;
        }
        if !append.grams.is_empty() {
            moved.push(append.cell);
        }
    }
    moved.sort_unstable();
    moved.dedup();
    for cell in moved {
        let head = heads.get(&cell).copied().unwrap_or(0);
        sqlx::query("UPDATE wetsproces.heads SET head = $2 WHERE cell = $1")
            .bind(&cell)
            .bind(to_db(head)?)
            .execute(&mut **tx)
            .await
            .map_err(backend("moving a head"))?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::Arc;

    use regelrecht_shared::postgres::test_utils::TestDatabase;

    use super::*;
    use crate::store::contract::{self, append, commit, key, unkeyed};

    /// A store on a database of its own, kept alive with it.
    struct Fixture {
        store: PostgresStore,
        database: TestDatabase,
    }

    impl Fixture {
        async fn new() -> Self {
            let database = TestDatabase::create().await;
            let store = PostgresStore::connect(database.url()).await.unwrap();
            Self { store, database }
        }

        fn store(&self) -> &dyn ChronicleStore {
            &self.store
        }

        async fn count(&self, sql: &str) -> i64 {
            sqlx::query_scalar(sql)
                .fetch_one(self.store.pool())
                .await
                .unwrap()
        }
    }

    contract::contract_tests!(Fixture::new());

    #[tokio::test]
    async fn the_tables_live_in_their_own_schema() {
        let f = Fixture::new().await;
        let tables = |schema: &'static str| {
            let pool = f.store.pool().clone();
            async move {
                let names: Vec<String> = sqlx::query_scalar(
                    "SELECT table_name::text FROM information_schema.tables \
                     WHERE table_schema = $1 ORDER BY table_name",
                )
                .bind(schema)
                .fetch_all(&pool)
                .await
                .unwrap();
                names
            }
        };
        assert_eq!(
            tables("wetsproces").await,
            ["_sqlx_migrations", "grams", "heads", "receipts"]
        );
        assert!(tables("public").await.is_empty());
    }

    #[tokio::test]
    async fn without_a_key_nothing_is_kept_to_replay() {
        let f = Fixture::new().await;
        let c = unkeyed(vec![append("a", 0, &["1"])]);
        assert_eq!(f.store.append(c).await.unwrap(), Appended::Committed);
        assert_eq!(f.count("SELECT count(*) FROM wetsproces.receipts").await, 0);
    }

    #[tokio::test]
    async fn stored_records_cannot_be_changed_or_removed() {
        let f = Fixture::new().await;
        f.store
            .append(unkeyed(vec![append("a", 0, &["1"])]))
            .await
            .unwrap();
        for sql in [
            "UPDATE wetsproces.grams SET gram = '{}'::jsonb",
            "DELETE FROM wetsproces.grams",
            "TRUNCATE wetsproces.grams CASCADE",
        ] {
            let e = sqlx::query(sql)
                .execute(f.store.pool())
                .await
                .unwrap_err()
                .to_string();
            assert!(e.contains("append-only"), "{sql}: {e}");
        }
        assert_eq!(f.store.load("a").await.unwrap().head(), 1);
    }

    #[tokio::test]
    async fn the_chronicles_survive_a_restart() {
        let f = Fixture::new().await;
        f.store
            .append(commit("k1", vec![append("a", 0, &["1", "2"])]))
            .await
            .unwrap();
        f.store.close().await;
        // A new instance on the same database: the migrations find nothing
        // left to do and the chronicle is as it was.
        let again = PostgresStore::connect(f.database.url()).await.unwrap();
        let a = again.load("a").await.unwrap();
        assert_eq!(
            a.grams,
            vec![contract::gram("1", "a"), contract::gram("2", "a")]
        );
        assert!(again.receipt(&key("k1")).await.unwrap().is_some());
        let next = commit("k2", vec![append("a", 2, &["3"])]);
        assert_eq!(again.append(next).await.unwrap(), Appended::Committed);
    }

    #[tokio::test]
    async fn a_record_in_an_unknown_format_is_refused() {
        let f = Fixture::new().await;
        f.store
            .append(unkeyed(vec![append("a", 0, &["1"])]))
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO wetsproces.grams (cell, seq, gram_id, format_version, gram) \
             VALUES ('a', 2, '2', 99, '{\"anders\": true}'::jsonb)",
        )
        .execute(f.store.pool())
        .await
        .unwrap();
        let e = f.store.load("a").await.unwrap_err();
        assert_eq!(
            e,
            StoreError::UnknownFormat {
                cell: "a".into(),
                seq: 2,
                version: 99
            }
        );
        assert!(e.to_string().contains("only format 1"), "{e}");
    }

    #[tokio::test]
    async fn the_service_runs_a_step_on_postgres_and_reads_it_back_after_a_restart() {
        use crate::clock::ControllableClock;
        use crate::service::Service;

        let f = Fixture::new().await;
        let start = |store: PostgresStore| async move {
            let (corpus, _) =
                crate::case::load(std::path::Path::new(crate::config::DEFAULT_MANIFEST)).unwrap();
            let clock = Arc::new(ControllableClock::new(corpus.casus.speeldatum));
            Service::start(corpus, Arc::new(store), clock)
                .await
                .unwrap()
        };
        let service = start(f.store.clone()).await;
        let before = service.chronicle("gemeente_wilgendaal").await.unwrap();
        service
            .perform(Some(key("aanvraag-1")), crate::service::tests::submission())
            .await
            .unwrap();
        let after = service.chronicle("gemeente_wilgendaal").await.unwrap();
        assert_eq!(after["kop"], before["kop"].as_u64().unwrap() + 1);
        f.store.close().await;

        let again = start(PostgresStore::connect(f.database.url()).await.unwrap()).await;
        assert_eq!(again.chronicle("gemeente_wilgendaal").await.unwrap(), after);
        // The beginning is not recorded twice, and the forms still derive
        // from what was read back.
        again.forms().await.unwrap();
    }
}
