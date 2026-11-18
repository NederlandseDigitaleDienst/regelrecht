//! The chronicle store in memory: empty at every start, gone at every stop.
//! What test mode runs on.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::store::{
    Appended, ChronicleStore, Commit, IdempotencyKey, Receipt, StoreError, StoredChronicle,
};

#[derive(Debug, Default)]
struct State {
    chronicles: BTreeMap<String, StoredChronicle>,
    receipts: BTreeMap<IdempotencyKey, Receipt>,
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

    async fn receipt(&self, key: &IdempotencyKey) -> Result<Option<Receipt>, StoreError> {
        Ok(self.state()?.receipts.get(key).cloned())
    }

    async fn append(&self, commit: Commit) -> Result<Appended, StoreError> {
        let mut state = self.state()?;
        if let Some(receipt) = state.receipts.get(&commit.key) {
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
        for append in commit.appends {
            state
                .chronicles
                .entry(append.cell)
                .or_default()
                .grams
                .extend(append.grams);
        }
        state.receipts.insert(commit.key, commit.receipt);
        Ok(Appended::Committed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use regelrecht_cel::Gram;
    use serde_json::json;

    use super::*;
    use crate::store::CellAppend;

    fn gram(id: &str, cell: &str) -> Gram {
        serde_json::from_value(json!({
            "id": id,
            "kind": "EXECUTOGRAM",
            "basis": "voorbeeld#1",
            "fact": "feit",
            "recording_actor": cell,
            "recorded_on": "2026-10-13",
            "fields": {},
        }))
        .unwrap()
    }

    fn key(k: &str) -> IdempotencyKey {
        k.parse().unwrap()
    }

    fn commit(k: &str, appends: Vec<CellAppend>) -> Commit {
        Commit {
            key: key(k),
            appends,
            receipt: Receipt {
                request: json!({ "k": k }),
                response: json!({ "answered": k }),
            },
        }
    }

    fn append(cell: &str, expected_head: u64, ids: &[&str]) -> CellAppend {
        CellAppend {
            cell: cell.into(),
            expected_head,
            grams: ids.iter().map(|id| gram(id, cell)).collect(),
        }
    }

    #[tokio::test]
    async fn a_commit_appends_to_every_cell_after_its_head() {
        let store = InMemoryStore::new();
        assert_eq!(store.load("a").await.unwrap().head(), 0);
        let c = commit(
            "k1",
            vec![append("a", 0, &["1"]), append("b", 0, &["2", "3"])],
        );
        assert_eq!(store.append(c).await.unwrap(), Appended::Committed);
        assert_eq!(store.load("a").await.unwrap().head(), 1);
        assert_eq!(store.load("b").await.unwrap().head(), 2);
        let c = commit("k2", vec![append("a", 1, &["4"])]);
        store.append(c).await.unwrap();
        let ids: Vec<String> = store
            .load("a")
            .await
            .unwrap()
            .grams
            .into_iter()
            .map(|g| g.id)
            .collect();
        assert_eq!(ids, ["1", "4"]);
    }

    #[tokio::test]
    async fn a_moved_head_refuses_the_whole_commit() {
        let store = InMemoryStore::new();
        store
            .append(commit("k1", vec![append("a", 0, &["1"])]))
            .await
            .unwrap();
        // "b" is fine, "a" moved: neither is applied.
        let stale = commit("k2", vec![append("b", 0, &["2"]), append("a", 0, &["3"])]);
        assert_eq!(
            store.append(stale).await,
            Err(StoreError::Conflict {
                cell: "a".into(),
                expected: 0,
                actual: 1
            })
        );
        assert_eq!(store.load("b").await.unwrap().head(), 0);
        assert_eq!(store.load("a").await.unwrap().head(), 1);
        assert_eq!(store.receipt(&key("k2")).await.unwrap(), None);
        // A cell only read, with no records, is checked as well.
        let read_only = commit("k3", vec![append("b", 0, &["2"]), append("a", 0, &[])]);
        assert!(matches!(
            store.append(read_only).await,
            Err(StoreError::Conflict { .. })
        ));
    }

    #[tokio::test]
    async fn a_key_used_before_is_replayed_and_applies_nothing() {
        let store = InMemoryStore::new();
        let first = commit("k1", vec![append("a", 0, &["1"])]);
        let receipt = first.receipt.clone();
        store.append(first).await.unwrap();
        assert_eq!(
            store.receipt(&key("k1")).await.unwrap(),
            Some(receipt.clone())
        );
        // The retry comes after the head moved; it is still answered, not
        // refused as a conflict, and appends nothing.
        let retry = commit("k1", vec![append("a", 0, &["2"])]);
        assert_eq!(
            store.append(retry).await.unwrap(),
            Appended::Replayed(receipt)
        );
        assert_eq!(store.load("a").await.unwrap().head(), 1);
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
        assert_ne!(IdempotencyKey::fresh(), IdempotencyKey::fresh());
    }
}
