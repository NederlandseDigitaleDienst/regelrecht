//! The behaviour every [`ChronicleStore`] owes the service, as tests that
//! take the store to check. Each implementation runs all of them through
//! [`contract_tests!`], so the store in memory and the one in Postgres
//! cannot drift apart.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use futures::future::join_all;
use regelrecht_cel::Gram;
use serde_json::json;

use crate::store::{
    Actor, Appended, CellAppend, Channel, ChronicleStore, Commit, IdempotencyKey, Keyed, Receipt,
    StoreError, StoredChronicle,
};

/// How many requests the concurrency tests send at once: more than one
/// connection's worth, fewer than a pool holds.
const CONCURRENT: usize = 8;

/// The account a keyed test commit is made by: a made-up `sub`.
pub const ACCOUNT: &str = "00000000-0000-4000-8000-00000000000a";

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
        recorded_by: Some(Actor {
            account: ACCOUNT.into(),
            channel: Channel::Keycloak,
        }),
        by_policy: BTreeSet::new(),
        generation: 0,
    }
}

pub fn unkeyed(appends: Vec<CellAppend>) -> Commit {
    Commit {
        appends,
        keyed: None,
        recorded_by: None,
        by_policy: BTreeSet::new(),
        generation: 0,
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
    chronicle.grams.iter().map(|g| g.gram.id.as_str()).collect()
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
    assert_eq!(a.grams[1].gram, gram("4", "a"));
}

pub async fn the_store_keeps_who_recorded_each_gram(store: &dyn ChronicleStore) {
    let by_account = commit("k1", vec![append("a", 0, &["1"]), append("b", 0, &["2"])]);
    store.append(by_account).await.unwrap();
    let by_server = unkeyed(vec![append("a", 1, &["3"])]);
    store.append(by_server).await.unwrap();
    let all = store.load_all(&["a", "b"]).await.unwrap();
    let by: Vec<Vec<Option<&str>>> = all
        .iter()
        .map(|c| c.grams.iter().map(|g| g.recorded_by.as_deref()).collect())
        .collect();
    assert_eq!(by, [vec![Some(ACCOUNT), None], vec![Some(ACCOUNT)]]);
    let via: Vec<Option<Channel>> = all[0].grams.iter().map(|g| g.recorded_via).collect();
    assert_eq!(via, [Some(Channel::Keycloak), None]);
    // Who recorded it stays beside the gram, not in it.
    assert_eq!(all[0].grams[0].gram, gram("1", "a"));
    // Every channel is kept as it went in, the admin panel's as well.
    for (head, channel) in (1..).zip(Channel::ALL) {
        let c = Commit {
            appends: vec![append("b", head, &[&format!("via-{}", channel.as_str())])],
            keyed: None,
            recorded_by: Some(Actor {
                account: ACCOUNT.into(),
                channel,
            }),
            by_policy: BTreeSet::new(),
            generation: 0,
        };
        store.append(c).await.unwrap();
    }
    let b = store.load("b").await.unwrap();
    let via: Vec<Option<Channel>> = b.grams.iter().skip(1).map(|g| g.recorded_via).collect();
    assert_eq!(via, Channel::ALL.map(Some));
}

pub async fn a_record_the_policy_made_is_kept_as_made_by_its_article(store: &dyn ChronicleStore) {
    // One step: the citizen's record, and the two the executor's policy made
    // with it, in two cells.
    let mut c = commit(
        "k-policy",
        vec![append("a", 0, &["1", "2"]), append("b", 0, &["3"])],
    );
    c.by_policy = ["2".to_string(), "3".to_string()].into();
    store.append(c).await.unwrap();
    let all = store.load_all(&["a", "b"]).await.unwrap();
    let kept: Vec<(Option<&str>, Option<Channel>)> = all
        .iter()
        .flat_map(|c| &c.grams)
        .map(|g| (g.recorded_by.as_deref(), g.recorded_via))
        .collect();
    assert_eq!(
        kept,
        [
            (Some(ACCOUNT), Some(Channel::Keycloak)),
            (Some("voorbeeld#1"), Some(Channel::Policy)),
            (Some("voorbeeld#1"), Some(Channel::Policy)),
        ]
    );
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

pub async fn a_record_id_is_stored_once(store: &dyn ChronicleStore) {
    store
        .append(unkeyed(vec![append("a", 0, &["1"])]))
        .await
        .unwrap();
    // Taken in another cell: nothing of the commit is applied.
    let again = commit("k1", vec![append("b", 0, &["2"]), append("c", 0, &["1"])]);
    assert_eq!(
        store.append(again).await,
        Err(StoreError::DuplicateGram { id: "1".into() })
    );
    // Twice in one commit.
    let twice = unkeyed(vec![append("b", 0, &["3"]), append("c", 0, &["3"])]);
    assert_eq!(
        store.append(twice).await,
        Err(StoreError::DuplicateGram { id: "3".into() })
    );
    let heads = store.load_all(&["a", "b", "c"]).await.unwrap();
    assert_eq!(
        heads.iter().map(StoredChronicle::head).collect::<Vec<_>>(),
        [1, 0, 0]
    );
    assert_eq!(store.receipt(&key("k1")).await.unwrap(), None);
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

pub async fn the_test_clock_holds_the_day_it_was_last_set_to(store: &dyn ChronicleStore) {
    assert_eq!(store.test_day().await.unwrap(), None);
    let day = |s: &str| s.parse::<chrono::NaiveDate>().unwrap();
    store.set_test_day(day("2026-11-01")).await.unwrap();
    assert_eq!(store.test_day().await.unwrap(), Some(day("2026-11-01")));
    // Back as well as forward: the cell, not the clock, guards the order.
    store.set_test_day(day("2026-10-20")).await.unwrap();
    assert_eq!(store.test_day().await.unwrap(), Some(day("2026-10-20")));
}

/// A reset empties every chronicle, the kept answers and the test clock,
/// and the store takes new records from a head of nothing again.
pub async fn a_reset_empties_the_chronicles_the_answers_and_the_clock(store: &dyn ChronicleStore) {
    store
        .append(commit(
            "k1",
            vec![append("a", 0, &["1"]), append("b", 0, &["2"])],
        ))
        .await
        .unwrap();
    let day = "2026-12-01".parse::<chrono::NaiveDate>().unwrap();
    store.set_test_day(day).await.unwrap();
    store.reset(Vec::new()).await.unwrap();
    let after = store.load_all(&["a", "b"]).await.unwrap();
    for chronicle in &after {
        assert_eq!(chronicle.head(), 0);
        assert_eq!(chronicle.generation, 1);
    }
    assert!(store.receipt(&key("k1")).await.unwrap().is_none());
    assert_eq!(store.test_day().await.unwrap(), None);
    // The same ids and the same key are free again, in the new generation.
    let mut again = commit("k1", vec![append("a", 0, &["1"])]);
    again.generation = 1;
    assert_eq!(store.append(again).await.unwrap(), Appended::Committed);
}

/// A reset records what it is given to begin with, in the same go: nobody
/// sees the store empty, and those records are attributed to no account.
pub async fn a_reset_records_the_beginning_it_is_given(store: &dyn ChronicleStore) {
    store
        .append(unkeyed(vec![append("a", 0, &["1", "2"])]))
        .await
        .unwrap();
    store
        .reset(vec![append("a", 0, &["begin-a"]), append("b", 0, &[])])
        .await
        .unwrap();
    let all = store.load_all(&["a", "b"]).await.unwrap();
    assert_eq!(ids(&all[0]), ["begin-a"]);
    assert_eq!(all[0].grams[0].recorded_by, None);
    assert_eq!(all[1].head(), 0);
    // An id from before the reset is free again; one of the beginning is not.
    let mut next = unkeyed(vec![append("a", 1, &["1"])]);
    next.generation = all[0].generation;
    assert_eq!(store.append(next).await.unwrap(), Appended::Committed);
    let mut taken = unkeyed(vec![append("b", 0, &["begin-a"])]);
    taken.generation = all[0].generation;
    assert!(matches!(
        store.append(taken).await,
        Err(StoreError::DuplicateGram { .. })
    ));
}

/// A step read before a reset is refused when it commits after it, also
/// when the heads it names happen to stand where they stood before: the
/// records they count are others (ABA).
pub async fn a_commit_read_before_a_reset_is_refused_though_its_heads_match(
    store: &dyn ChronicleStore,
) {
    store
        .append(unkeyed(vec![append("a", 0, &["1"])]))
        .await
        .unwrap();
    let read = store.load("a").await.unwrap();
    // The reset begins with one record again, so the head is 1 as before.
    store
        .reset(vec![append("a", 0, &["begin-a"])])
        .await
        .unwrap();
    let mut late = unkeyed(vec![append("a", read.head(), &["2"])]);
    late.generation = read.generation;
    assert_eq!(
        store.append(late).await,
        Err(StoreError::Reset {
            expected: read.generation,
            actual: read.generation + 1,
        })
    );
    assert_eq!(ids(&store.load("a").await.unwrap()), ["begin-a"]);
}

pub async fn deliveries_over_a_link_are_kept_per_cell_until_a_reset(store: &dyn ChronicleStore) {
    assert!(store.delivered("ggd").await.unwrap().is_empty());
    store.mark_delivered("1", "ggd").await.unwrap();
    // A second confirmation of the same record changes nothing.
    store.mark_delivered("1", "ggd").await.unwrap();
    store.mark_delivered("2", "rechtbank").await.unwrap();
    assert_eq!(
        store.delivered("ggd").await.unwrap(),
        BTreeSet::from(["1".to_string()])
    );
    assert_eq!(
        store.delivered("rechtbank").await.unwrap(),
        BTreeSet::from(["2".to_string()])
    );
    store.reset(Vec::new()).await.unwrap();
    assert!(store.delivered("ggd").await.unwrap().is_empty());
}

/// One `#[tokio::test]` per contract test, each on a fresh store from
/// `$fixture`, an async expression whose value has a `store()` method.
macro_rules! contract_tests {
    ($fixture:expr) => {
        $crate::store::contract::contract_tests!(@each $fixture;
            a_commit_appends_to_every_cell_after_its_head,
            the_store_keeps_who_recorded_each_gram,
            a_record_the_policy_made_is_kept_as_made_by_its_article,
            load_all_gives_every_cell_in_the_order_asked,
            a_moved_head_refuses_the_whole_commit,
            a_key_used_before_is_replayed_and_applies_nothing,
            a_record_id_is_stored_once,
            of_concurrent_appends_on_one_cell_exactly_one_wins,
            of_concurrent_retries_under_one_key_one_commits_and_the_rest_replay,
            the_test_clock_holds_the_day_it_was_last_set_to,
            a_reset_empties_the_chronicles_the_answers_and_the_clock,
            a_reset_records_the_beginning_it_is_given,
            a_commit_read_before_a_reset_is_refused_though_its_heads_match,
            deliveries_over_a_link_are_kept_per_cell_until_a_reset,
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
