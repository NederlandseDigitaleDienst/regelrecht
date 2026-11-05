//! Which decisions in a chronicle still have consequences, from the effects
//! the law gives a decision on another one (`produces[].effects`, schema
//! v0.8.14).
//!
//! The law says what a decision does with another decision: it revokes it,
//! as the decision on an objection revokes the contested decision (Awb 7:11
//! lid 2), or it replaces it, as a new decision taken in the place of the
//! revoked one. The cell plays those effects in the order the decisions were
//! recorded:
//!
//! - every decision has consequences, unless a decision that itself has
//!   consequences revokes or replaces it;
//! - a replacing decision has consequences of its own, in the place of the
//!   decision it replaces;
//! - when a revoking decision loses its consequences, such as a withdrawal
//!   that is itself revoked on objection, the decision it revoked has
//!   consequences again.
//!
//! A reduction is then given only the decisions that count, and decides from
//! them what it answers. It holds no rule of the Awb.
//!
//! This is separate from the replacement [`crate::Cell`] already knows, where
//! the decision on an objection holds the replacing decision by its id as an
//! output. That one does not read `effects`, and this module does not read
//! it; the two are not reconciled yet.
//!
//! Experimental: dates play no part yet. Whether a decision counts says
//! nothing about from when or until when (ex nunc, ex tunc, a decision that
//! takes effect later).

use regelrecht_engine::LawExecutionService;
use regelrecht_law_model::DecisionEffect;

use crate::chronicle::{Chronicle, Gram};
use crate::error::{refused, Result};
use crate::shape::article_on;

/// The decisions in `chronicle` whose consequences count, in the order they
/// were recorded.
///
/// A chronolexogram is a decision when its article takes a decision of its
/// kind (`produces`); submissions and executograms are not. An effect names
/// a field of the decision whose value is the id of the other decision,
/// which must be a decision recorded before it.
pub fn counting<'c>(
    service: &LawExecutionService,
    chronicle: &'c Chronicle,
) -> Result<Vec<&'c Gram>> {
    let decisions: Vec<(&Gram, &[DecisionEffect])> = chronicle
        .grams()
        .iter()
        .filter_map(|gram| decision_effects(service, gram).map(|effects| (gram, effects)))
        .collect();

    // undone_by[i]: the decisions with an effect on decision i. Each was
    // recorded after i, which the check below makes sure of.
    let mut undone_by: Vec<Vec<usize>> = vec![Vec::new(); decisions.len()];
    for (later, (gram, effects)) in decisions.iter().enumerate() {
        for effect in *effects {
            let name = effect.parameter();
            let target = gram.fields.get(name).and_then(|v| v.as_str()).ok_or_else(|| {
                refused(format!(
                    "decision '{}' ({}) has an effect on '{name}', but holds no decision id under it",
                    gram.id, gram.basis
                ))
            })?;
            let earlier = decisions[..later]
                .iter()
                .position(|(g, _)| g.id == target)
                .ok_or_else(|| {
                    refused(format!(
                        "decision '{}' ({}) has an effect on '{target}', which is no decision \
                         recorded before it",
                        gram.id, gram.basis
                    ))
                })?;
            undone_by[earlier].push(later);
        }
    }

    // From the last decision back: whether a decision counts depends only on
    // decisions recorded after it, which are settled by then.
    let mut counts = vec![true; decisions.len()];
    for i in (0..decisions.len()).rev() {
        counts[i] = !undone_by[i].iter().any(|&j| counts[j]);
    }

    Ok(decisions
        .into_iter()
        .zip(counts)
        .filter_map(|((gram, _), counts)| counts.then_some(gram))
        .collect())
}

/// The effects of `gram` when it is a decision: its article, in the law of
/// the day it was recorded, takes a decision of the kind `gram` has. `None`
/// for anything else in the chronicle.
fn decision_effects<'s>(
    service: &'s LawExecutionService,
    gram: &Gram,
) -> Option<&'s [DecisionEffect]> {
    let (law_id, number) = gram.basis.split_once('#')?;
    let decision = article_on(service, law_id, number, gram.recorded_on)?.get_decision()?;
    (decision.legal_character() == Some(gram.kind.as_str())).then(|| decision.effects())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::NaiveDate;
    use serde_json::{json, Value};

    use super::*;

    /// The synthetic law of the cell's BDD suite: article 1 grants, 2 revokes
    /// `bestreden_besluit`, 3 replaces it, 4 revokes `ingetrokken_besluit`.
    const LAW: &str =
        include_str!("../bdd/fixtures/regulation/test_chronicle_effects/2025-01-01.yaml");

    fn service() -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service.load_law(LAW).unwrap();
        service
    }

    /// A decision `id` under `article`, with `fields`.
    fn decision(id: &str, article: &str, fields: Value) -> Gram {
        Gram {
            id: id.to_string(),
            kind: "BESCHIKKING".to_string(),
            basis: format!("test_chronicle_effects#{article}"),
            fact: None,
            refers_to: None,
            received: None,
            after: None,
            recording_actor: "test".to_string(),
            recorded_on: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
            moment: None,
            fields: serde_json::from_value::<BTreeMap<String, Value>>(fields).unwrap(),
            missing: Vec::new(),
            outputs: BTreeMap::new(),
        }
    }

    fn counting_ids(grams: Vec<Gram>) -> Result<Vec<String>> {
        let chronicle = Chronicle::in_memory_with(grams).unwrap();
        Ok(counting(&service(), &chronicle)?
            .into_iter()
            .map(|g| g.id.clone())
            .collect())
    }

    #[test]
    fn a_revoked_decision_does_not_count() {
        let ids = counting_ids(vec![
            decision("a", "1", json!({"ernstig_beperkt": true})),
            decision("b", "2", json!({"bestreden_besluit": "a"})),
        ])
        .unwrap();
        assert_eq!(ids, ["b"]);
    }

    #[test]
    fn revoking_the_revocation_restores_the_decision() {
        let ids = counting_ids(vec![
            decision("a", "1", json!({"ernstig_beperkt": true})),
            decision("b", "4", json!({"ingetrokken_besluit": "a"})),
            decision("c", "2", json!({"bestreden_besluit": "b"})),
        ])
        .unwrap();
        assert_eq!(ids, ["a", "c"]);
    }

    #[test]
    fn a_replacing_decision_counts_in_the_place_of_the_other() {
        let ids = counting_ids(vec![
            decision("a", "1", json!({"ernstig_beperkt": false})),
            decision(
                "b",
                "3",
                json!({"bestreden_besluit": "a", "ernstig_beperkt": true}),
            ),
        ])
        .unwrap();
        assert_eq!(ids, ["b"]);
    }

    #[test]
    fn revoking_the_replacement_restores_the_replaced_decision() {
        let ids = counting_ids(vec![
            decision("a", "1", json!({"ernstig_beperkt": true})),
            decision(
                "b",
                "3",
                json!({"bestreden_besluit": "a", "ernstig_beperkt": false}),
            ),
            decision("c", "2", json!({"bestreden_besluit": "b"})),
        ])
        .unwrap();
        assert_eq!(ids, ["a", "c"]);
    }

    #[test]
    fn an_effect_needs_an_earlier_decision() {
        for (grams, why) in [
            (
                vec![decision("b", "2", json!({"bestreden_besluit": "x"}))],
                "which is no decision recorded before it",
            ),
            (
                vec![
                    decision("b", "2", json!({"bestreden_besluit": "a"})),
                    decision("a", "1", json!({"ernstig_beperkt": true})),
                ],
                "which is no decision recorded before it",
            ),
            (
                vec![decision("b", "2", json!({}))],
                "holds no decision id under it",
            ),
        ] {
            let err = counting_ids(grams).expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
    }

    #[test]
    fn a_record_that_is_no_decision_of_its_article_is_left_out() {
        let mut submission = decision("s", "1", json!({"ernstig_beperkt": true}));
        submission.kind = "AANVRAAG".to_string();
        let ids = counting_ids(vec![
            submission,
            decision("a", "1", json!({"ernstig_beperkt": true})),
        ])
        .unwrap();
        assert_eq!(ids, ["a"]);
    }
}
