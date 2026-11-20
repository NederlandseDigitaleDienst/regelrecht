//! What the admin panel shows of an external party of the case: a cell the
//! process needs, such as the GGD that advises or the court that decides on
//! an appeal, for which nobody works in this server. Until a real link with
//! such a party exists, an admin enters what it did on its behalf
//! ([`crate::api`], the routes under `/api/backoffice/beheer/`).
//!
//! The panel shows of a party only what that input needs, never its whole
//! chronicle: the submissions that wait for its decision ([`open`]) and its
//! decisions of which no copy reached another cell yet ([`unsent`]). Which
//! facts it can record, the forms say (`andere_cellen`). Everything here is
//! read from the records and the case alone; no step of a particular law is
//! named.

use std::collections::BTreeSet;

use regelrecht_cel::{EXECUTOGRAM, Gram};

/// The kinds of record that are a submission, as the case's regelwerken
/// establish them (`AANVRAAG`, `BEZWAAR`, `BEROEP`).
#[derive(Debug, Clone, Default)]
pub struct SubmissionKinds(BTreeSet<String>);

impl SubmissionKinds {
    pub fn new(kinds: impl IntoIterator<Item = String>) -> Self {
        Self(kinds.into_iter().collect())
    }

    /// Whether `gram` is a submission.
    pub fn is_submission(&self, gram: &Gram) -> bool {
        self.0.contains(&gram.kind)
    }

    /// Whether `gram` is a decision: neither a submission nor a recorded
    /// fact.
    pub fn is_decision(&self, gram: &Gram) -> bool {
        !self.is_submission(gram) && gram.kind != EXECUTOGRAM
    }
}

/// The submissions in the chronicle `grams` on which no decision was taken
/// yet, in chronicle order.
pub fn open<'g>(grams: &'g [Gram], kinds: &SubmissionKinds) -> Vec<&'g Gram> {
    grams
        .iter()
        .filter(|g| kinds.is_submission(g))
        .filter(|g| {
            !grams
                .iter()
                .any(|d| d.refers_to.as_deref() == Some(g.id.as_str()) && kinds.is_decision(d))
        })
        .collect()
}

/// The decisions in the chronicle `grams` taken on a submission in it, of
/// which no receipt in `elsewhere` (the other cells' records) holds a copy
/// yet: those the party can still send to the cell they concern. A receipt
/// is a recorded fact that received the decision from this party; a
/// submission elsewhere that contests the decision also carries what it
/// received, and is no copy sent. Only
/// decisions on a submission whose receipt the law requires are offered,
/// those in `copied` by the submission's kind; the kern refuses the rest.
pub fn unsent<'g>(
    grams: &'g [Gram],
    elsewhere: &[&Gram],
    kinds: &SubmissionKinds,
    copied: &BTreeSet<String>,
) -> Vec<&'g Gram> {
    grams
        .iter()
        .filter(|d| kinds.is_decision(d))
        .filter(|d| {
            d.refers_to
                .as_deref()
                .is_some_and(|op| grams.iter().any(|s| s.id == op && copied.contains(&s.kind)))
        })
        .filter(|d| {
            !elsewhere.iter().any(|g| {
                g.kind == EXECUTOGRAM
                    && g.received
                        .as_ref()
                        .is_some_and(|r| r.id == d.id && r.from == d.recording_actor)
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn gram(id: &str, kind: &str, refers_to: Option<&str>) -> Gram {
        let mut g = json!({
            "id": id,
            "kind": kind,
            "basis": "awb#8:70",
            "recording_actor": "rechtbank",
            "recorded_on": "2026-10-13",
            "fields": {},
        });
        if let Some(r) = refers_to {
            g["refers_to"] = json!(r);
        }
        serde_json::from_value(g).unwrap()
    }

    fn receipt(id: &str, of: &str) -> Gram {
        let mut g = gram(id, EXECUTOGRAM, None);
        g.received = Some(regelrecht_cel::Received {
            from: "rechtbank".into(),
            id: of.into(),
        });
        g
    }

    fn kinds() -> SubmissionKinds {
        SubmissionKinds::new(["AANVRAAG".to_string(), "BEROEP".to_string()])
    }

    #[test]
    fn a_submission_is_open_until_a_decision_refers_to_it() {
        let grams = [
            gram("b1", "BEROEP", None),
            gram("b2", "BEROEP", None),
            gram("f", EXECUTOGRAM, Some("b2")),
            gram("u1", "UITSPRAAK", Some("b1")),
        ];
        let open: Vec<&str> = open(&grams, &kinds())
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        // A recorded fact about a submission decides nothing.
        assert_eq!(open, ["b2"]);
    }

    #[test]
    fn a_decision_is_unsent_until_another_cell_received_a_copy() {
        let grams = [
            gram("b1", "BEROEP", None),
            gram("u1", "UITSPRAAK", Some("b1")),
            gram("b2", "BEROEP", None),
            gram("u2", "UITSPRAAK", Some("b2")),
            gram("a1", "AANVRAAG", None),
            gram("d1", "BESCHIKKING", Some("a1")),
        ];
        let copied = BTreeSet::from(["BEROEP".to_string()]);
        let received = receipt("r1", "u1");
        // A submission elsewhere that carries u2 is no copy of it.
        let mut contesting = gram("h1", "BEROEP", None);
        contesting.received = receipt("x", "u2").received;
        let unsent: Vec<&str> = unsent(&grams, &[&received, &contesting], &kinds(), &copied)
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        // u1 was received; d1 decides a kind whose receipt no law requires.
        assert_eq!(unsent, ["u2"]);
    }
}
