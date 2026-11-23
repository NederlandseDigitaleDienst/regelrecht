//! Everything that happened about one application, in the order it
//! happened, across the chronicles of every cell of the case.
//!
//! Which records belong to an application follows from the links the
//! records carry themselves, and nothing else: the record a record refers to
//! (`refers_to`), the record it was taken after (`after`), the decision of
//! another cell it received (`received`), and for a recorded fact a field
//! whose value is the id of a record, such as the application a request to
//! another party was made for. A field of a submission or a decision is no
//! link: a citizen types its value, and could type any id. A request's receipt, the answer and the receipt of the answer name
//! their request by nothing in themselves; the kern pairs them
//! ([`crate::verzoek::verzoeken`]) and the caller passes those pairs as
//! `onder`. No kind, fact or article of a particular law is named here.
//!
//! [`wortel`] walks up those links from any record to the application at
//! the top; [`stukken`] collects everything that links to it, however many
//! links away, and puts it in order.
//!
//! The order: each chronicle keeps the order its cell recorded in, a record
//! comes after every record it links to, and between chronicles the day
//! each record was recorded on decides, then the order of the cells in the
//! case. A test clock that stands still puts a whole process on one day, so
//! the day alone cannot order a decision after the advice it read.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_cel::{EXECUTOGRAM, Gram};
use serde_json::Value;

use crate::verzoek::Verzoek;

/// Per record of a request of the case's cell, the record it hangs under:
/// the other party's receipt and the answer under the request, the receipt
/// of the answer under the answer.
pub fn onder_verzoeken(verzoeken: &[Verzoek]) -> BTreeMap<String, String> {
    let mut onder = BTreeMap::new();
    for v in verzoeken {
        if let Some(o) = &v.ontvangen {
            onder.insert(o.id.clone(), v.verzoek.id.clone());
        }
        if let Some(a) = &v.antwoord {
            onder.insert(a.id.clone(), v.verzoek.id.clone());
            if let Some(o) = &v.antwoord_ontvangen {
                onder.insert(o.id.clone(), a.id.clone());
            }
        }
    }
    onder
}

/// The ids `gram` links to, in the order an upward walk prefers them: what
/// it received from another cell, what it refers to, what it was taken
/// after, what `onder` hangs it under, and for an executogram the text
/// values of its fields.
/// Which of them name a record, the caller checks.
fn links<'a>(gram: &'a Gram, onder: &'a BTreeMap<String, String>) -> impl Iterator<Item = &'a str> {
    gram.received
        .as_ref()
        .map(|r| r.id.as_str())
        .into_iter()
        .chain(gram.refers_to.as_deref())
        .chain(gram.after.as_deref())
        .chain(onder.get(&gram.id).map(String::as_str))
        .chain(
            gram.fields
                .values()
                .filter(|_| gram.kind == EXECUTOGRAM)
                .filter_map(Value::as_str),
        )
        .filter(move |id| *id != gram.id)
}

/// The record at the top of the chain `id` is in: from `id`, the first
/// record each one links to ([`links`]), until one links to none. `None`
/// when no record among `grams` has the id `id`.
pub fn wortel<'g>(
    grams: &[&'g Gram],
    onder: &BTreeMap<String, String>,
    id: &str,
) -> Option<&'g Gram> {
    let per_id: BTreeMap<&str, &'g Gram> = grams.iter().map(|g| (g.id.as_str(), *g)).collect();
    let mut huidig = *per_id.get(id)?;
    let mut gezien = BTreeSet::from([huidig.id.as_str()]);
    // Each step moves to a record not seen yet, so the walk ends.
    while let Some(boven) = links(huidig, onder)
        .filter(|l| !gezien.contains(l))
        .find_map(|l| per_id.get(l).copied())
    {
        gezien.insert(boven.id.as_str());
        huidig = boven;
    }
    Some(huidig)
}

/// The records of `kronieken` (one chronicle per cell, in the order of the
/// case) that belong to the record `wortel`: itself, and every record that
/// links to one that belongs, however many links away. In the order they
/// happened (see the module).
pub fn stukken<'g>(
    kronieken: &[&'g [Gram]],
    onder: &BTreeMap<String, String>,
    wortel: &str,
) -> Vec<&'g Gram> {
    let mut set: BTreeSet<&str> = BTreeSet::from([wortel]);
    let alle: Vec<&'g Gram> = kronieken.iter().flat_map(|k| k.iter()).collect();
    if !alle.iter().any(|g| g.id == wortel) {
        return Vec::new();
    }
    // A record links only to records recorded before it, but the cells are
    // read one after another: repeat until a pass adds nothing.
    loop {
        let erbij: Vec<&str> = alle
            .iter()
            .filter(|g| !set.contains(g.id.as_str()))
            .filter(|g| links(g, onder).any(|l| set.contains(l)))
            .map(|g| g.id.as_str())
            .collect();
        if erbij.is_empty() {
            break;
        }
        set.extend(erbij);
    }
    in_volgorde(kronieken, onder, &set)
}

/// The records of `kronieken` whose id is in `set`, in the order they
/// happened: a topological order of "recorded before in the same chronicle"
/// and "linked to", the earliest day first, then the first cell.
fn in_volgorde<'g>(
    kronieken: &[&'g [Gram]],
    onder: &BTreeMap<String, String>,
    set: &BTreeSet<&str>,
) -> Vec<&'g Gram> {
    // Per record: its sort key (day, cell, place) and the records before it.
    let mut knopen: Vec<(&'g Gram, (chrono::NaiveDate, usize, usize))> = Vec::new();
    for (cel, kroniek) in kronieken.iter().enumerate() {
        for (plek, g) in kroniek.iter().enumerate() {
            if set.contains(g.id.as_str()) {
                knopen.push((g, (g.recorded_on, cel, plek)));
            }
        }
    }
    let index: BTreeMap<&str, usize> = knopen
        .iter()
        .enumerate()
        .map(|(i, (g, _))| (g.id.as_str(), i))
        .collect();
    let mut voor: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); knopen.len()];
    for (i, (g, (_, cel, _))) in knopen.iter().enumerate() {
        // The record before it in its own chronicle.
        if let Some(j) = (0..i).rev().find(|j| knopen[*j].1.1 == *cel) {
            voor[i].insert(j);
        }
        for l in links(g, onder) {
            if let Some(j) = index.get(l).filter(|j| **j != i) {
                voor[i].insert(*j);
            }
        }
    }
    let mut gedaan = vec![false; knopen.len()];
    let mut uit = Vec::with_capacity(knopen.len());
    while uit.len() < knopen.len() {
        let klaar = (0..knopen.len())
            .filter(|i| !gedaan[*i] && voor[*i].iter().all(|j| gedaan[*j]))
            .min_by_key(|i| knopen[*i].1);
        // A cycle cannot come from records that link only to earlier ones;
        // should one arise, the rest goes in key order rather than nowhere.
        let volgende = klaar.or_else(|| {
            (0..knopen.len())
                .filter(|i| !gedaan[*i])
                .min_by_key(|i| knopen[*i].1)
        });
        let Some(i) = volgende else { break };
        gedaan[i] = true;
        uit.push(knopen[i].0);
    }
    uit
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn gram(id: &str, cel: &str, day: &str, links: &Value) -> Gram {
        let mut g = json!({
            "id": id,
            "kind": "BESCHIKKING",
            "basis": "voorbeeld#1",
            "recording_actor": cel,
            "recorded_on": day,
            "fields": {},
        });
        if let (Some(g), Some(l)) = (g.as_object_mut(), links.as_object()) {
            g.extend(l.clone());
        }
        serde_json::from_value(g).unwrap()
    }

    #[test]
    fn a_timeline_holds_what_links_to_the_application_and_nothing_else() {
        let eigen = [
            gram("a", "gemeente", "2026-10-13", &json!({})),
            gram(
                "v",
                "gemeente",
                "2026-10-13",
                &json!({ "kind": "EXECUTOGRAM", "fields": { "aanvraag": "a" } }),
            ),
            gram("ander", "gemeente", "2026-10-13", &json!({})),
            gram("ontvangst-advies", "gemeente", "2026-10-13", &json!({})),
            gram("b", "gemeente", "2026-10-13", &json!({ "refers_to": "a" })),
            gram(
                "anders-besluit",
                "gemeente",
                "2026-10-13",
                &json!({ "refers_to": "ander" }),
            ),
        ];
        let ggd = [
            gram(
                "v-ontvangen",
                "ggd",
                "2026-10-13",
                &json!({ "kind": "EXECUTOGRAM", "fields": { "aanvraag": "a" } }),
            ),
            gram("advies", "ggd", "2026-10-13", &json!({})),
        ];
        let rechtbank = [gram(
            "beroep",
            "rechtbank",
            "2026-10-13",
            &json!({ "received": { "from": "gemeente", "id": "b" } }),
        )];
        let onder = BTreeMap::from([
            ("advies".to_string(), "v".to_string()),
            ("ontvangst-advies".to_string(), "advies".to_string()),
        ]);
        let kronieken: [&[Gram]; 3] = [&eigen, &ggd, &rechtbank];
        let ids: Vec<&str> = stukken(&kronieken, &onder, "a")
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        // One day, so the links order it: the advice comes in before the
        // decision that read it, and the appeal after that decision.
        assert_eq!(
            ids,
            [
                "a",
                "v",
                "v-ontvangen",
                "advies",
                "ontvangst-advies",
                "b",
                "beroep"
            ]
        );
        let alle: Vec<&Gram> = kronieken.iter().flat_map(|k| k.iter()).collect();
        for id in ids {
            assert_eq!(wortel(&alle, &onder, id).unwrap().id, "a", "{id}");
        }
        assert_eq!(wortel(&alle, &onder, "anders-besluit").unwrap().id, "ander");
        assert!(wortel(&alle, &onder, "onbekend").is_none());
    }

    #[test]
    fn between_chronicles_an_earlier_day_comes_first() {
        let eigen = [
            gram("a", "gemeente", "2026-10-13", &json!({})),
            gram(
                "later",
                "gemeente",
                "2026-10-20",
                &json!({ "refers_to": "a" }),
            ),
        ];
        let ander = [gram(
            "eerder",
            "ggd",
            "2026-10-15",
            &json!({ "kind": "EXECUTOGRAM", "fields": { "over": "a" } }),
        )];
        let kronieken: [&[Gram]; 2] = [&eigen, &ander];
        let ids: Vec<&str> = stukken(&kronieken, &BTreeMap::new(), "a")
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "eerder", "later"]);
    }

    #[test]
    fn a_field_of_a_submission_links_to_nothing_whatever_id_it_holds() {
        let eigen = [
            gram("vreemd", "gemeente", "2026-10-13", &json!({})),
            gram(
                "a",
                "gemeente",
                "2026-10-13",
                &json!({ "kind": "AANVRAAG", "fields": { "toelichting": "vreemd" } }),
            ),
        ];
        let kronieken: [&[Gram]; 1] = [&eigen];
        let alle: Vec<&Gram> = eigen.iter().collect();
        assert_eq!(wortel(&alle, &BTreeMap::new(), "a").unwrap().id, "a");
        let ids: Vec<&str> = stukken(&kronieken, &BTreeMap::new(), "vreemd")
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        assert_eq!(ids, ["vreemd"]);
    }
}
