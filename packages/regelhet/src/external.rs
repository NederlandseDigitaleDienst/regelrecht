//! What the admin panel shows of an external party of the case: a cell the
//! process needs, such as the GGD that advises or the court that decides on
//! an appeal, for which nobody works in this server. Until a real link with
//! such a party exists, an admin enters what it did on its behalf
//! ([`crate::api`], the routes under `/api/backoffice/beheer/`).
//!
//! The panel shows of a party only what that input needs, never its whole
//! chronicle: the submissions that wait for its decision (the kern's
//! [`Stand::open`]) and its decisions of which no copy reached another cell
//! yet ([`unsent`]). Which facts it can record, the forms say
//! (`andere_cellen`). Which record is a submission and which a decision comes
//! from the kern ([`regelrecht_poc_chronolexografie::stand`]); no step of a
//! particular law is named here.

use regelrecht_cel::{EXECUTOGRAM, Gram};
use regelrecht_poc_chronolexografie::casus::Afleiding;
use regelrecht_poc_chronolexografie::stand::{Rol, Stand};

/// The decisions in the chronicle `grams` taken on a submission, of which
/// no receipt in `elsewhere` (the other cells' records) holds a copy yet:
/// those the party can still send to the cell they concern. A receipt is a
/// recorded fact that received the decision from this party; a submission
/// elsewhere that contests the decision also carries what it received, and
/// is no copy sent. Only decisions under an article whose copy the law
/// requires a receipt of are offered ([`Afleiding::ontvangst_van`]), the
/// rule by which `kern::toezenden` accepts or refuses a copy.
pub fn unsent<'g>(
    grams: &'g [Gram],
    elsewhere: &[&Gram],
    stand: &Stand,
    afleiding: &Afleiding,
) -> Vec<&'g Gram> {
    grams
        .iter()
        .filter(|d| stand.is(&d.id, Rol::Besluit) && d.refers_to.is_some())
        .filter(|d| afleiding.ontvangst_van(&d.basis).is_some())
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
    use regelrecht_poc_chronolexografie::casus::Indiening;
    use serde_json::json;

    use super::*;

    fn gram(id: &str, kind: &str, basis: &str, refers_to: Option<&str>) -> Gram {
        let mut g = json!({
            "id": id,
            "kind": kind,
            "basis": basis,
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
        let mut g = gram(id, EXECUTOGRAM, "awb#8:79", None);
        g.received = Some(regelrecht_cel::Received {
            from: "rechtbank".into(),
            id: of.into(),
        });
        g
    }

    fn indiening(artikel: &str, soort: &str, besluit: &str, ontvangst: Option<&str>) -> Indiening {
        Indiening {
            artikel: artikel.into(),
            soort: soort.into(),
            besluit: besluit.into(),
            cel: "rechtbank".into(),
            ontvangst: ontvangst.map(str::to_string),
        }
    }

    /// Two articles establish an appeal, each asking a decision of its own;
    /// only the first one's copy the law requires a receipt of.
    fn afleiding() -> Afleiding {
        Afleiding {
            law: "wet".into(),
            article: "1".into(),
            decision_law: "wet".into(),
            decision: "1".into(),
            indieningen: vec![
                indiening("awb#8:1", "BEROEP", "awb#8:70", Some("awb#8:79#afschrift")),
                indiening("awb#8:2", "BEROEP", "awb#8:71", None),
            ],
            ambtshalve: Vec::new(),
            rol: "de rechtbank".into(),
            regelwerken: Vec::new(),
        }
    }

    #[test]
    fn only_a_decision_whose_copy_the_law_requires_is_unsent_until_received() {
        let grams = [
            gram("b1", "BEROEP", "awb#8:1", None),
            gram("u1", "UITSPRAAK", "awb#8:70", Some("b1")),
            gram("b2", "BEROEP", "awb#8:1", None),
            gram("u2", "UITSPRAAK", "awb#8:70", Some("b2")),
            gram("b3", "BEROEP", "awb#8:2", None),
            gram("u3", "UITSPRAAK", "awb#8:71", Some("b3")),
        ];
        let mut stand = Stand::default();
        for g in &grams {
            let rol = if g.kind == "BEROEP" {
                Rol::Indiening
            } else {
                Rol::Besluit
            };
            stand.rollen.insert(g.id.clone(), rol);
        }
        let received = receipt("r1", "u1");
        // A submission elsewhere that carries u2 is no copy of it.
        let mut contesting = gram("h1", "BEROEP", "awb#8:1", None);
        contesting.received = receipt("x", "u2").received;
        let unsent: Vec<&str> = unsent(&grams, &[&received, &contesting], &stand, &afleiding())
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        // u1 was received; u3 is of the same kind as u2, but under an
        // article whose copy no law requires a receipt of: the kern would
        // refuse to send it.
        assert_eq!(unsent, ["u2"]);
    }
}
