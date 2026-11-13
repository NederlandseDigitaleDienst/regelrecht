//! What the poc reads from the UI policy of a source (`ui_beleid.yaml` at the
//! root of the source, the file its manifest names): per field of the
//! application, the source the form prefills it from (`vult_voor`).
//!
//! A UI policy is a register of elements (`elementen`), each with what a
//! screen shows, asks and fills, and on what grounds. The poc reads only
//! `vult_voor` of each element and leaves every other key to the register
//! and its own checks, so a register that grows a key does not stop a case
//! from loading. A source is `<regelwerk>#<article>#<output>`; what it gives
//! the cell computes when the form opens ([`crate::kern::voorinvullen`]).

use std::collections::BTreeMap;

use serde::Deserialize;

/// The part of a UI policy the poc reads.
#[derive(Debug, Deserialize)]
struct Register {
    #[serde(default)]
    elementen: Vec<Element>,
}

/// The part of an element the poc reads. The id names the element in a
/// message; the register decides its form, so any value will do.
#[derive(Debug, Deserialize)]
struct Element {
    id: Option<serde_yaml_ng::Value>,
    #[serde(default)]
    vult_voor: BTreeMap<String, String>,
}

/// Per field the source the form prefills it from, over every element of the
/// UI policy `tekst`. Two elements may name the same source for a field, as
/// the form and the sheet that edits a field do.
///
/// # Errors
///
/// The text when it is no UI policy, a source that is no
/// `<regelwerk>#<article>#<output>`, and a field two elements prefill from
/// different sources.
pub fn vult_voor(tekst: &str) -> Result<BTreeMap<String, String>, String> {
    let register: Register =
        serde_yaml_ng::from_str(tekst).map_err(|e| format!("the UI policy: {e}"))?;
    let mut uit: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (i, element) in register.elementen.iter().enumerate() {
        let naam = match &element.id {
            Some(serde_yaml_ng::Value::String(id)) => id.clone(),
            Some(id) => serde_yaml_ng::to_string(id)
                .map(|t| t.trim().to_string())
                .unwrap_or_else(|_| format!("element {}", i + 1)),
            None => format!("element {}", i + 1),
        };
        for (veld, bron) in &element.vult_voor {
            if !regelrecht_law_model::is_fact_reference(bron) {
                return Err(format!(
                    "the UI policy: {naam} prefills '{veld}' from '{bron}', which is no \
                     <regelwerk>#<article>#<output>"
                ));
            }
            match uit.get(veld) {
                Some((eerder, door)) if eerder != bron => {
                    return Err(format!(
                        "the UI policy: {door} prefills '{veld}' from '{eerder}', {naam} from \
                         '{bron}'"
                    ));
                }
                Some(_) => {}
                None => {
                    uit.insert(veld.clone(), (bron.clone(), naam.clone()));
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
    vult_voor:
      naam: 'r#5h#naam'
      zetels: 'r#5g#zetels'
    nog_te_bouwen:
      - {naam: x, soort: parameter, artikel: 'r#6', reden: later}
  - id: UI5
    vult_voor:
      naam: 'r#5h#naam'
  - id: UI2
";
        let uit = vult_voor(tekst).unwrap();
        assert_eq!(
            uit,
            BTreeMap::from([
                ("naam".to_string(), "r#5h#naam".to_string()),
                ("zetels".to_string(), "r#5g#zetels".to_string()),
            ])
        );
    }

    #[test]
    fn an_element_with_a_number_for_its_id_is_named_by_it() {
        let e = vult_voor("elementen: [{id: 5, vult_voor: {naam: 'r#5h'}}]").unwrap_err();
        assert!(e.contains("5 prefills 'naam'"), "{e}");
    }

    #[test]
    fn a_policy_without_elements_prefills_nothing() {
        assert!(vult_voor("status: concept").unwrap().is_empty());
    }

    #[test]
    fn two_sources_for_one_field_are_refused() {
        let tekst = "
elementen:
  - {id: UI1, vult_voor: {naam: 'r#5h#naam'}}
  - {id: UI5, vult_voor: {naam: 'r#5#naam'}}
";
        let e = vult_voor(tekst).unwrap_err();
        assert!(
            e.contains("UI1 prefills 'naam' from 'r#5h#naam', UI5 from 'r#5#naam'"),
            "{e}"
        );
    }

    #[test]
    fn a_source_that_is_no_output_reference_is_refused() {
        let e = vult_voor("elementen: [{vult_voor: {naam: 'r#5h'}}]").unwrap_err();
        assert!(e.contains("element 1 prefills 'naam' from 'r#5h'"), "{e}");
        assert!(vult_voor("elementen: 3").is_err());
    }
}
