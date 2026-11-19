//! Where the chronicles of the cells live between requests.
//!
//! The service talks to a [`ChronicleStore`] and nothing else, so the
//! storage can change without the service changing. The trait is shaped for
//! a database as much as for memory:
//!
//! - **per cell**: a chronicle is loaded by the cell's id (its recording
//!   actor), with its head, the number of records in it;
//! - **atomic**: one step may record in several cells (a GGD advice lands
//!   in the GGD's chronicle and in the municipality's), so a [`Commit`]
//!   appends to every cell at once or to none;
//! - **optimistic**: every cell in a commit names the head it was read at.
//!   When another request appended in between, the whole commit is refused
//!   with [`StoreError::Conflict`] and the service runs the step again on
//!   the new state. A cell the step only read is in the commit too, with no
//!   records, so a decision on a stale view of another cell is refused as
//!   well;
//! - **idempotent**: a commit may carry an [`IdempotencyKey`] and the
//!   request it answered. A second commit under a key already used is not
//!   applied; the store hands back the first one's [`Receipt`] instead;
//! - **one moment**: a step reads all cells of its case at once
//!   ([`ChronicleStore::load_all`]), so it never sees one cell before a
//!   commit and another after it.
//!
//! Two stores implement it, with the same behaviour, held to it by one set
//! of contract tests (`contract`):
//!
//! - [`memory::InMemoryStore`] holds it all in memory, empty at every start;
//! - [`postgres::PostgresStore`] keeps it in PostgreSQL, in the schema
//!   `wetsproces`, append-only.

#[cfg(test)]
mod contract;
pub mod memory;
pub mod postgres;

use async_trait::async_trait;
use regelrecht_cel::Gram;
use serde_json::Value;

/// The chronicle of one cell as the store holds it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoredChronicle {
    pub grams: Vec<Gram>,
}

impl StoredChronicle {
    /// The number of records in it: the head a commit names.
    pub fn head(&self) -> u64 {
        self.grams.len() as u64
    }
}

/// What a request asks to make unique: the client's own key for one
/// request, sent again with a retry of that request. Between 1 and
/// [`IdempotencyKey::MAX_LEN`] visible ASCII characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// The longest key, generous for a client's uuid with a prefix.
    pub const MAX_LEN: usize = 128;

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for IdempotencyKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.len() > Self::MAX_LEN {
            return Err(format!(
                "an idempotency key has 1 to {} characters",
                Self::MAX_LEN
            ));
        }
        if !s.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("an idempotency key is visible ASCII, without spaces".into());
        }
        Ok(Self(s.to_string()))
    }
}

/// The records a commit appends to one cell, after the head it was read at.
/// No records: the cell was only read, and the commit holds only when its
/// head has not moved.
#[derive(Debug, Clone, PartialEq)]
pub struct CellAppend {
    pub cell: String,
    pub expected_head: u64,
    pub grams: Vec<Gram>,
}

/// What a request answered, kept under its key.
#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
    /// The request as the service received it: a retry under the same key
    /// with another request is refused rather than answered with this one.
    pub request: Value,
    pub response: Value,
}

/// A key and what the request under it answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyed {
    pub key: IdempotencyKey,
    pub receipt: Receipt,
}

/// One step's appends, all or none. With a key, the store keeps the
/// receipt under it; a request without one leaves nothing to replay, so
/// nothing is kept for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub appends: Vec<CellAppend>,
    pub keyed: Option<Keyed>,
}

/// What became of a commit that the store did not refuse.
#[derive(Debug, Clone, PartialEq)]
pub enum Appended {
    /// Applied: every append is in its chronicle.
    Committed,
    /// Not applied: the key was used before, and this is what it answered.
    Replayed(Receipt),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum StoreError {
    /// The head of `cell` moved since it was read: nothing was applied.
    #[error("the chronicle of '{cell}' is at {actual}, not at {expected}")]
    Conflict {
        cell: String,
        expected: u64,
        actual: u64,
    },
    /// A stored record is in a format this build does not read. Nothing
    /// is guessed: the chronicle is refused as a whole.
    #[error(
        "record {seq} of '{cell}' is stored in format {version}, and this build reads only format \
         {current}",
        current = postgres::FORMAT_VERSION
    )]
    UnknownFormat {
        cell: String,
        seq: u64,
        version: i32,
    },
    /// The store itself failed.
    #[error("chronicle store: {0}")]
    Backend(String),
}

/// The chronicles of the cells, behind whatever keeps them.
#[async_trait]
pub trait ChronicleStore: Send + Sync + std::fmt::Debug {
    /// The chronicle of `cell`, empty when nothing was recorded in it.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the store fails.
    async fn load(&self, cell: &str) -> Result<StoredChronicle, StoreError>;

    /// The chronicles of `cells`, in that order, as they stood at one
    /// moment: a step reads every cell of its case from one state, never the
    /// municipality before a commit and the GGD after it.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the store fails.
    async fn load_all(&self, cells: &[&str]) -> Result<Vec<StoredChronicle>, StoreError>;

    /// What the commit under `key` answered, if one was applied.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the store fails.
    async fn receipt(&self, key: &IdempotencyKey) -> Result<Option<Receipt>, StoreError>;

    /// Apply `commit`: every append after its expected head, and the receipt
    /// under its key if it has one, at once. A key used before applies
    /// nothing and gives [`Appended::Replayed`]; that check comes before the
    /// heads, so a retry of an applied request is answered even after other
    /// requests moved on.
    ///
    /// # Errors
    ///
    /// [`StoreError::Conflict`] when a head moved, and nothing is applied;
    /// [`StoreError::Backend`] when the store fails.
    async fn append(&self, commit: Commit) -> Result<Appended, StoreError>;
}
