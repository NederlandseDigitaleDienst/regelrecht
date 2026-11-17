//! What the poc reads from the UI spec of a source (`ui_spec.yaml` at the
//! root of the source, the file its manifest names): per field of the
//! application, the source the form prefills it from, and the views the
//! authority sees beside each of its decisions ([`Weergave`]).
//!
//! A UI spec is a register of elements, each with what a screen shows, asks
//! and fills, and on what grounds. It comes in two forms:
//!
//! - per field: `elements`, each with `fields`, and a field the form
//!   prefills carries `fill: {from: <source>}`;
//! - the older form: `elementen`, each with a map `prefill` from field to
//!   source.
//!
//! A field of the per-field form may also carry `answer: {from: <source>,
//! decision: <parameter>}`: the element then shows, beside each decision of
//! the case's cell, what the cell answers for that source, with the id of the
//! decision as the parameter `decision`. The fields of such an element that
//! it asks (`visibility: asked`) are what the authority gives those sources
//! besides, such as a rate of interest no regelwerk holds.
//!
//! The poc reads only those sources and leaves every other key to the
//! register and its own checks, so a register that grows a key does not stop
//! a case from loading. A source is `<regelwerk>#<article>#<output>`; what it
//! gives the cell computes when the form opens ([`crate::kern::voorinvullen`]).
//!
//! A UI spec that has neither list is refused. If it were read as empty, a
//! renamed key would turn every prefill off without a word. A UI spec that mixes the two
//! forms is refused too.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_yaml_ng::Value;

/// The part of a UI spec the poc reads, in either form.
#[derive(Debug, Deserialize)]
struct Register {
    elements: Option<Vec<Element>>,
    elementen: Option<Vec<OldElement>>,
}

/// An element of the per-field form.
#[derive(Debug, Deserialize)]
struct Element {
    id: Option<Value>,
    /// The name of the element, as a view beside a decision is titled.
    element: Option<String>,
    #[serde(default)]
    fields: Vec<Field>,
    /// The key of the older form; present here, it is a mix-up.
    prefill: Option<IgnoredAny>,
}

/// A field of an element; the poc reads only how the form fills it. A
/// field without a name is left to the register, unless it is prefilled.
#[derive(Debug, Deserialize)]
struct Field {
    name: Option<String>,
    fill: Option<Fill>,
    visibility: Option<String>,
    answer: Option<Answer>,
}

/// What a field of a view beside a decision shows: the answer of the case's
/// cell for `from`, with the id of the decision as the parameter
/// `decision`. An unknown key is refused, as in [`Fill`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    from: String,
    decision: String,
}

/// What the poc reads from a UI spec: the prefill of the application, and the
/// views beside a decision.
#[derive(Debug, Default, Clone)]
pub struct UiSpec {
    /// Per field of the application, the source the form prefills it from.
    pub prefill: BTreeMap<String, String>,
    /// The elements that show answers beside each decision of the case's cell.
    pub weergaven: Vec<Weergave>,
}

/// An element that shows, beside each decision of the case's cell, what the
/// cell answers about it: such as what is still to be paid on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Weergave {
    /// The element's name, or its id.
    pub naam: String,
    pub antwoorden: Vec<Antwoord>,
    /// The fields the element asks: what the authority gives the sources
    /// besides the decision.
    pub gevraagd: Vec<String>,
}

/// One field of a [`Weergave`]: the answer for `bron`
/// (`<regelwerk>#<article>#<output>`), with the id of the decision as the
/// parameter `besluit`.
#[derive(Debug, Clone, PartialEq)]
pub struct Antwoord {
    pub veld: String,
    pub bron: String,
    pub besluit: String,
}

/// How a field is filled: from a source (`from`), or by the element itself
/// (`by`, which the poc does not read). An unknown key is refused, so that a
/// misspelt `from` does not drop a prefill.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    from: Option<String>,
    by: Option<IgnoredAny>,
}

/// An element of the older form.
#[derive(Debug, Deserialize)]
struct OldElement {
    id: Option<Value>,
    #[serde(default)]
    prefill: BTreeMap<String, String>,
    /// The key of the per-field form; present here, it is a mix-up.
    fields: Option<IgnoredAny>,
}

/// One element's prefill, with the name it goes by in a message.
struct ElementPrefill {
    name: String,
    sources: Vec<(String, String)>,
}

/// Per field the source the form prefills it from, over every element of the
/// UI spec `tekst` ([`lees`]).
///
/// # Errors
///
/// As [`lees`].
pub fn prefill(tekst: &str) -> Result<BTreeMap<String, String>, String> {
    lees(tekst).map(|spec| spec.prefill)
}

/// What the poc reads from the UI spec `tekst`: per field the source the form
/// prefills it from, over every element (two elements may name the same
/// source for a field, as the form and the sheet that edits a field do), and
/// the views beside a decision.
///
/// # Errors
///
/// The text when it is no UI spec; a UI spec with neither `elements` nor
/// `elementen`, or with both; an element of one form that carries the key of
/// the other; a `fill` with a key other than `from` or `by`, or with neither;
/// a field with both `fill` and `answer`, or an `answer` without a `name`;
/// a source that is no `<regelwerk>#<article>#<output>`; and a field two
/// elements prefill from different sources.
pub fn lees(tekst: &str) -> Result<UiSpec, String> {
    let register: Register =
        serde_yaml_ng::from_str(tekst).map_err(|e| format!("the UI spec: {e}"))?;
    let (elements, weergaven) = match (register.elements, register.elementen) {
        (Some(elements), None) => per_field(elements)?,
        (None, Some(elementen)) => (older_form(elementen)?, Vec::new()),
        (Some(_), Some(_)) => {
            return Err("the UI spec has both `elements` and `elementen`; keep one form".into());
        }
        (None, None) => {
            return Err(
                "the UI spec has no `elements` (nor the older `elementen`), \
                 so it would prefill nothing"
                    .into(),
            );
        }
    };
    Ok(UiSpec {
        prefill: merge(elements)?,
        weergaven,
    })
}

/// The prefill of the elements of the per-field form, and the views beside a
/// decision among them.
fn per_field(elements: Vec<Element>) -> Result<(Vec<ElementPrefill>, Vec<Weergave>), String> {
    let mut uit = Vec::new();
    let mut weergaven = Vec::new();
    for (i, element) in elements.into_iter().enumerate() {
        let name = element_name(element.id.as_ref(), i);
        if let Some(weergave) = weergave(&name, element.element.as_deref(), &element.fields)? {
            weergaven.push(weergave);
        }
        if element.prefill.is_some() {
            return Err(format!(
                "the UI spec: {name} has `prefill`, a key of the older form; \
                 in `elements` a field says `fill: {{from: ...}}`"
            ));
        }
        let mut sources = Vec::new();
        for field in element.fields {
            let Some(fill) = field.fill else { continue };
            let veld = field.name.unwrap_or_default();
            match (fill.from, fill.by.is_some()) {
                (_, false) | (Some(_), true) if veld.is_empty() => {
                    return Err(format!(
                        "the UI spec: {name} fills a field without a `name`"
                    ));
                }
                (Some(_), true) => {
                    return Err(format!(
                        "the UI spec: {name} fills '{veld}' both `from` a source and `by` itself"
                    ));
                }
                (Some(from), false) => sources.push((veld, from)),
                (None, true) => {}
                (None, false) => {
                    return Err(format!(
                        "the UI spec: {name} fills '{veld}' with neither `from` nor `by`"
                    ));
                }
            }
        }
        uit.push(ElementPrefill { name, sources });
    }
    Ok((uit, weergaven))
}

/// The view beside a decision that the element `name` (titled `titel`) makes
/// of its `fields`, or none when no field carries an `answer`.
fn weergave(name: &str, titel: Option<&str>, fields: &[Field]) -> Result<Option<Weergave>, String> {
    let mut antwoorden = Vec::new();
    for field in fields {
        let Some(answer) = &field.answer else {
            continue;
        };
        let veld = field.name.as_deref().unwrap_or_default();
        if veld.is_empty() {
            return Err(format!(
                "the UI spec: {name} shows an answer for a field without a `name`"
            ));
        }
        if field.fill.is_some() {
            return Err(format!(
                "the UI spec: {name} both fills '{veld}' and shows it as an answer; keep one"
            ));
        }
        if !regelrecht_law_model::is_fact_reference(&answer.from) {
            return Err(format!(
                "the UI spec: {name} shows '{veld}' from '{}', which is no \
                 <regelwerk>#<article>#<output>",
                answer.from
            ));
        }
        antwoorden.push(Antwoord {
            veld: veld.to_string(),
            bron: answer.from.clone(),
            besluit: answer.decision.clone(),
        });
    }
    if antwoorden.is_empty() {
        return Ok(None);
    }
    let gevraagd = fields
        .iter()
        .filter(|f| {
            f.answer.is_none() && f.fill.is_none() && f.visibility.as_deref() == Some("asked")
        })
        .filter_map(|f| f.name.clone())
        .collect();
    Ok(Some(Weergave {
        naam: titel.map_or_else(|| name.to_string(), str::to_string),
        antwoorden,
        gevraagd,
    }))
}

/// The prefill of the elements of the older form.
fn older_form(elementen: Vec<OldElement>) -> Result<Vec<ElementPrefill>, String> {
    let mut uit = Vec::new();
    for (i, element) in elementen.into_iter().enumerate() {
        let name = element_name(element.id.as_ref(), i);
        if element.fields.is_some() {
            return Err(format!(
                "the UI spec: {name} has `fields`, a key of the per-field form; \
                 put it under `elements`"
            ));
        }
        uit.push(ElementPrefill {
            name,
            sources: element.prefill.into_iter().collect(),
        });
    }
    Ok(uit)
}

/// The name of element `i` in a message: its id, whatever form that has.
fn element_name(id: Option<&Value>, i: usize) -> String {
    match id {
        Some(Value::String(id)) => id.clone(),
        Some(id) => serde_yaml_ng::to_string(id)
            .map(|t| t.trim().to_string())
            .unwrap_or_else(|_| format!("element {}", i + 1)),
        None => format!("element {}", i + 1),
    }
}

/// Every element's prefill as one map, checking each source and that no two
/// elements prefill a field from different sources.
fn merge(elements: Vec<ElementPrefill>) -> Result<BTreeMap<String, String>, String> {
    let mut uit: BTreeMap<String, (String, String)> = BTreeMap::new();
    for element in elements {
        let naam = element.name;
        for (veld, bron) in element.sources {
            if !regelrecht_law_model::is_fact_reference(&bron) {
                return Err(format!(
                    "the UI spec: {naam} prefills '{veld}' from '{bron}', which is no \
                     <regelwerk>#<article>#<output>"
                ));
            }
            match uit.get(&veld) {
                Some((eerder, door)) if *eerder != bron => {
                    return Err(format!(
                        "the UI spec: {door} prefills '{veld}' from '{eerder}', {naam} from \
                         '{bron}'"
                    ));
                }
                Some(_) => {}
                None => {
                    uit.insert(veld, (bron, naam.clone()));
                }
            }
        }
    }
    Ok(uit
        .into_iter()
        .map(|(veld, (bron, _))| (veld, bron))
        .collect())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_element_adds_its_fields_and_other_keys_are_left_alone() {
        let tekst = "
status: concept
elementen:
  - id: UI1
    regel: {beleid: [{artikel: '6'}]}
    prefill:
      naam: 'r#5h#naam'
      zetels: 'r#5g#zetels'
    nog_te_bouwen:
      - {naam: x, soort: parameter, artikel: 'r#6', reden: later}
  - id: UI5
    prefill:
      naam: 'r#5h#naam'
  - id: UI2
";
        let uit = prefill(tekst).unwrap();
        assert_eq!(
            uit,
            BTreeMap::from([
                ("naam".to_string(), "r#5h#naam".to_string()),
                ("zetels".to_string(), "r#5g#zetels".to_string()),
            ])
        );
    }

    #[test]
    fn the_per_field_form_reads_fill_from_and_leaves_other_keys_alone() {
        let tekst = "
status: concept
layer: example
elements:
  - id: UI1
    builds_on: G1
    basis: {policy: [{article: '6'}]}
    fields:
      - {name: jaar, visibility: asked, editable: true}
      - name: naam
        visibility: asked
        editable: true
        fill: {from: 'r#5h#naam'}
        basis: {law: [{regulation: w, article: '2'}]}
      - {name: kanaal, visibility: hidden, fill: {by: system}}
      - {name: bedrag, to_build: {kind: outcome, article: 'r#6', reason: later}}
    outcomes: [{name: volledig}]
  - id: UI5
    fields:
      - {name: naam, fill: {from: 'r#5h#naam'}}
      - {name: zetels, fill: {from: 'r#5g#zetels'}}
  - id: UI2
";
        let uit = prefill(tekst).unwrap();
        assert_eq!(
            uit,
            BTreeMap::from([
                ("naam".to_string(), "r#5h#naam".to_string()),
                ("zetels".to_string(), "r#5g#zetels".to_string()),
            ])
        );
    }

    #[test]
    fn both_forms_give_the_same_prefill() {
        let oud = "elementen: [{id: UI1, prefill: {naam: 'r#5h#naam'}}]";
        let nieuw = "elements: [{id: UI1, fields: [{name: naam, fill: {from: 'r#5h#naam'}}]}]";
        assert_eq!(prefill(oud).unwrap(), prefill(nieuw).unwrap());
    }

    #[test]
    fn an_element_with_a_number_for_its_id_is_named_by_it() {
        let e = prefill("elementen: [{id: 5, prefill: {naam: 'r#5h'}}]").unwrap_err();
        assert!(e.contains("5 prefills 'naam'"), "{e}");
        let e = prefill("elements: [{id: 5, fields: [{name: naam, fill: {from: 'r#5h'}}]}]")
            .unwrap_err();
        assert!(e.contains("5 prefills 'naam'"), "{e}");
    }

    #[test]
    fn a_spec_without_either_list_is_refused_not_read_as_empty() {
        let e = prefill("status: concept").unwrap_err();
        assert!(e.contains("no `elements`"), "{e}");
        // A misspelt list is no list.
        let e = prefill("element: [{id: UI1}]").unwrap_err();
        assert!(e.contains("no `elements`"), "{e}");
    }

    #[test]
    fn an_empty_list_prefills_nothing() {
        assert!(prefill("elements: []").unwrap().is_empty());
        assert!(prefill("elementen: []").unwrap().is_empty());
    }

    #[test]
    fn a_spec_that_mixes_the_forms_is_refused() {
        let e = prefill("elements: []\nelementen: []").unwrap_err();
        assert!(e.contains("both"), "{e}");
        let e = prefill("elements: [{id: UI1, prefill: {naam: 'r#5h#naam'}}]").unwrap_err();
        assert!(e.contains("UI1 has `prefill`"), "{e}");
        let e = prefill("elementen: [{id: UI1, fields: []}]").unwrap_err();
        assert!(e.contains("UI1 has `fields`"), "{e}");
    }

    #[test]
    fn a_fill_with_a_misspelt_or_no_key_is_refused() {
        let e = prefill("elements: [{id: UI1, fields: [{name: naam, fill: {form: 'r#5h#naam'}}]}]")
            .unwrap_err();
        assert!(e.contains("unknown field `form`"), "{e}");
        let e = prefill("elements: [{id: UI1, fields: [{name: naam, fill: {}}]}]").unwrap_err();
        assert!(e.contains("neither `from` nor `by`"), "{e}");
    }

    #[test]
    fn a_fill_both_from_a_source_and_by_itself_is_refused() {
        let e = prefill(
            "elements: [{id: UI1, fields: [{name: naam, fill: {from: 'r#5h#naam', by: system}}]}]",
        )
        .unwrap_err();
        assert!(e.contains("both `from` a source and `by` itself"), "{e}");
    }

    #[test]
    fn a_field_without_a_name_counts_only_when_it_is_prefilled() {
        let tekst = "elements: [{id: UI1, fields: [{to_build: {kind: outcome}}, {fill: {by: {kind: system}}}]}]";
        assert!(prefill(tekst).unwrap().is_empty());
        let e =
            prefill("elements: [{id: UI1, fields: [{fill: {from: 'r#5h#naam'}}]}]").unwrap_err();
        assert!(e.contains("UI1 fills a field without a `name`"), "{e}");
        let e = prefill("elements: [{id: UI1, fields: [{fill: {}}]}]").unwrap_err();
        assert!(e.contains("UI1 fills a field without a `name`"), "{e}");
    }

    #[test]
    fn two_sources_for_one_field_are_refused() {
        let tekst = "
elementen:
  - {id: UI1, prefill: {naam: 'r#5h#naam'}}
  - {id: UI5, prefill: {naam: 'r#5#naam'}}
";
        let e = prefill(tekst).unwrap_err();
        assert!(
            e.contains("UI1 prefills 'naam' from 'r#5h#naam', UI5 from 'r#5#naam'"),
            "{e}"
        );
        let tekst = "
elements:
  - {id: UI1, fields: [{name: naam, fill: {from: 'r#5h#naam'}}]}
  - {id: UI5, fields: [{name: naam, fill: {from: 'r#5#naam'}}]}
";
        let e = prefill(tekst).unwrap_err();
        assert!(
            e.contains("UI1 prefills 'naam' from 'r#5h#naam', UI5 from 'r#5#naam'"),
            "{e}"
        );
    }

    #[test]
    fn a_source_that_is_no_output_reference_is_refused() {
        let e = prefill("elementen: [{prefill: {naam: 'r#5h'}}]").unwrap_err();
        assert!(e.contains("element 1 prefills 'naam' from 'r#5h'"), "{e}");
        assert!(prefill("elementen: 3").is_err());
    }

    #[test]
    fn an_element_with_answers_is_a_view_beside_a_decision() {
        let tekst = "
elements:
  - id: UI1
    fields: [{name: naam, fill: {from: 'r#5h#naam'}}]
  - id: UI8
    element: stand van de betaling
    role: behandelaar
    fields:
      - {name: rente, visibility: asked}
      - {name: saldo, visibility: shown, answer: {from: 'b#2#saldo', decision: besluit}}
      - {name: kanaal, visibility: asked, fill: {by: system}}
      - {name: geen, visibility: hidden}
";
        let spec = lees(tekst).unwrap();
        assert_eq!(spec.prefill.len(), 1);
        assert_eq!(
            spec.weergaven,
            vec![Weergave {
                naam: "stand van de betaling".into(),
                antwoorden: vec![Antwoord {
                    veld: "saldo".into(),
                    bron: "b#2#saldo".into(),
                    besluit: "besluit".into(),
                }],
                gevraagd: vec!["rente".into()],
            }]
        );
        // Without a name of its own the view goes by its id.
        let spec = lees(
            "elements: [{id: UI9, fields: [{name: s, answer: {from: 'b#2#s', decision: d}}]}]",
        )
        .unwrap();
        assert_eq!(spec.weergaven[0].naam, "UI9");
    }

    #[test]
    fn an_answer_that_does_not_read_is_refused() {
        let e = lees(
            "elements: [{id: UI8, fields: [{name: s, answer: {form: 'b#2#s', decision: d}}]}]",
        )
        .unwrap_err();
        assert!(e.contains("unknown field `form`"), "{e}");
        let e =
            lees("elements: [{id: UI8, fields: [{name: s, answer: {from: 'b#2', decision: d}}]}]")
                .unwrap_err();
        assert!(e.contains("UI8 shows 's' from 'b#2'"), "{e}");
        let e = lees(
            "elements: [{id: UI8, fields: [{name: s, fill: {by: system}, answer: {from: 'b#2#s', decision: d}}]}]",
        )
        .unwrap_err();
        assert!(e.contains("both fills 's'"), "{e}");
        let e = lees("elements: [{id: UI8, fields: [{answer: {from: 'b#2#s', decision: d}}]}]")
            .unwrap_err();
        assert!(e.contains("without a `name`"), "{e}");
    }
}
