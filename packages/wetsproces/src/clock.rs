//! What day it is, for the service: every step runs on the day the
//! [`Clock`] gives, and records it as the day the cell recorded it.
//!
//! Two clocks: [`ControllableClock`], which the API sets in test mode so a
//! process can be played over weeks in minutes, and [`RealClock`], the date
//! in the Netherlands (`Europe/Amsterdam`) for stable mode.
//!
//! The controllable clock keeps its day in the chronicle store
//! ([`ChronicleStore::test_day`]), not in the process. Two instances on one
//! store, such as the one for citizens and the one for the back office
//! ([`crate::surface`]), then run their steps on the same day: the admin
//! sets it on the back office, and a citizen's submission a moment later
//! carries that date. On the store in memory the day lives as long as the
//! process, as before; on Postgres it survives a restart, as the chronicles
//! do.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;

use crate::store::{ChronicleStore, StoreError};

/// The day a step runs on.
#[async_trait]
pub trait Clock: Send + Sync + std::fmt::Debug {
    /// Today, as this clock tells it.
    ///
    /// # Errors
    ///
    /// When the clock keeps its day in a store and the store fails.
    async fn today(&self) -> Result<NaiveDate, StoreError>;
}

/// The calendar date in the Netherlands, from the system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealClock;

#[async_trait]
impl Clock for RealClock {
    async fn today(&self) -> Result<NaiveDate, StoreError> {
        Ok(regelrecht_shared::dates::today())
    }
}

/// A clock that stands still on the day it was last set to, kept in a
/// store, and on the day it starts on until it is first set.
#[derive(Debug)]
pub struct ControllableClock {
    store: Arc<dyn ChronicleStore>,
    start: NaiveDate,
}

impl ControllableClock {
    /// A clock on `store`, on `start` until it is set.
    pub fn new(store: Arc<dyn ChronicleStore>, start: NaiveDate) -> Self {
        Self { store, start }
    }

    /// Set the clock to `day`, for every instance on its store. It may go
    /// back as well as forward: the cell refuses a record dated before its
    /// last one, so going back in time cannot rewrite a chronicle.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn set(&self, day: NaiveDate) -> Result<(), StoreError> {
        self.store.set_test_day(day).await
    }
}

#[async_trait]
impl Clock for ControllableClock {
    async fn today(&self) -> Result<NaiveDate, StoreError> {
        Ok(self.store.test_day().await?.unwrap_or(self.start))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::store::memory::InMemoryStore;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn the_controllable_clock_stands_still_until_it_is_set() {
        let clock = ControllableClock::new(Arc::new(InMemoryStore::new()), day("2026-10-13"));
        assert_eq!(clock.today().await.unwrap(), day("2026-10-13"));
        assert_eq!(clock.today().await.unwrap(), day("2026-10-13"));
        clock.set(day("2026-11-01")).await.unwrap();
        assert_eq!(clock.today().await.unwrap(), day("2026-11-01"));
        clock.set(day("2026-10-20")).await.unwrap();
        assert_eq!(clock.today().await.unwrap(), day("2026-10-20"));
    }

    #[tokio::test]
    async fn two_clocks_on_one_store_tell_the_same_day() {
        let store: Arc<dyn ChronicleStore> = Arc::new(InMemoryStore::new());
        let citizens = ControllableClock::new(Arc::clone(&store), day("2026-10-13"));
        let backoffice = ControllableClock::new(store, day("2026-10-13"));
        backoffice.set(day("2026-11-01")).await.unwrap();
        assert_eq!(citizens.today().await.unwrap(), day("2026-11-01"));
    }

    #[tokio::test]
    async fn the_real_clock_gives_the_date_in_the_netherlands() {
        assert_eq!(
            RealClock.today().await.unwrap(),
            regelrecht_shared::dates::today()
        );
    }
}
