//! The chronicle store in PostgreSQL: chronicles that survive a restart.
//!
//! Everything lives in the schema [`SCHEMA`] of the database it is given,
//! the migration table included, next to whatever else that database holds
//! and apart from it. The migrations in `packages/regelhet/migrations/`
//! make three tables for the chronicles (and two for FSC voor Burgers,
//! [`crate::fsc4b::store`]):
//!
//! - `grams`: one row per record, keyed by cell and sequence number (1, 2,
//!   ...), with the record as JSON, the [`crate::store::FORMAT_VERSION`] it was written in,
//!   when it was stored and who asked for it (`recorded_by`, migration 0002)
//!   by which channel (`recorded_via`, migrations 0003, 0006 and 0009). A trigger refuses every `UPDATE`, `DELETE` and
//!   `TRUNCATE`: a chronicle only grows, except that a reset of test mode
//!   may empty it (migrations 0007 and 0008);
//! - `heads`: per cell its head, the number of records in it. A commit locks
//!   the head rows of its cells, so commits on one cell take turns;
//! - `receipts`: per idempotency key what the request under it answered;
//! - `test_clock` (migration 0005): the day the test clock was set to;
//! - `generation` (migration 0008): how often a reset of test mode emptied
//!   the store; a commit names the generation it read in;
//! - `stable` (migration 0008): set once a server in stable mode used the
//!   database ([`PostgresStore::mark_stable`]); from then on no reset gets
//!   past the triggers.
//!
//! A commit is one transaction. It first claims its key in `receipts`, so a
//! second request under the same key waits for the first and then replays
//! it; then it locks the heads of its cells in one fixed order (by name),
//! so two commits never wait for each other in a circle; then it checks
//! every head and the generation, appends the records and moves the heads.

use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use regelrecht_cel::Gram;
use regelrecht_shared::postgres::migrate_locked;
use serde_json::Value;
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::store::{
    Actor, Appended, CellAppend, Channel, ChronicleStore, Commit, FORMAT_VERSION, IdempotencyKey,
    Keyed, Receipt, StoreError, StoredChronicle, StoredGram,
};

/// The schema the store keeps its tables in.
pub const SCHEMA: &str = "regelhet";

/// The advisory lock that keeps two instances from migrating at once.
/// Another key than the pipeline's, so neither holds the other up for the
/// whole of its run; sqlx's own lock per database inside `Migrator::run`
/// still makes the two runs take turns while they migrate.
pub(crate) const MIGRATION_LOCK_KEY: i64 = 0x5745_5453_5052_4F43; // "WETSPROC"

/// How many connections an instance may hold to the database at most, all
/// in one pool that the chronicles, the sessions and FSC voor Burgers share.
/// A request holds one for a read or a commit; a commit does all its work on
/// the one connection of its transaction, however many cells it locks.
/// Starting takes two at once: one holds the advisory lock of the migration
/// ([`MIGRATION_LOCK_KEY`]) while the migrator runs on another. Several
/// components share one database role on a platform such as ZAD, each with
/// a pool of its own, so the default is low.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSize(u32);

impl PoolSize {
    /// The fewest connections that let the migration run.
    pub const MIN: u32 = 2;

    /// The number of connections.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Default for PoolSize {
    fn default() -> Self {
        Self(5)
    }
}

impl TryFrom<u32> for PoolSize {
    type Error = String;

    fn try_from(n: u32) -> Result<Self, Self::Error> {
        if n < Self::MIN {
            return Err(format!(
                "{n} connections cannot run the migration, which holds a lock on one connection \
                 while it migrates on another; give at least {}",
                Self::MIN
            ));
        }
        Ok(Self(n))
    }
}

impl std::str::FromStr for PoolSize {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n: u32 = s
            .trim()
            .parse()
            .map_err(|_| format!("`{s}` is no number of connections"))?;
        Self::try_from(n)
    }
}

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
        Self::connect_within(url, CONNECT_TIMEOUT, PoolSize::default()).await
    }

    /// [`PostgresStore::connect`] with at most `size` connections, giving up
    /// on a connection after `timeout`.
    ///
    /// # Errors
    ///
    /// As [`PostgresStore::connect`].
    pub async fn connect_within(
        url: &str,
        timeout: Duration,
        size: PoolSize,
    ) -> Result<Self, StoreError> {
        let options: PgConnectOptions = url
            .parse()
            .map_err(|e| StoreError::Backend(format!("the database URL does not parse: {e}")))?;
        // Every connection looks in the schema first, so sqlx's own
        // migration table lands there too and not in `public`.
        let options = options.options([("search_path", SCHEMA)]);
        let pool = PgPoolOptions::new()
            .max_connections(size.get())
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

    /// Record that a server in stable mode uses this database. From then on
    /// the triggers of the chronicles refuse every reset (migration 0008),
    /// also from a server in test mode that is pointed here by mistake.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the store fails.
    pub async fn mark_stable(&self) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO regelhet.stable DEFAULT VALUES ON CONFLICT DO NOTHING")
            .execute(&self.pool)
            .await
            .map_err(backend("marking the database as stable"))?;
        Ok(())
    }

    /// The connections, with [`SCHEMA`] first in their search path; the
    /// session store of the login shares them.
    pub fn pool(&self) -> &PgPool {
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
        // One snapshot for both statements: every cell as it stood at once,
        // in the generation it stood in.
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .await
            .map_err(backend("starting a read"))?;
        let generation = read_generation(&mut tx).await?;
        let rows = sqlx::query(
            "SELECT cell, seq, format_version, gram, recorded_by, recorded_via FROM regelhet.grams \
             WHERE cell = ANY($1) ORDER BY cell, seq",
        )
        .bind(cells)
        .fetch_all(&mut *tx)
        .await
        .map_err(backend("reading the chronicles"))?;
        tx.commit().await.map_err(backend("ending a read"))?;
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
            chronicle.grams.push(StoredGram {
                gram,
                recorded_by: column(row, "recorded_by")?,
                recorded_via: channel(column(row, "recorded_via")?)?,
            });
        }
        Ok(cells
            .iter()
            .map(|cell| StoredChronicle {
                generation,
                ..by_cell.get(*cell).cloned().unwrap_or_default()
            })
            .collect())
    }

    async fn receipt(&self, key: &IdempotencyKey) -> Result<Option<Receipt>, StoreError> {
        let row = sqlx::query("SELECT request, response FROM regelhet.receipts WHERE key = $1")
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
        // After the heads, so a reset under way has finished; shared, so
        // the next one waits for this commit. Before the heads are compared,
        // as the store in memory does: after a reset the answer is a reset.
        let actual = read_generation(&mut tx).await?;
        if actual != commit.generation {
            return Err(StoreError::Reset {
                expected: commit.generation,
                actual,
            });
        }
        check_heads(&heads, &commit.appends)?;
        write_appends(&mut tx, heads, &commit.appends, |gram| {
            commit.actor_of(gram)
        })
        .await?;
        tx.commit().await.map_err(backend("committing"))?;
        Ok(Appended::Committed)
    }

    async fn test_day(&self) -> Result<Option<NaiveDate>, StoreError> {
        // As text, so the store needs no date type of sqlx's.
        let day: Option<String> = sqlx::query_scalar("SELECT day::text FROM regelhet.test_clock")
            .fetch_optional(&self.pool)
            .await
            .map_err(backend("reading the test clock"))?;
        day.map(|d| {
            d.parse().map_err(|e| {
                StoreError::Backend(format!("the test clock holds '{d}', which is no day: {e}"))
            })
        })
        .transpose()
    }

    async fn set_test_day(&self, day: NaiveDate) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO regelhet.test_clock (day) VALUES ($1::date) \
             ON CONFLICT (singleton) DO UPDATE SET day = EXCLUDED.day, set_at = now()",
        )
        .bind(day.to_string())
        .execute(&self.pool)
        .await
        .map_err(backend("setting the test clock"))?;
        Ok(())
    }

    async fn reset(&self, begin: Vec<CellAppend>) -> Result<(), StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(backend("starting a reset"))?;
        // Every table at once, in the order a commit takes them: the kept
        // answers first (a commit with a key claims it there before its
        // heads), then the heads, the generation and the records. A read
        // takes the generation before the records too, so neither waits on
        // the other in a circle. A commit under way finishes before the
        // reset; one after it names the old generation and is refused.
        sqlx::query(
            "LOCK TABLE regelhet.receipts, regelhet.heads, regelhet.generation, \
             regelhet.grams, regelhet.test_clock IN ACCESS EXCLUSIVE MODE",
        )
        .execute(&mut *tx)
        .await
        .map_err(backend("locking the chronicles for a reset"))?;
        // The one transaction in which the triggers of the chronicles let a
        // TRUNCATE through (migration 0007).
        sqlx::query("SELECT set_config('regelhet.leegmaken', 'testomgeving', true)")
            .execute(&mut *tx)
            .await
            .map_err(backend("opening the chronicles for a reset"))?;
        sqlx::query(
            "TRUNCATE regelhet.grams, regelhet.heads, regelhet.receipts, \
             regelhet.test_clock",
        )
        .execute(&mut *tx)
        .await
        .map_err(backend("emptying the chronicles"))?;
        // A step read before this point names the old generation, and is
        // refused when it commits after it, wherever the heads then stand.
        sqlx::query("UPDATE regelhet.generation SET generation = generation + 1")
            .execute(&mut *tx)
            .await
            .map_err(backend("starting a new generation"))?;
        // What the case begins with, in the same transaction: no request
        // finds the store empty in between.
        let heads = lock_heads(&mut tx, &begin).await?;
        write_appends(&mut tx, heads, &begin, |_| None).await?;
        tx.commit().await.map_err(backend("committing a reset"))?;
        Ok(())
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

/// The channel a row names in `recorded_via`.
fn channel(name: Option<String>) -> Result<Option<Channel>, StoreError> {
    name.map(|n| {
        Channel::from_name(&n)
            .ok_or_else(|| StoreError::Backend(format!("the database holds the channel '{n}'")))
    })
    .transpose()
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
        "INSERT INTO regelhet.receipts (key, request, response) VALUES ($1, $2, $3) \
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
    let row = sqlx::query("SELECT request, response FROM regelhet.receipts WHERE key = $1")
        .bind(keyed.key.as_str())
        .fetch_one(&mut **tx)
        .await
        .map_err(backend("reading the receipt of a used key"))?;
    read_receipt(&row).map(Some)
}

/// The generation of the store, held shared until `tx` ends.
async fn read_generation(tx: &mut Transaction<'_, Postgres>) -> Result<u64, StoreError> {
    let generation: i64 = sqlx::query_scalar("SELECT generation FROM regelhet.generation")
        .fetch_one(&mut **tx)
        .await
        .map_err(backend("reading the generation"))?;
    from_db(generation)
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
        "INSERT INTO regelhet.heads (cell, head) \
         SELECT cell, 0 FROM unnest($1::text[]) AS cell ORDER BY cell COLLATE \"C\" \
         ON CONFLICT (cell) DO NOTHING",
    )
    .bind(&cells)
    .execute(&mut **tx)
    .await
    .map_err(backend("adding heads"))?;
    let rows = sqlx::query(
        "SELECT cell, head FROM regelhet.heads WHERE cell = ANY($1) \
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

/// Append every record of `appends` after its cell's head, with who made
/// it (`actor_of`, such as [`Commit::actor_of`]), and move the heads.
async fn write_appends(
    tx: &mut Transaction<'_, Postgres>,
    mut heads: BTreeMap<String, u64>,
    appends: &[CellAppend],
    actor_of: impl Fn(&Gram) -> Option<Actor>,
) -> Result<(), StoreError> {
    let mut moved = Vec::new();
    for append in appends {
        let head = heads.entry(append.cell.clone()).or_insert(0);
        for gram in &append.grams {
            *head += 1;
            let actor = actor_of(gram);
            sqlx::query(
                "INSERT INTO regelhet.grams \
                 (cell, seq, gram_id, format_version, gram, recorded_by, recorded_via) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&append.cell)
            .bind(to_db(*head)?)
            .bind(&gram.id)
            .bind(FORMAT_VERSION)
            .bind(Json(gram))
            .bind(actor.as_ref().map(|a| a.account.as_str()))
            .bind(actor.as_ref().map(|a| a.channel.as_str()))
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
            moved.push(append.cell.clone());
        }
    }
    moved.sort_unstable();
    moved.dedup();
    for cell in moved {
        let head = heads.get(&cell).copied().unwrap_or(0);
        sqlx::query("UPDATE regelhet.heads SET head = $2 WHERE cell = $1")
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
            tables("regelhet").await,
            [
                "_sqlx_migrations",
                "fsc4b_challenges",
                "fsc4b_sessions",
                "generation",
                "grams",
                "heads",
                "receipts",
                "stable",
                "test_clock"
            ]
        );
        assert!(tables("public").await.is_empty());
    }

    #[tokio::test]
    async fn without_a_key_nothing_is_kept_to_replay() {
        let f = Fixture::new().await;
        let c = unkeyed(vec![append("a", 0, &["1"])]);
        assert_eq!(f.store.append(c).await.unwrap(), Appended::Committed);
        assert_eq!(f.count("SELECT count(*) FROM regelhet.receipts").await, 0);
    }

    #[tokio::test]
    async fn stored_records_cannot_be_changed_or_removed() {
        let f = Fixture::new().await;
        f.store
            .append(unkeyed(vec![append("a", 0, &["1"])]))
            .await
            .unwrap();
        for sql in [
            "UPDATE regelhet.grams SET gram = '{}'::jsonb",
            "DELETE FROM regelhet.grams",
            "TRUNCATE regelhet.grams CASCADE",
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

    /// The smallest pool [`PoolSize`] allows.
    fn smallest() -> PoolSize {
        PoolSize::try_from(PoolSize::MIN).unwrap()
    }

    #[test]
    fn a_pool_holds_at_least_the_connections_the_migration_needs() {
        assert_eq!(PoolSize::default().get(), 5);
        assert_eq!("3".parse::<PoolSize>().unwrap().get(), 3);
        for wrong in ["0", "1", "veel", "-2", ""] {
            assert!(wrong.parse::<PoolSize>().is_err(), "{wrong}");
        }
    }

    #[tokio::test]
    async fn instances_with_the_smallest_pool_migrate_at_once_without_waiting_forever() {
        let database = TestDatabase::create().await;
        let url = database.url().to_string();
        // Three instances start together; each holds the migration lock on
        // one connection while its migrator runs on the other.
        let starts = (0..3).map(|_| {
            let url = url.clone();
            tokio::spawn(async move {
                PostgresStore::connect_within(&url, Duration::from_secs(20), smallest()).await
            })
        });
        let started =
            tokio::time::timeout(Duration::from_secs(60), futures::future::join_all(starts))
                .await
                .expect("three instances should start within a minute");
        for store in started {
            store.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn commits_over_several_cells_do_not_starve_the_smallest_pool() {
        let database = TestDatabase::create().await;
        let store = Arc::new(
            PostgresStore::connect_within(database.url(), Duration::from_secs(20), smallest())
                .await
                .unwrap(),
        );
        // Sixteen writers on two connections. Half write two cells of their
        // own; the other half fight over the same two cells and retry on a
        // conflict, so commits lock several cells and wait on each other.
        let writers = (0..16).map(|i| {
            let store = Arc::clone(&store);
            tokio::spawn(async move {
                let cells = if i % 2 == 0 {
                    [format!("eigen-{i}-a"), format!("eigen-{i}-b")]
                } else {
                    ["gedeeld-a".to_string(), "gedeeld-b".to_string()]
                };
                let id = |c: &str| format!("{c}-{i}");
                loop {
                    let heads = store
                        .load_all(&[cells[0].as_str(), cells[1].as_str()])
                        .await
                        .unwrap();
                    let appends = cells
                        .iter()
                        .zip(&heads)
                        .map(|(cell, h)| append(cell, h.head(), &[id(cell).as_str()]))
                        .collect();
                    match store.append(unkeyed(appends)).await {
                        Ok(_) => break,
                        Err(StoreError::Conflict { .. }) => continue,
                        Err(e) => panic!("writer {i}: {e}"),
                    }
                }
            })
        });
        tokio::time::timeout(Duration::from_secs(60), futures::future::join_all(writers))
            .await
            .expect("sixteen writers on two connections should finish within a minute")
            .into_iter()
            .for_each(|w| w.unwrap());
        let shared = store.load("gedeeld-a").await.unwrap();
        assert_eq!(shared.into_grams().len(), 8);
    }

    #[tokio::test]
    async fn without_its_own_setting_a_transaction_still_cannot_truncate_the_chronicles() {
        let f = Fixture::new().await;
        f.store
            .append(commit("k1", vec![append("a", 0, &["1"])]))
            .await
            .unwrap();
        let e = sqlx::query("TRUNCATE regelhet.grams, regelhet.heads")
            .execute(f.store.pool())
            .await
            .unwrap_err();
        assert!(e.to_string().contains("append-only"), "{e}");
        assert_eq!(f.count("SELECT count(*) FROM regelhet.grams").await, 1);
    }

    #[tokio::test]
    async fn a_reset_amid_commits_leaves_no_commit_half() {
        let f = Fixture::new().await;
        let store = Arc::new(f.store.clone());
        // Commits over two cells each, while a reset runs among them: every
        // commit lands whole or not at all, so both cells always hold as
        // many records as each other.
        // Half the writers commit under a key, as a submission does: they
        // claim it among the kept answers before they lock their heads.
        let writers = (0..8).map(|i| {
            let store = Arc::clone(&store);
            tokio::spawn(async move {
                let heads = store.load_all(&["a", "b"]).await.unwrap();
                let a = format!("a-{i}");
                let b = format!("b-{i}");
                let appends = vec![
                    append("a", heads[0].head(), &[a.as_str()]),
                    append("b", heads[1].head(), &[b.as_str()]),
                ];
                let c = if i % 2 == 0 {
                    commit(&format!("k{i}"), appends)
                } else {
                    unkeyed(appends)
                };
                // A commit that lands before the reset, or that a reset
                // overtook, is fine; any other failure is not.
                match store.append(c).await {
                    Ok(_) | Err(StoreError::Conflict { .. } | StoreError::Reset { .. }) => {}
                    Err(e) => panic!("writer {i}: {e}"),
                }
            })
        });
        // Readers too, which take the generation before the records: a
        // reset must not wait on them in a circle.
        let readers = (0..8).map(|_| {
            let store = Arc::clone(&store);
            tokio::spawn(async move {
                let heads = store.load_all(&["a", "b"]).await.unwrap();
                assert_eq!(heads[0].generation, heads[1].generation);
            })
        });
        let reset = {
            let store = Arc::clone(&store);
            tokio::spawn(async move { store.reset(Vec::new()).await.unwrap() })
        };
        futures::future::join_all(writers).await;
        for reader in futures::future::join_all(readers).await {
            reader.unwrap();
        }
        reset.await.unwrap();
        let heads = store.load_all(&["a", "b"]).await.unwrap();
        assert_eq!(heads[0].head(), heads[1].head());
    }

    #[tokio::test]
    async fn a_database_that_stable_mode_used_is_never_reset() {
        let f = Fixture::new().await;
        f.store
            .append(commit("k1", vec![append("a", 0, &["1"])]))
            .await
            .unwrap();
        // Before the mark a reset goes through; after it, not even the
        // store's own reset gets past the triggers.
        f.store.reset(Vec::new()).await.unwrap();
        f.store.mark_stable().await.unwrap();
        f.store.mark_stable().await.unwrap();
        let mut c = commit("k2", vec![append("a", 0, &["2"])]);
        c.generation = 1;
        f.store.append(c).await.unwrap();
        let e = f.store.reset(Vec::new()).await.unwrap_err();
        assert!(e.to_string().contains("append-only"), "{e}");
        assert_eq!(f.store.load("a").await.unwrap().head(), 1);
        assert!(f.store.receipt(&key("k2")).await.unwrap().is_some());
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
            a.into_grams(),
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
            "INSERT INTO regelhet.grams (cell, seq, gram_id, format_version, gram) \
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
            let store: Arc<dyn ChronicleStore> = Arc::new(store);
            let clock = Arc::new(ControllableClock::new(
                Arc::clone(&store),
                corpus.casus.speeldatum,
            ));
            Service::start(corpus, store, clock).await.unwrap()
        };
        let service = start(f.store.clone()).await;
        let before = service.chronicle("gemeente_wilgendaal").await.unwrap();
        service
            .perform(
                Some(key("aanvraag-1")),
                crate::service::tests::submission(),
                &crate::service::tests::actor(),
            )
            .await
            .unwrap();
        let after = service.chronicle("gemeente_wilgendaal").await.unwrap();
        // The application and the request for an examination it brought
        // about.
        assert_eq!(after["kop"], before["kop"].as_u64().unwrap() + 2);
        f.store.close().await;

        let again = start(PostgresStore::connect(f.database.url()).await.unwrap()).await;
        assert_eq!(again.chronicle("gemeente_wilgendaal").await.unwrap(), after);
        // The beginning is not recorded twice, and the forms still derive
        // from what was read back.
        again.forms().await.unwrap();
    }
}
