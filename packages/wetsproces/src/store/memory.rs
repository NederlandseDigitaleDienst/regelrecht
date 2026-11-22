//! The chronicle store in memory: empty at every start, gone at every stop.
//! What test mode runs on.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::NaiveDate;

use crate::store::{
    Actor, Appended, CellAppend, ChronicleStore, Commit, IdempotencyKey, Receipt, StoreError,
    StoredChronicle, StoredGram,
};

#[derive(Debug, Default)]
struct State {
    chronicles: BTreeMap<String, StoredChronicle>,
    receipts: BTreeMap<IdempotencyKey, Receipt>,
    /// The id of every stored record, in any cell.
    ids: BTreeSet<String>,
    /// The day the test clock was last set to.
    test_day: Option<NaiveDate>,
    /// How often the store was reset.
    generation: u64,
}

impl State {
    fn chronicle(&self, cell: &str) -> StoredChronicle {
        StoredChronicle {
            generation: self.generation,
            ..self.chronicles.get(cell).cloned().unwrap_or_default()
        }
    }

    /// Append every record of `appends`, as recorded by `recorded_by`. The
    /// caller checked the heads and the ids.
    fn write(&mut self, appends: Vec<CellAppend>, recorded_by: Option<&Actor>) {
        for append in appends {
            self.ids.extend(append.grams.iter().map(|g| g.id.clone()));
            let stored = append.grams.into_iter().map(|gram| StoredGram {
                gram,
                recorded_by: recorded_by.map(|a| a.account.clone()),
                recorded_via: recorded_by.map(|a| a.channel),
            });
            self.chronicles
                .entry(append.cell)
                .or_default()
                .grams
                .extend(stored);
        }
    }
}

/// Every chronicle and receipt in one map behind one lock, so a commit is
/// checked and applied in one go.
#[derive(Debug, Default)]
pub struct InMemoryStore {
    state: Mutex<State>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, StoreError> {
        // A panic while the lock was held may have left a commit half
        // applied; nothing after it can be trusted, so say so.
        self.state
            .lock()
            .map_err(|_| StoreError::Backend("the store in memory is poisoned".into()))
    }
}

#[async_trait]
impl ChronicleStore for InMemoryStore {
    async fn load(&self, cell: &str) -> Result<StoredChronicle, StoreError> {
        Ok(self.state()?.chronicle(cell))
    }

    async fn load_all(&self, cells: &[&str]) -> Result<Vec<StoredChronicle>, StoreError> {
        let state = self.state()?;
        Ok(cells.iter().map(|cell| state.chronicle(cell)).collect())
    }

    async fn receipt(&self, key: &IdempotencyKey) -> Result<Option<Receipt>, StoreError> {
        Ok(self.state()?.receipts.get(key).cloned())
    }

    async fn append(&self, commit: Commit) -> Result<Appended, StoreError> {
        let mut state = self.state()?;
        if let Some(keyed) = &commit.keyed
            && let Some(receipt) = state.receipts.get(&keyed.key)
        {
            return Ok(Appended::Replayed(receipt.clone()));
        }
        if commit.generation != state.generation {
            return Err(StoreError::Reset {
                expected: commit.generation,
                actual: state.generation,
            });
        }
        for append in &commit.appends {
            let actual = state
                .chronicles
                .get(&append.cell)
                .map_or(0, StoredChronicle::head);
            if actual != append.expected_head {
                return Err(StoreError::Conflict {
                    cell: append.cell.clone(),
                    expected: append.expected_head,
                    actual,
                });
            }
        }
        let mut ids = BTreeSet::new();
        for gram in commit.appends.iter().flat_map(|a| &a.grams) {
            if state.ids.contains(&gram.id) || !ids.insert(gram.id.clone()) {
                return Err(StoreError::DuplicateGram {
                    id: gram.id.clone(),
                });
            }
        }
        state.write(commit.appends, commit.recorded_by.as_ref());
        if let Some(keyed) = commit.keyed {
            state.receipts.insert(keyed.key, keyed.receipt);
        }
        Ok(Appended::Committed)
    }

    async fn test_day(&self) -> Result<Option<NaiveDate>, StoreError> {
        Ok(self.state()?.test_day)
    }

    async fn set_test_day(&self, day: NaiveDate) -> Result<(), StoreError> {
        self.state()?.test_day = Some(day);
        Ok(())
    }

    async fn reset(&self, begin: Vec<CellAppend>) -> Result<(), StoreError> {
        // Under the lock every commit takes, so none is left half and none
        // finds the store empty.
        let mut state = self.state()?;
        *state = State {
            generation: state.generation + 1,
            ..State::default()
        };
        state.write(begin, None);
        drop(state);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::store::contract::{self, append, unkeyed};

    impl InMemoryStore {
        fn store(&self) -> &dyn ChronicleStore {
            self
        }
    }

    contract::contract_tests!(async { InMemoryStore::new() });

    #[tokio::test]
    async fn without_a_key_nothing_is_kept_to_replay() {
        let store = InMemoryStore::new();
        let c = unkeyed(vec![append("a", 0, &["1"])]);
        assert_eq!(store.append(c).await.unwrap(), Appended::Committed);
        assert!(store.state().unwrap().receipts.is_empty());
    }

    #[test]
    fn an_idempotency_key_is_short_visible_ascii() {
        assert!("abc-123".parse::<IdempotencyKey>().is_ok());
        assert!("".parse::<IdempotencyKey>().is_err());
        assert!("a b".parse::<IdempotencyKey>().is_err());
        assert!("é".parse::<IdempotencyKey>().is_err());
        assert!(
            "x".repeat(IdempotencyKey::MAX_LEN + 1)
                .parse::<IdempotencyKey>()
                .is_err()
        );
    }
}
