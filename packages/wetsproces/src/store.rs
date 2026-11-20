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
//!   commit and another after it;
//! - **attributed**: a commit names who made it ([`Commit::recorded_by`]):
//!   the account and the channel it came in by ([`Channel`]). The store
//!   keeps both beside every record it appends, never inside the record: a
//!   gram says what the law made happen, the store says which account asked
//!   for it and how it was logged in;
//! - **the test clock**: in test mode the day an admin set the clock to
//!   ([`ChronicleStore::test_day`]) lives beside the chronicles, so every
//!   instance on one store runs its steps on the same day. An instance for
//!   citizens and one for the back office ([`crate::surface`]) tell the
//!   same date without talking to each other.
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
use chrono::NaiveDate;
use regelrecht_cel::Gram;
use serde_json::Value;

/// The format a stored record is in: a `regelrecht_cel::Gram` as serde
/// writes it. A store that keeps records outside the process tags each
/// with it and refuses a record in any other format when it reads it
/// ([`StoreError::UnknownFormat`]); a change to the gram that an older
/// reader would misread raises this number.
pub const FORMAT_VERSION: i32 = 1;

/// One record as the store holds it: the gram, and who asked for the step
/// that recorded it.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredGram {
    pub gram: Gram,
    /// The account (its OIDC `sub`) whose request recorded it; `None` for
    /// what the server recorded itself, such as the case's beginning.
    pub recorded_by: Option<String>,
    /// The channel that account came in by; `None` beside `None`, and for
    /// what was stored before the channel was kept.
    pub recorded_via: Option<Channel>,
}

/// How an account was logged in when it asked for a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// A browser session after the login through Keycloak.
    Keycloak,
    /// A browser session after the test login of test mode.
    TestLogin,
    /// A session of the FSC voor Burgers channel ([`crate::fsc4b`]).
    Fsc4b,
    /// The admin panel of the back office, through which an admin records
    /// what an external party of the case did (the GGD's advice, the court's
    /// judgment), until a real link with that party replaces it. The record
    /// itself names the party (its `recording_actor`); the account beside it
    /// is the admin's.
    AdminPanel,
}

impl Channel {
    /// Every channel.
    pub const ALL: [Self; 4] = [
        Self::Keycloak,
        Self::TestLogin,
        Self::Fsc4b,
        Self::AdminPanel,
    ];

    /// The name the store keeps.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keycloak => "keycloak",
            Self::TestLogin => "testlogin",
            Self::Fsc4b => "fsc4b",
            Self::AdminPanel => "beheerpaneel",
        }
    }

    /// The channel the store kept as `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == name)
    }
}

/// Who asked for a step: the account and the channel it came in by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub account: String,
    pub channel: Channel,
}

/// The chronicle of one cell as the store holds it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoredChronicle {
    pub grams: Vec<StoredGram>,
}

impl StoredChronicle {
    /// The number of records in it: the head a commit names.
    pub fn head(&self) -> u64 {
        self.grams.len() as u64
    }

    /// Its records without who recorded them, as a cell takes them.
    pub fn into_grams(self) -> Vec<Gram> {
        self.grams.into_iter().map(|g| g.gram).collect()
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
    /// Who made the step, kept beside each record it appends
    /// ([`StoredGram::recorded_by`], [`StoredGram::recorded_via`]).
    pub recorded_by: Option<Actor>,
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
        current = FORMAT_VERSION
    )]
    UnknownFormat {
        cell: String,
        seq: u64,
        version: i32,
    },
    /// A record in the commit has the id of a stored record, or of another
    /// record in the same commit: nothing was applied. Ids are made by the
    /// cell, so this is a fault in the server, not in the request.
    #[error("a record with id '{id}' is already stored")]
    DuplicateGram { id: String },
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
    /// [`StoreError::UnknownFormat`] when a record is in a format this
    /// build does not read; [`StoreError::Backend`] when the store fails.
    async fn load(&self, cell: &str) -> Result<StoredChronicle, StoreError>;

    /// The chronicles of `cells`, in that order, as they stood at one
    /// moment: a step reads every cell of its case from one state, never the
    /// municipality before a commit and the GGD after it.
    ///
    /// # Errors
    ///
    /// [`StoreError::UnknownFormat`] when a record is in a format this
    /// build does not read; [`StoreError::Backend`] when the store fails.
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
    /// [`StoreError::Conflict`] when a head moved, and
    /// [`StoreError::DuplicateGram`] when a record's id is taken; then
    /// nothing is applied.
    /// [`StoreError::Backend`] when the store fails.
    async fn append(&self, commit: Commit) -> Result<Appended, StoreError>;

    /// The day the test clock was last set to, or `None` when it never was
    /// on this store.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the store fails.
    async fn test_day(&self) -> Result<Option<NaiveDate>, StoreError>;

    /// Set the test clock to `day`, for every instance on this store.
    ///
    /// # Errors
    ///
    /// [`StoreError::Backend`] when the store fails.
    async fn set_test_day(&self, day: NaiveDate) -> Result<(), StoreError>;
}
