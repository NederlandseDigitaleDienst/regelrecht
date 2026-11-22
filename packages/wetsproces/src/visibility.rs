//! What an explanation of a decision may show the one who reads it. A
//! preview, a decision just taken and `waarom` carry why the decision came
//! out so (`kern::Waarom`): the chain of steps, the open terms and who filled
//! them, and per answer of a cell how far that cell read its own chronicle.
//! That last part names records of the answering cell, and the raw trace
//! (`spoor`) holds everything the engine read. A reader sees neither of
//! another cell: an employee of the executor sees the executor's records and
//! what reached it from another cell, a citizen their own dossier.
//!
//! [`for_reader`] strips what the reader may not see: the raw trace, how far
//! another cell read its chronicle, and any record id outside what the reader
//! may see. The chain, the answers themselves and their values stay: an
//! answer is what the other cell gave the deciding cell.
//!
//! The admin panel's routes, through which an admin acts for an external
//! party, are not read this way: the admin enters that party's decisions
//! and sees its records.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// The keys of an answer of a cell (`AnswerProvenance` of the engine, and the
/// kern's own form of it in `antwoorden`): who answered, and how far it read
/// its chronicle (how many records, and the last).
const PROVENANCE: [(&str, [&str; 2]); 2] = [
    ("answerer", ["records_read", "last_record"]),
    ("door", ["gelezen", "tot_en_met"]),
];

/// The length of a record id: the chronicles give them out as UUIDs.
const ID_LEN: usize = 36;

/// Whether `s` has the form the chronicles give out record ids in: a UUID.
fn is_record_id(s: &str) -> bool {
    s.len() == ID_LEN
        && s.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

/// `text` with each record id in it the reader may not see replaced by an
/// ellipsis, such as an id inside a message of the cell; `None` when it
/// names none.
fn without_unseen_ids(text: &str, visible: &dyn Fn(&str) -> bool) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut rest = text;
    while !rest.is_empty() {
        let candidate = rest.get(..ID_LEN).filter(|c| is_record_id(c));
        match candidate {
            Some(id) if !visible(id) => {
                out.push('…');
                rest = &rest[ID_LEN..];
                changed = true;
            }
            _ => {
                let Some(c) = rest.chars().next() else { break };
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    changed.then_some(out)
}

/// The ids of the records in `value` that the cell `reader` recorded itself,
/// such as the decision a preview would take: a record names its cell
/// (`recording_actor`).
fn own_records(value: &Value, reader: &str, into: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if map.get("recording_actor").and_then(Value::as_str) == Some(reader)
                && let Some(id) = map.get("id").and_then(Value::as_str)
            {
                into.insert(id.to_string());
            }
            map.values().for_each(|v| own_records(v, reader, into));
        }
        Value::Array(items) => items.iter().for_each(|v| own_records(v, reader, into)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Strip from `map` how far a cell other than `reader` read its chronicle.
fn strip_provenance(map: &mut Map<String, Value>, reader: Option<&str>) {
    for (who, read) in PROVENANCE {
        let Some(answerer) = map.get(who).and_then(Value::as_str) else {
            continue;
        };
        let is_answer = read.iter().any(|k| map.contains_key(*k));
        if is_answer && Some(answerer) != reader {
            for k in read {
                map.remove(k);
            }
        }
    }
}

fn strip(value: &mut Value, reader: Option<&str>, visible: &dyn Fn(&str) -> bool) {
    match value {
        Value::Object(map) => {
            // The raw trace beside the chain: everything the engine read, in
            // text. No screen shows it; the chain says the same in steps.
            if map.contains_key("keten") {
                map.remove("spoor");
            }
            strip_provenance(map, reader);
            map.values_mut().for_each(|v| strip(v, reader, visible));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| strip(v, reader, visible)),
        Value::String(s) if is_record_id(s) => {
            if !visible(s) {
                *value = Value::Null;
            }
        }
        Value::String(s) => {
            if let Some(text) = without_unseen_ids(s, visible) {
                *s = text;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// `value`, an answer that explains a decision, as the reader may see it.
/// `reader` is the cell the reader works for, if any: its own answers keep
/// how far it read its chronicle, and the records it recorded in `value`
/// count as seen. `visible` says which other record ids the reader may see;
/// any other id becomes `null`, or an ellipsis inside a text.
pub fn for_reader(mut value: Value, reader: Option<&str>, visible: &BTreeSet<String>) -> Value {
    let mut own = BTreeSet::new();
    if let Some(reader) = reader {
        own_records(&value, reader, &mut own);
    }
    let may_see = |id: &str| visible.contains(id) || own.contains(id);
    strip(&mut value, reader, &may_see);
    value
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const MINE: &str = "00000000-0000-4000-8000-000000000001";
    const THEIRS: &str = "00000000-0000-4000-8000-000000000002";
    const NEW: &str = "00000000-0000-4000-8000-000000000003";

    #[test]
    fn another_cells_reading_its_chronicle_and_its_ids_do_not_reach_the_reader() {
        let preview = json!({
            "voorlopig": {
                "besluit": { "id": NEW, "recording_actor": "gemeente", "refers_to": MINE },
                "waarom": {
                    "spoor": format!("read {THEIRS}"),
                    "keten": [{ "soort": "begrip", "ingevuld_door": { "answer": { "answers": [
                        { "answerer": "ggd", "answer": "a", "records_read": 2, "last_record": THEIRS },
                        { "answerer": "gemeente", "answer": "b", "records_read": 1, "last_record": MINE },
                    ] } } }],
                    "antwoorden": [
                        { "door": "ggd", "antwoord": "a", "gelezen": 2, "tot_en_met": THEIRS, "waarde": true },
                    ],
                    "elders": THEIRS,
                },
            },
            "fout": format!("no cell holds '{THEIRS}' after '{MINE}'"),
        });
        let visible = BTreeSet::from([MINE.to_string()]);
        let seen = for_reader(preview, Some("gemeente"), &visible);
        let waarom = &seen["voorlopig"]["waarom"];
        assert!(waarom.get("spoor").is_none());
        let answers = &waarom["keten"][0]["ingevuld_door"]["answer"]["answers"];
        assert_eq!(answers[0], json!({ "answerer": "ggd", "answer": "a" }));
        assert_eq!(answers[1]["last_record"], json!(MINE));
        assert_eq!(
            waarom["antwoorden"][0],
            json!({ "door": "ggd", "antwoord": "a", "waarde": true })
        );
        assert_eq!(waarom["elders"], Value::Null);
        assert_eq!(
            seen["fout"],
            json!(format!("no cell holds '…' after '{MINE}'"))
        );
        // The decision the preview would take is the reader's own.
        assert_eq!(seen["voorlopig"]["besluit"]["id"], json!(NEW));
    }

    #[test]
    fn a_reader_of_no_cell_sees_no_cell_read_its_chronicle() {
        let waarom = json!({ "antwoorden": [
            { "door": "gemeente", "antwoord": "b", "gelezen": 1, "tot_en_met": MINE },
        ] });
        let seen = for_reader(waarom, None, &BTreeSet::from([MINE.to_string()]));
        assert_eq!(
            seen["antwoorden"][0],
            json!({ "door": "gemeente", "antwoord": "b" })
        );
    }
}
