//! The timeline of one application: every record that happened about it, in
//! the order it happened, and what can or must happen next. One derivation
//! for both sides; the citizen and the employee each get it kept to what
//! they may see ([`Timeline::within`]).
//!
//! Which records belong to the application, and their order, the kern says
//! ([`tijdlijn`]). What comes next is read from what the kern already knows
//! about those records, never from a step of a particular law:
//!
//! - a submission that waits for a decision ([`Stand::open`]): the cell that
//!   decides on it is to act, the executor or an external party;
//! - a request of the executor's policy that the other party has not
//!   answered ([`Verzoek::open`]);
//! - the next act after a grant ([`vervolg::Stand::volgende`]), the system's
//!   when the case says the system takes it (`door_het_systeem`), else a
//!   person's of the executor;
//! - a decision of an external party not sent to the cell it concerns
//!   ([`external::unsent`]);
//! - a decision that still counts and gives the field a submission form
//!   reads as its deadline ([`kern::termijnvelden`]), with that day not
//!   passed and no such submission against it yet: whoever may submit
//!   against it can do so until then.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_cel::Gram;
use regelrecht_poc_chronolexografie::casus::Afleiding;
use regelrecht_poc_chronolexografie::stand::{Rol, Stand};
use regelrecht_poc_chronolexografie::tijdlijn;
use regelrecht_poc_chronolexografie::vervolg;
use regelrecht_poc_chronolexografie::verzoek::Verzoek;
use serde::Serialize;
use serde_json::Value;

use crate::external;

/// Who is to act next. On the wire in Dutch, as the rest of the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Actor {
    /// A person of the executor.
    #[serde(rename = "uitvoerder")]
    Executor,
    /// The system, by an article of the executor's policy.
    #[serde(rename = "systeem")]
    System,
    /// An external party of the case; in this server an admin enters what it
    /// does, in the admin panel.
    #[serde(rename = "externe_partij")]
    External,
    /// Whoever may make a submission against a decision.
    #[serde(rename = "indiener")]
    Submitter,
}

/// One thing that can or must happen next.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum Next {
    /// The cell `cel` decides on the submission `op`; with `na` again, after
    /// that record.
    Besluit {
        wie: Actor,
        cel: String,
        op: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        na: Option<String>,
    },
    /// The cell `cel` answers the request `op` with the fact `feit`; `taak`
    /// names that as a step to take, where the case does.
    Antwoord {
        wie: Actor,
        cel: String,
        op: String,
        feit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        taak: Option<String>,
    },
    /// The executor records the act `feit` after the grant `op`.
    Handeling {
        wie: Actor,
        cel: String,
        op: String,
        feit: String,
        /// The article by which the system takes it, for the system's act.
        #[serde(skip_serializing_if = "Option::is_none")]
        artikel: Option<String>,
        /// The act as a step to take, where the case names it.
        #[serde(skip_serializing_if = "Option::is_none")]
        taak: Option<String>,
    },
    /// The cell `cel` sends a copy of its decision `op`.
    Toezending { wie: Actor, cel: String, op: String },
    /// A submission against the decision `op` can be made until `tot`, the
    /// value of its output `veld`.
    Termijn {
        wie: Actor,
        op: String,
        veld: String,
        tot: NaiveDate,
    },
}

impl Next {
    /// The ids of the records it names.
    fn ids(&self) -> impl Iterator<Item = &str> {
        let (op, na) = match self {
            Self::Besluit { op, na, .. } => (op, na.as_deref()),
            Self::Antwoord { op, .. }
            | Self::Handeling { op, .. }
            | Self::Toezending { op, .. }
            | Self::Termijn { op, .. } => (op, None),
        };
        std::iter::once(op.as_str()).chain(na)
    }
}

/// The timeline of one application.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Timeline {
    /// The application at the top; left out for a reader who may not learn
    /// its id ([`Timeline::within`]).
    #[serde(rename = "stuk", skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The records that belong to it, in the order they happened.
    #[serde(rename = "grammen")]
    pub grams: Vec<Gram>,
    /// What the kern says of those records: what each is, which are revoked
    /// and what answers each submission.
    pub stand: Stand,
    /// Per record the system made by an article of the policy, that article.
    pub door_het_systeem: BTreeMap<String, String>,
    /// What can or must happen next.
    #[serde(rename = "volgende")]
    pub next: Vec<Next>,
}

impl Timeline {
    /// What of it a reader gets who may see the records for which `shown`
    /// holds, and learn the ids for which `named` holds: those records, the
    /// entries of the standing that name no other id ([`Stand::binnen`]),
    /// and the next steps that name no other id. A citizen sees and names
    /// the records of their dossier; an employee sees the executor's own
    /// records and may learn the id of what reached it from another cell,
    /// as for why a decision came out so. No other id reaches the reader.
    pub fn within(self, shown: impl Fn(&str) -> bool, named: impl Fn(&str) -> bool) -> Self {
        Self {
            root: self.root.filter(|r| named(r)),
            grams: self.grams.into_iter().filter(|g| shown(&g.id)).collect(),
            stand: self.stand.binnen(&named),
            door_het_systeem: self
                .door_het_systeem
                .into_iter()
                .filter(|(id, _)| shown(id))
                .collect(),
            next: self
                .next
                .into_iter()
                .filter(|n| n.ids().all(&named))
                .collect(),
        }
    }
}

/// What the timeline is derived from: every chronicle of the case as it is
/// now, with what the kern says of each.
pub struct Sources<'a> {
    /// Per cell of the case, in its order: its id, its records and its
    /// standing. The executor's first.
    pub cells: &'a [(String, Vec<Gram>, Stand)],
    pub executor: &'a str,
    pub requests: &'a [Verzoek],
    pub grants: &'a [vervolg::Stand],
    /// What follows a grant in the case, if it says.
    pub after_grant: Option<&'a vervolg::NaToekenning>,
    pub afleiding: &'a Afleiding,
    /// Per submission article, the field that holds its deadline.
    pub deadlines: &'a BTreeMap<String, String>,
    pub by_policy: &'a BTreeMap<String, String>,
    pub today: NaiveDate,
}

/// The timeline of the application the record `id` belongs to; `None` when
/// no cell holds `id`.
pub fn derive(src: &Sources<'_>, id: &str) -> Option<Timeline> {
    let onder = tijdlijn::onder_verzoeken(src.requests);
    let all: Vec<&Gram> = src.cells.iter().flat_map(|(_, g, _)| g).collect();
    let root = tijdlijn::wortel(&all, &onder, id)?;
    let chronicles: Vec<&[Gram]> = src.cells.iter().map(|(_, g, _)| g.as_slice()).collect();
    let grams: Vec<Gram> = tijdlijn::stukken(&chronicles, &onder, &root.id)
        .into_iter()
        .cloned()
        .collect();
    let ids: BTreeSet<&str> = grams.iter().map(|g| g.id.as_str()).collect();
    let within = |x: &str| ids.contains(x);

    let mut stand = Stand::default();
    for (_, _, s) in src.cells {
        let s = s.clone().binnen(within);
        stand.rollen.extend(s.rollen);
        stand.herroepen.extend(s.herroepen);
        stand.antwoorden.extend(s.antwoorden);
    }
    let mut next = Vec::new();
    decisions(src, &within, &mut next);
    answers(src, &within, &mut next);
    acts(src, &within, &mut next);
    copies(src, &within, &mut next);
    deadlines(src, &grams, &stand, &mut next);
    let door_het_systeem = src
        .by_policy
        .iter()
        .filter(|(id, _)| within(id))
        .map(|(id, artikel)| (id.clone(), artikel.clone()))
        .collect();
    Some(Timeline {
        root: Some(root.id.clone()),
        grams,
        stand,
        door_het_systeem,
        next,
    })
}

/// The submissions that wait for a decision of the cell that holds them.
fn decisions(src: &Sources<'_>, within: &dyn Fn(&str) -> bool, next: &mut Vec<Next>) {
    for (cel, grams, stand) in src.cells {
        for open in stand.open(grams).into_iter().filter(|o| within(&o.stuk.id)) {
            next.push(Next::Besluit {
                wie: actor_of(src, cel),
                cel: cel.clone(),
                op: open.stuk.id.clone(),
                na: open.na.map(str::to_string),
            });
        }
    }
}

/// The requests of the executor's policy the other party has not answered.
fn answers(src: &Sources<'_>, within: &dyn Fn(&str) -> bool, next: &mut Vec<Next>) {
    for r in src
        .requests
        .iter()
        .filter(|r| r.open() && within(&r.verzoek.id))
    {
        if let Some(feit) = &r.antwoordfeit {
            next.push(Next::Antwoord {
                wie: actor_of(src, &r.aan),
                cel: r.aan.clone(),
                op: r.verzoek.id.clone(),
                feit: feit.clone(),
                taak: r.antwoordtaak.clone(),
            });
        }
    }
}

/// The next act after each grant.
fn acts(src: &Sources<'_>, within: &dyn Fn(&str) -> bool, next: &mut Vec<Next>) {
    let Some(na) = src.after_grant else { return };
    for grant in src.grants.iter().filter(|g| within(&g.toekenning)) {
        let Some(feit) = &grant.volgende else {
            continue;
        };
        let handeling = na.handelingen.iter().find(|h| &h.feit == feit);
        let artikel = handeling
            .and_then(|h| h.door_het_systeem.as_ref())
            .map(|o| o.artikel.clone());
        next.push(Next::Handeling {
            wie: if artikel.is_some() {
                Actor::System
            } else {
                Actor::Executor
            },
            cel: src.executor.to_string(),
            op: grant.toekenning.clone(),
            feit: feit.clone(),
            artikel,
            taak: handeling.and_then(|h| h.taak.clone()),
        });
    }
}

/// The decisions of external parties not sent yet.
fn copies(src: &Sources<'_>, within: &dyn Fn(&str) -> bool, next: &mut Vec<Next>) {
    for (cel, grams, stand) in src.cells.iter().filter(|(c, _, _)| c != src.executor) {
        let elsewhere: Vec<&Gram> = src
            .cells
            .iter()
            .filter(|(other, _, _)| other != cel)
            .flat_map(|(_, g, _)| g)
            .collect();
        for d in external::unsent(grams, &elsewhere, stand, src.afleiding) {
            if within(&d.id) {
                next.push(Next::Toezending {
                    wie: Actor::External,
                    cel: cel.clone(),
                    op: d.id.clone(),
                });
            }
        }
    }
}

/// The deadlines that run: per decision in `grams` that still counts, each
/// output that a submission form reads as its deadline, with that day not
/// passed and no submission with that deadline made against it yet.
fn deadlines(src: &Sources<'_>, grams: &[Gram], stand: &Stand, next: &mut Vec<Next>) {
    let fields: BTreeSet<&String> = src.deadlines.values().collect();
    for d in grams
        .iter()
        .filter(|g| stand.is(&g.id, Rol::Besluit) && !stand.herroepen.contains_key(&g.id))
    {
        for veld in &fields {
            let Some(tot) = d
                .outputs
                .get(*veld)
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<NaiveDate>().ok())
            else {
                continue;
            };
            let against = grams.iter().any(|s| {
                stand.is(&s.id, Rol::Indiening)
                    && src.deadlines.get(&s.basis) == Some(*veld)
                    && (s.refers_to.as_deref() == Some(d.id.as_str())
                        || s.received.as_ref().is_some_and(|r| r.id == d.id))
            });
            if tot >= src.today && !against {
                next.push(Next::Termijn {
                    wie: Actor::Submitter,
                    op: d.id.clone(),
                    veld: (*veld).clone(),
                    tot,
                });
            }
        }
    }
}

/// Who acts for the cell `cel`: a person of the executor, or an external
/// party.
fn actor_of(src: &Sources<'_>, cel: &str) -> Actor {
    if cel == src.executor {
        Actor::Executor
    } else {
        Actor::External
    }
}
