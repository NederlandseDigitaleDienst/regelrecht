//! Whether the law a record rests on is still the law of the corpus.
//!
//! A submission or a decision keeps the regulation versions it was recorded
//! under (`law_versions`): per regelwerk its `$id`, the `valid_from` of the
//! version and the hash of its content. A replay takes the decision again
//! under the corpus as it is now. Comparing the two says whether the corpus
//! still holds what the record was taken under, apart from whether the
//! outcome is the same: a regelwerk can change without changing the
//! outcome, and a different outcome then has a cause to name.

use std::fmt;

use chrono::NaiveDate;

use regelrecht_engine::{LawExecutionService, RegulationRead};

use crate::chronicle::Gram;

/// What became of the regulation versions a record was taken under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LawSince {
    /// The record keeps no versions: it is from before the cell kept them,
    /// or it ran no article, as an executogram.
    NotRecorded,
    /// Each version is in the corpus with the same content, and no later
    /// version is in force on the day of the record.
    Unchanged,
    /// These versions are not, in the order the record keeps them.
    Changed(Vec<LawChange>),
}

/// A regulation version a record was taken under that the corpus no longer
/// holds as it was, or that a later version replaces on the day of the
/// record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawChange {
    /// The version as the record keeps it.
    pub recorded: RegulationRead,
    /// The corpus now: the same version with other content, a later
    /// version in force on the day of the record, or none when the version
    /// is gone.
    pub now: Option<RegulationRead>,
}

impl fmt::Display for LawChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = |read: &RegulationRead| read.valid_from.clone().unwrap_or_else(|| "?".into());
        let id = &self.recorded.law_id;
        match &self.now {
            None => write!(f, "{id} no longer has version {}", from(&self.recorded)),
            Some(now) if now.valid_from == self.recorded.valid_from => write!(
                f,
                "{id}, version {}, has other content ({} was {})",
                from(now),
                now.hash.as_deref().unwrap_or("no hash"),
                self.recorded.hash.as_deref().unwrap_or("no hash")
            ),
            Some(now) => write!(
                f,
                "{id} is in force in version {} that day, not version {}",
                from(now),
                from(&self.recorded)
            ),
        }
    }
}

impl fmt::Display for LawSince {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRecorded => write!(f, "the record keeps no regulation versions"),
            Self::Unchanged => write!(f, "the regulation versions are unchanged"),
            Self::Changed(changes) => {
                write!(f, "since the record, ")?;
                for (i, change) in changes.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{change}")?;
                }
                Ok(())
            }
        }
    }
}

/// The versions `gram` was taken under, against the corpus of `service`.
///
/// A version is compared with itself first: gone from the corpus, or
/// with other content. Then with the day: another, later version of the
/// regelwerk now in force on the day of the record, which the record could
/// not have run. A version that was not in force on that day, such as the
/// newest version of a law whose hooks the engine reads, is no change by
/// itself.
pub fn since(service: &LawExecutionService, gram: &Gram) -> LawSince {
    if gram.law_versions.is_empty() {
        return LawSince::NotRecorded;
    }
    let changes: Vec<LawChange> = gram
        .law_versions
        .iter()
        .filter_map(|recorded| change_of(service, recorded, gram.recorded_on))
        .collect();
    if changes.is_empty() {
        LawSince::Unchanged
    } else {
        LawSince::Changed(changes)
    }
}

/// What became of the version `recorded` that a record of `day` ran, if
/// anything did.
fn change_of(
    service: &LawExecutionService,
    recorded: &RegulationRead,
    day: NaiveDate,
) -> Option<LawChange> {
    let change = |now: Option<RegulationRead>| {
        Some(LawChange {
            recorded: recorded.clone(),
            now,
        })
    };
    let same = service
        .get_law_version(&recorded.law_id, recorded.valid_from.as_deref())
        .ok()
        .map(RegulationRead::of);
    match same {
        None => return change(None),
        Some(same) if same.hash != recorded.hash => return change(Some(same)),
        Some(_) => {}
    }
    let in_force = service
        .resolver()
        .get_law_for_date(&recorded.law_id, Some(day))
        .map(RegulationRead::of)?;
    // Dates as `YYYY-MM-DD` order as text.
    let later = match (&in_force.valid_from, &recorded.valid_from) {
        (Some(now), Some(then)) => now > then,
        _ => false,
    };
    if later { change(Some(in_force)) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(law_id: &str, valid_from: &str, hash: &str) -> RegulationRead {
        RegulationRead {
            law_id: law_id.into(),
            valid_from: Some(valid_from.into()),
            hash: Some(hash.into()),
        }
    }

    #[test]
    fn changes_are_named_one_after_another() {
        let since = LawSince::Changed(vec![
            LawChange {
                recorded: read("a", "2026-01-01", "sha256:1"),
                now: None,
            },
            LawChange {
                recorded: read("b", "2026-01-01", "sha256:2"),
                now: Some(read("b", "2026-03-01", "sha256:3")),
            },
        ]);
        assert_eq!(
            since.to_string(),
            "since the record, a no longer has version 2026-01-01; b is in force in version \
             2026-03-01 that day, not version 2026-01-01"
        );
    }
}
