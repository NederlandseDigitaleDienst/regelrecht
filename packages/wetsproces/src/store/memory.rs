//! The chronicle store in memory: empty at every start, gone at every stop.
//! What test mode runs on.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use async_trait::async_trait;

use crate::store::{
    Appended, ChronicleStore, Commit, IdempotencyKey, Receipt, StoreError, StoredChronicle,
    StoredGram,
};

#[derive(Debug, Default)]
struct State {
    chronicles: BTreeMap<String, StoredChronicle>,
    receipts: BTreeMap<IdempotencyKey, Receipt>,
    /// The id of every stored record, in any cell.
    ids: BTreeSet<String>,
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
        Ok(self
            .state()?
            .chronicles
            .get(cell)
            .cloned()
            .unwrap_or_default())
    }

    async fn load_all(&self, cells: &[&str]) -> Result<Vec<StoredChronicle>, StoreError> {
        let state = self.state()?;
        Ok(cells
            .iter()
            .map(|cell| state.chronicles.get(*cell).cloned().unwrap_or_default())
            .collect())
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
        state.ids.append(&mut ids);
        for append in commit.appends {
            let stored = append.grams.into_iter().map(|gram| StoredGram {
                gram,
                recorded_by: commit.recorded_by.clone(),
            });
            state
                .chronicles
                .entry(append.cell)
                .or_default()
                .grams
                .extend(stored);
        }
        if let Some(keyed) = commit.keyed {
            state.receipts.insert(keyed.key, keyed.receipt);
        }
        Ok(Appended::Committed)
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
