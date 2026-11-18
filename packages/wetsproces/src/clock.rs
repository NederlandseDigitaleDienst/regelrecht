//! What day it is, for the service: every step runs on the day the
//! [`Clock`] gives, and records it as the day the cell recorded it.
//!
//! Two clocks: [`ControllableClock`], which the API sets in test mode so a
//! process can be played over weeks in minutes, and [`RealClock`], the date
//! in the Netherlands (`Europe/Amsterdam`) for stable mode.

use std::sync::RwLock;

use chrono::NaiveDate;

/// The day a step runs on.
pub trait Clock: Send + Sync + std::fmt::Debug {
    /// Today, as this clock tells it.
    fn today(&self) -> NaiveDate;
}

/// The calendar date in the Netherlands, from the system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealClock;

impl Clock for RealClock {
    fn today(&self) -> NaiveDate {
        regelrecht_shared::dates::today()
    }
}

/// A clock that stands still on the day it was last set to.
#[derive(Debug)]
pub struct ControllableClock {
    day: RwLock<NaiveDate>,
}

impl ControllableClock {
    /// A clock on `day`.
    pub fn new(day: NaiveDate) -> Self {
        Self {
            day: RwLock::new(day),
        }
    }

    /// Set the clock to `day`. It may go back as well as forward: the cell
    /// refuses a record dated before its last one, so going back in time
    /// cannot rewrite a chronicle.
    pub fn set(&self, day: NaiveDate) {
        // A poisoned lock still holds a whole date: a write of a `Copy`
        // value cannot be left half done.
        let mut guard = self
            .day
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = day;
    }
}

impl Clock for ControllableClock {
    fn today(&self) -> NaiveDate {
        *self
            .day
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn the_controllable_clock_stands_still_until_it_is_set() {
        let clock = ControllableClock::new(day("2026-10-13"));
        assert_eq!(clock.today(), day("2026-10-13"));
        assert_eq!(clock.today(), day("2026-10-13"));
        clock.set(day("2026-11-01"));
        assert_eq!(clock.today(), day("2026-11-01"));
        clock.set(day("2026-10-20"));
        assert_eq!(clock.today(), day("2026-10-20"));
    }

    #[test]
    fn the_real_clock_gives_the_date_in_the_netherlands() {
        assert_eq!(RealClock.today(), regelrecht_shared::dates::today());
    }
}
