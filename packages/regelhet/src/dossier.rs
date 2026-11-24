//! The dossier of one person: the records of every cell that are about
//! them, derived from the chronicles on every read.
//!
//! A record is about the person when it holds their identifier in the
//! case's login field (an application, an advice about the applicant), or
//! when it links to a record that is: a decision refers to the application
//! it is taken on, an objection to the decision it contests, a receipt to
//! the copy it received ([`links`]). Nothing is kept beside the chronicles:
//! the dossier is a view of them, rebuilt whenever it is read, so it cannot
//! drift from what the cells recorded.

use std::collections::BTreeSet;

use regelrecht_cel::Gram;
use serde_json::Value;

/// The ids `gram` links to: the record it refers to, the record it comes
/// after, and the record of another cell it received.
fn links(gram: &Gram) -> impl Iterator<Item = &str> {
    gram.refers_to
        .as_deref()
        .into_iter()
        .chain(gram.after.as_deref())
        .chain(gram.received.as_ref().map(|r| r.id.as_str()))
}

/// Whether `gram` holds `subject` in the field `field`.
fn names(gram: &Gram, field: &str, subject: &str) -> bool {
    matches!(gram.fields.get(field), Some(Value::String(s)) if s == subject)
}

/// The ids of the records among `grams` that are about `subject`, the
/// identifier the login field `field` holds: those that hold it, and every
/// record that links to one of them, however many links away.
pub fn ids<'g>(grams: &[&'g Gram], field: &str, subject: &str) -> BTreeSet<&'g str> {
    let mut ids: BTreeSet<&str> = grams
        .iter()
        .filter(|g| names(g, field, subject))
        .map(|g| g.id.as_str())
        .collect();
    // A record links only to records recorded before it, but the cells are
    // read one after another, so a pass may meet a link before its target:
    // repeat until a pass adds nothing.
    loop {
        let found: Vec<&str> = grams
            .iter()
            .filter(|g| !ids.contains(g.id.as_str()) && links(g).any(|l| ids.contains(l)))
            .map(|g| g.id.as_str())
            .collect();
        if found.is_empty() {
            return ids;
        }
        ids.extend(found);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn gram(id: &str, fields: &Value, refers_to: Option<&str>) -> Gram {
        serde_json::from_value(json!({
            "id": id,
            "kind": "AANVRAAG",
            "basis": "voorbeeld#1",
            "recording_actor": "gemeente",
            "recorded_on": "2026-10-13",
            "refers_to": refers_to,
            "fields": fields,
        }))
        .unwrap()
    }

    #[test]
    fn a_dossier_holds_what_names_the_subject_and_what_links_to_it() {
        let mine = gram("1", &json!({ "bsn": "999990019" }), None);
        let theirs = gram("2", &json!({ "bsn": "999990020" }), None);
        let decision = gram("3", &json!({}), Some("1"));
        let objection = gram("4", &json!({}), Some("3"));
        let their_decision = gram("5", &json!({}), Some("2"));
        // Read before what it links to.
        let all = [&objection, &decision, &mine, &theirs, &their_decision];
        let ids = ids(&all, "bsn", "999990019");
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), ["1", "3", "4"]);
    }

    #[test]
    fn only_a_text_equal_to_the_subject_names_it() {
        let number = gram("1", &json!({ "bsn": 999_990_019 }), None);
        let other_field = gram("2", &json!({ "bsn_partner": "999990019" }), None);
        let all = [&number, &other_field];
        assert!(ids(&all, "bsn", "999990019").is_empty());
    }
}
