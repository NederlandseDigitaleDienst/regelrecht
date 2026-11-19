//! The behaviour every [`ChronicleStore`] owes the service, as tests that
//! take the store to check. Each implementation runs all of them through
//! [`contract_tests!`], so the store in memory and the one in Postgres
//! cannot drift apart.

#![allow(clippy::unwrap_used)]

use futures::future::join_all;
use regelrecht_cel::Gram;
use serde_json::json;

use crate::store::{
    Appended, CellAppend, ChronicleStore, Commit, IdempotencyKey, Keyed, Receipt, StoreError,
    StoredChronicle,
};

/// How many requests the concurrency tests send at once: more than one
/// connection's worth, fewer than a pool holds.
const CONCURRENT: usize = 8;

pub fn gram(id: &str, cell: &str) -> Gram {
    serde_json::from_value(json!({
        "id": id,
        "kind": "EXECUTOGRAM",
        "basis": "voorbeeld#1",
        "fact": "feit",
        "recording_actor": cell,
        "recorded_on": "2026-10-13",
        "fields": { "bedrag": 12.5, "naam": "A. de Vries", "lijst": [1, 2] },
    }))
    .unwrap()
}

pub fn key(k: &str) -> IdempotencyKey {
    k.parse().unwrap()
}

pub fn commit(k: &str, appends: Vec<CellAppend>) -> Commit {
    Commit {
        appends,
        keyed: Some(Keyed {
            key: key(k),
            receipt: Receipt {
                request: json!({ "k": k }),
                response: json!({ "answered": k }),
            },
        }),
    }
}

pub fn unkeyed(appends: Vec<CellAppend>) -> Commit {
    Commit {
        appends,
        keyed: None,
    }
}

pub fn append(cell: &str, expected_head: u64, ids: &[&str]) -> CellAppend {
    CellAppend {
        cell: cell.into(),
        expected_head,
        grams: ids.iter().map(|id| gram(id, cell)).collect(),
    }
}

fn ids(chronicle: &StoredChronicle) -> Vec<&str> {
    chronicle.grams.iter().map(|g| g.id.as_str()).collect()
}

pub async fn a_commit_appends_to_every_cell_after_its_head(store: &dyn ChronicleStore) {
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
    let a = store.load("a").await.unwrap();
    assert_eq!(ids(&a), ["1", "4"]);
    // A record comes back as it went in.
    assert_eq!(a.grams[1], gram("4", "a"));
}

pub async fn load_all_gives_every_cell_in_the_order_asked(store: &dyn ChronicleStore) {
    let c = unkeyed(vec![append("a", 0, &["1"]), append("c", 0, &["2", "3"])]);
    assert_eq!(store.append(c).await.unwrap(), Appended::Committed);
    let all = store.load_all(&["c", "b", "a"]).await.unwrap();
    assert_eq!(
        all.iter().map(StoredChronicle::head).collect::<Vec<_>>(),
        [2, 0, 1]
    );
    assert_eq!(ids(&all[0]), ["2", "3"]);
    assert!(store.load_all(&[]).await.unwrap().is_empty());
}

pub async fn a_moved_head_refuses_the_whole_commit(store: &dyn ChronicleStore) {
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
    assert_eq!(store.load("b").await.unwrap().head(), 0);
    // Read at the right head, the cell only read lets the commit through.
    let current = commit("k4", vec![append("b", 0, &["2"]), append("a", 1, &[])]);
    assert_eq!(store.append(current).await.unwrap(), Appended::Committed);
    assert_eq!(store.load("a").await.unwrap().head(), 1);
}

pub async fn a_key_used_before_is_replayed_and_applies_nothing(store: &dyn ChronicleStore) {
    let first = commit("k1", vec![append("a", 0, &["1"])]);
    let receipt = first.keyed.clone().unwrap().receipt;
    store.append(first).await.unwrap();
    assert_eq!(
        store.receipt(&key("k1")).await.unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(store.receipt(&key("k9")).await.unwrap(), None);
    // The retry comes after the head moved; it is still answered, not
    // refused as a conflict, and appends nothing.
    let retry = commit("k1", vec![append("a", 0, &["2"])]);
    assert_eq!(
        store.append(retry).await.unwrap(),
        Appended::Replayed(receipt)
    );
    assert_eq!(store.load("a").await.unwrap().head(), 1);
}

pub async fn of_concurrent_appends_on_one_cell_exactly_one_wins(store: &dyn ChronicleStore) {
    let attempts = (0..CONCURRENT).map(|i| {
        let id = format!("g{i}");
        let c = commit(&format!("k{i}"), vec![append("a", 0, &[id.as_str()])]);
        store.append(c)
    });
    let results = join_all(attempts).await;
    let won = results
        .iter()
        .filter(|r| matches!(r, Ok(Appended::Committed)))
        .count();
    let lost = results
        .iter()
        .filter(|r| {
            matches!(
                r,
                Err(StoreError::Conflict {
                    expected: 0,
                    actual: 1,
                    ..
                })
            )
        })
        .count();
    assert_eq!((won, lost), (1, CONCURRENT - 1), "{results:?}");
    assert_eq!(store.load("a").await.unwrap().head(), 1);
}

pub async fn of_concurrent_retries_under_one_key_one_commits_and_the_rest_replay(
    store: &dyn ChronicleStore,
) {
    let attempts = (0..CONCURRENT).map(|i| {
        let id = format!("g{i}");
        let c = commit("same", vec![append("a", 0, &[id.as_str()])]);
        store.append(c)
    });
    let results = join_all(attempts).await;
    let committed = results
        .iter()
        .filter(|r| matches!(r, Ok(Appended::Committed)))
        .count();
    let replayed = results
        .iter()
        .filter(|r| matches!(r, Ok(Appended::Replayed(_))))
        .count();
    assert_eq!((committed, replayed), (1, CONCURRENT - 1), "{results:?}");
    assert_eq!(store.load("a").await.unwrap().head(), 1);
}

/// One `#[tokio::test]` per contract test, each on a fresh store from
/// `$fixture`, an async expression whose value has a `store()` method.
macro_rules! contract_tests {
    ($fixture:expr) => {
        $crate::store::contract::contract_tests!(@each $fixture;
            a_commit_appends_to_every_cell_after_its_head,
            load_all_gives_every_cell_in_the_order_asked,
            a_moved_head_refuses_the_whole_commit,
            a_key_used_before_is_replayed_and_applies_nothing,
            of_concurrent_appends_on_one_cell_exactly_one_wins,
            of_concurrent_retries_under_one_key_one_commits_and_the_rest_replay,
        );
    };
    (@each $fixture:expr; $($name:ident,)*) => {
        $(
            #[tokio::test]
            async fn $name() {
                let fixture = $fixture.await;
                $crate::store::contract::$name(fixture.store()).await;
            }
        )*
    };
}
pub(crate) use contract_tests;
