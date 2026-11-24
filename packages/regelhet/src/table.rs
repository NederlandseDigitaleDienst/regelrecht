//! A table from the chronicle: what a table element of the UI spec
//! ([`Tabel`]) shows beside one submission, with per row where it comes from.
//!
//! The rows are records: an array of records the submission holds (a field,
//! or an output), an array its article gives on the records as they are now,
//! or every record of a fact in the chronicles. The columns are the keys the
//! UI spec names. Per row the table says where it comes from: the record that
//! holds it (the submission, or the executogram of the fact), and for each
//! fact the UI spec names under `provenance` the last record of it, in any
//! cell's chronicle, whose fields equal the row on the keys of `match`; with
//! no `match`, on every key the row and the record share whose value is no
//! list or record. That is how a screen shows, beside a number the
//! submission holds, the record of another cell it was taken from, and on
//! which day that cell recorded it. A row that no such record matches says
//! so, rather than leaving it out.
//!
//! Rows from a fact are the records of that fact that belong to the
//! submission: those whose fields equal, on the keys they share, a row of a
//! list of records the submission holds, or else a field of the submission
//! itself. A fact that shares no key with the submission gives every record.

use regelrecht_cel::chronicle::{EXECUTOGRAM, Gram};
use regelrecht_poc_chronolexografie::ui_spec::Tabel;
use serde::Serialize;
use serde_json::{Map, Value};

/// One record a row comes from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    /// The cell that recorded it.
    #[serde(rename = "cel")]
    pub cell: String,
    /// The fact it records, `<regelwerk>#<article>#<fact>`, or the article
    /// of a submission.
    #[serde(rename = "wat")]
    pub what: String,
    /// The record itself, by id.
    #[serde(rename = "stuk")]
    pub record: String,
    /// The day the cell recorded it.
    #[serde(rename = "op")]
    pub on: String,
}

/// One row of a table, with where it comes from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// Per column its value; a key the row does not hold is `null`.
    #[serde(rename = "waarden")]
    pub values: Map<String, Value>,
    /// The records it comes from: the one that holds it first.
    #[serde(rename = "herkomst")]
    pub sources: Vec<Source>,
    /// The facts of `provenance` no record of which matched the row.
    #[serde(rename = "zonder_herkomst", skip_serializing_if = "Vec::is_empty")]
    pub unmatched: Vec<String>,
}

/// A table as a screen shows it beside one submission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Table {
    /// The table element of the UI spec.
    #[serde(rename = "tabel")]
    pub spec: Tabel,
    #[serde(rename = "rijen")]
    pub rows: Vec<Row>,
    /// Why the table has no rows where it should have: the submission holds
    /// no array under the field, say.
    #[serde(rename = "leeg", skip_serializing_if = "Option::is_none")]
    pub empty: Option<String>,
}

/// The fact a record records, `<regelwerk>#<article>#<fact>`, if it is an
/// executogram.
fn fact_of(gram: &Gram) -> Option<String> {
    (gram.kind == EXECUTOGRAM)
        .then(|| gram.fact.as_ref().map(|f| format!("{}#{}", gram.basis, f)))
        .flatten()
}

fn source(gram: &Gram, what: String) -> Source {
    Source {
        cell: gram.recording_actor.clone(),
        what,
        record: gram.id.clone(),
        on: gram.recorded_on.to_string(),
    }
}

/// The values of `row` per column of `spec`; every key of the row when the
/// spec names no column.
fn values(spec: &Tabel, row: &Map<String, Value>) -> Map<String, Value> {
    if spec.kolommen.is_empty() {
        return row.clone();
    }
    spec.kolommen
        .iter()
        .map(|k| {
            (
                k.name.clone(),
                row.get(&k.name).cloned().unwrap_or(Value::Null),
            )
        })
        .collect()
}

/// The rows of `array`, each a record: anything else in it is left out.
fn records(array: &Value) -> Vec<&Map<String, Value>> {
    array
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .collect()
}

/// Whether `v` is a single value: no list and no record.
fn scalar(v: &Value) -> bool {
    !v.is_array() && !v.is_object()
}

/// The keys `row` and `gram` share with a single value in both.
fn shared(row: &Map<String, Value>, gram: &Gram) -> Vec<String> {
    row.iter()
        .filter(|(k, v)| scalar(v) && gram.fields.get(*k).is_some_and(scalar))
        .map(|(k, _)| k.clone())
        .collect()
}

/// Whether `gram` equals `row` on `keys`, of which there is at least one.
fn equal_on(row: &Map<String, Value>, gram: &Gram, keys: &[String]) -> bool {
    !keys.is_empty()
        && keys
            .iter()
            .all(|k| row.get(k).is_some_and(|v| gram.fields.get(k) == Some(v)))
}

/// Whether the record `gram` belongs to `submission`: it equals a row of a
/// list of records the submission holds on the keys they share, or else the
/// submission itself on the fields they share; `None` when they share no
/// key at all, and nothing tells.
fn belongs(gram: &Gram, submission: &Gram) -> Option<bool> {
    let rows: Vec<&Map<String, Value>> = submission
        .fields
        .values()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_object)
        .collect();
    let mut told = false;
    for row in rows {
        let keys = shared(row, gram);
        if keys.is_empty() {
            continue;
        }
        told = true;
        if equal_on(row, gram, &keys) {
            return Some(true);
        }
    }
    if told {
        return Some(false);
    }
    let own: Map<String, Value> = submission
        .fields
        .clone()
        .into_iter()
        .filter(|(_, v)| !v.is_null())
        .collect();
    let keys = shared(&own, gram);
    (!keys.is_empty()).then(|| equal_on(&own, gram, &keys))
}

/// Where `row` comes from besides the record that holds it: per fact of
/// `provenance`, the last record in `chronicles` whose fields equal the row
/// on the keys of `match`, or on the keys they share when it names none.
fn provenance(
    spec: &Tabel,
    row: &Map<String, Value>,
    chronicles: &[Vec<Gram>],
) -> (Vec<Source>, Vec<String>) {
    let mut found = Vec::new();
    let mut unmatched = Vec::new();
    for h in &spec.herkomst {
        let last = chronicles
            .iter()
            .flatten()
            .filter(|g| fact_of(g).as_deref() == Some(h.fact.as_str()))
            .filter(|g| {
                let keys = if h.on.is_empty() {
                    shared(row, g)
                } else {
                    h.on.clone()
                };
                equal_on(row, g, &keys)
            })
            .max_by(|a, b| a.recorded_on.cmp(&b.recorded_on));
        match last {
            Some(g) => found.push(source(g, h.fact.clone())),
            None => unmatched.push(h.fact.clone()),
        }
    }
    (found, unmatched)
}

/// The table `spec` beside the submission `submission`, on `chronicles` (every
/// cell's records), with `outcomes` what its article gives on them (for rows
/// from an `outcome`).
pub fn table(spec: &Tabel, submission: &Gram, outcomes: &Value, chronicles: &[Vec<Gram>]) -> Table {
    let mut rows = Vec::new();
    let mut empty = None;
    if let Some(fact) = &spec.feit {
        for g in chronicles.iter().flatten() {
            if fact_of(g).as_deref() != Some(fact.as_str()) || belongs(g, submission) == Some(false)
            {
                continue;
            }
            let fields: Map<String, Value> = g.fields.clone().into_iter().collect();
            let (mut sources, unmatched) = provenance(spec, &fields, chronicles);
            sources.insert(0, source(g, fact.clone()));
            rows.push(Row {
                values: values(spec, &fields),
                sources,
                unmatched,
            });
        }
    } else {
        let (array, held_by) = match (&spec.veld, &spec.uitkomst) {
            (Some(field), _) => (submission.held(field), submission.basis.clone()),
            (None, Some(outcome)) => (outcomes.get(outcome), submission.basis.clone()),
            (None, None) => (None, submission.basis.clone()),
        };
        match array {
            Some(a) if a.is_array() => {
                for row in records(a) {
                    let (mut sources, unmatched) = provenance(spec, row, chronicles);
                    sources.insert(0, source(submission, held_by.clone()));
                    rows.push(Row {
                        values: values(spec, row),
                        sources,
                        unmatched,
                    });
                }
            }
            Some(Value::Null) | None => {
                empty = Some(format!(
                    "de indiening houdt niets onder '{}'",
                    spec.veld
                        .as_deref()
                        .or(spec.uitkomst.as_deref())
                        .unwrap_or_default()
                ));
            }
            Some(_) => {
                empty = Some(format!(
                    "'{}' is geen lijst van rijen",
                    spec.veld
                        .as_deref()
                        .or(spec.uitkomst.as_deref())
                        .unwrap_or_default()
                ));
            }
        }
    }
    Table {
        spec: spec.clone(),
        rows,
        empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use regelrecht_poc_chronolexografie::ui_spec::{TableColumn, TableProvenance};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn gram(
        id: &str,
        kind: &str,
        basis: &str,
        fact: Option<&str>,
        cell: &str,
        on: &str,
        fields: Value,
    ) -> Gram {
        Gram {
            id: id.into(),
            kind: kind.into(),
            basis: basis.into(),
            fact: fact.map(str::to_string),
            refers_to: None,
            received: None,
            after: None,
            recording_actor: cell.into(),
            recorded_on: NaiveDate::parse_from_str(on, "%Y-%m-%d").unwrap(),
            moment: None,
            fields: serde_json::from_value::<BTreeMap<String, Value>>(fields).unwrap(),
            missing: Vec::new(),
            outputs: BTreeMap::new(),
            law_versions: Vec::new(),
        }
    }

    fn spec() -> Tabel {
        Tabel {
            naam: "T1".into(),
            element: Some("plaatsen_per_wijk".into()),
            rol: Some("behandelaar".into()),
            rol_generiek: None,
            veld: Some("plaatsen".into()),
            uitkomst: None,
            feit: None,
            kolommen: vec![
                TableColumn {
                    name: "wijk".into(),
                    label: Some("Wijk".into()),
                },
                TableColumn {
                    name: "aantal".into(),
                    label: Some("Aantal".into()),
                },
            ],
            herkomst: vec![TableProvenance {
                fact: "telling#3#telling_ontvangen".into(),
                on: vec!["wijk".into()],
            }],
            label: None,
            help: None,
        }
    }

    #[test]
    fn a_row_names_the_submission_and_the_last_matching_record_of_another_cell() {
        let aanvraag = gram(
            "a1",
            "AANVRAAG",
            "regeling#2",
            None,
            "gemeente",
            "2026-03-06",
            json!({"plaatsen": [{"wijk": "Noord", "aantal": 3, "extra": 1}, {"wijk": "Zuid", "aantal": 2}]}),
        );
        let telling = |id: &str, wijk: &str, on: &str| {
            gram(
                id,
                EXECUTOGRAM,
                "telling#3",
                Some("telling_ontvangen"),
                "bureau",
                on,
                json!({"wijk": wijk}),
            )
        };
        let chronicles = vec![
            vec![aanvraag.clone()],
            vec![
                telling("t1", "Noord", "2026-01-02"),
                telling("t2", "Noord", "2026-02-02"),
            ],
        ];
        let t = table(&spec(), &aanvraag, &Value::Null, &chronicles);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(
            t.rows[0].values,
            json!({"wijk": "Noord", "aantal": 3})
                .as_object()
                .unwrap()
                .clone()
        );
        assert_eq!(t.rows[0].sources[0].record, "a1");
        assert_eq!(
            t.rows[0].sources[1].record, "t2",
            "the last record that matches"
        );
        assert_eq!(t.rows[0].sources[1].on, "2026-02-02");
        // Zuid has no count: the row stays, and says so.
        assert_eq!(t.rows[1].sources.len(), 1);
        assert_eq!(
            t.rows[1].unmatched,
            vec!["telling#3#telling_ontvangen".to_string()]
        );
    }

    #[test]
    fn rows_from_a_fact_are_its_records() {
        let mut s = spec();
        s.veld = None;
        s.feit = Some("telling#3#telling_ontvangen".into());
        s.herkomst.clear();
        let aanvraag = gram(
            "a1",
            "AANVRAAG",
            "regeling#2",
            None,
            "gemeente",
            "2026-03-06",
            json!({}),
        );
        let chronicles = vec![vec![gram(
            "t1",
            EXECUTOGRAM,
            "telling#3",
            Some("telling_ontvangen"),
            "bureau",
            "2026-01-02",
            json!({"wijk": "Oost", "aantal": 4}),
        )]];
        let t = table(&s, &aanvraag, &Value::Null, &chronicles);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0].sources[0].cell, "bureau");
        assert_eq!(t.rows[0].values["aantal"], json!(4));
    }

    #[test]
    fn without_match_a_row_and_a_record_match_on_the_keys_they_share() {
        let mut s = spec();
        s.herkomst[0].on.clear();
        let aanvraag = gram(
            "a1",
            "AANVRAAG",
            "regeling#2",
            None,
            "gemeente",
            "2026-03-06",
            json!({"plaatsen": [{"wijk": "Noord", "aantal": 3}]}),
        );
        let telling = gram(
            "t1",
            EXECUTOGRAM,
            "telling#3",
            Some("telling_ontvangen"),
            "bureau",
            "2026-01-02",
            json!({"wijk": "Noord", "lijst": [1, 2]}),
        );
        let t = table(
            &s,
            &aanvraag,
            &Value::Null,
            &[vec![aanvraag.clone()], vec![telling]],
        );
        assert_eq!(t.rows[0].sources.len(), 2);
        assert!(t.rows[0].unmatched.is_empty());
    }

    #[test]
    fn rows_from_a_fact_keep_to_the_records_of_the_submission() {
        let mut s = spec();
        s.veld = None;
        s.feit = Some("telling#3#telling_ontvangen".into());
        s.herkomst.clear();
        let aanvraag = gram(
            "a1",
            "AANVRAAG",
            "regeling#2",
            None,
            "gemeente",
            "2026-03-06",
            json!({"plaatsen": [{"wijk": "Noord", "aantal": 3}]}),
        );
        let telling = |id: &str, wijk: &str| {
            gram(
                id,
                EXECUTOGRAM,
                "telling#3",
                Some("telling_ontvangen"),
                "bureau",
                "2026-01-02",
                json!({"wijk": wijk, "geteld": 9}),
            )
        };
        let t = table(
            &s,
            &aanvraag,
            &Value::Null,
            &[vec![telling("t1", "Noord"), telling("t2", "Zuid")]],
        );
        assert_eq!(t.rows.len(), 1, "only Noord belongs to the submission");
        assert_eq!(t.rows[0].values["wijk"], json!("Noord"));
        // A fact that shares nothing with the submission gives every record.
        let los = gram(
            "a2",
            "AANVRAAG",
            "regeling#2",
            None,
            "gemeente",
            "2026-03-06",
            json!({"iets": 1}),
        );
        assert_eq!(
            table(
                &s,
                &los,
                &Value::Null,
                &[vec![telling("t1", "Noord"), telling("t2", "Zuid")]]
            )
            .rows
            .len(),
            2
        );
    }

    #[test]
    fn a_submission_without_the_array_says_why_the_table_is_empty() {
        let aanvraag = gram(
            "a1",
            "AANVRAAG",
            "regeling#2",
            None,
            "gemeente",
            "2026-03-06",
            json!({}),
        );
        let t = table(&spec(), &aanvraag, &Value::Null, &[]);
        assert!(t.rows.is_empty());
        assert!(t.empty.is_some());
    }
}
