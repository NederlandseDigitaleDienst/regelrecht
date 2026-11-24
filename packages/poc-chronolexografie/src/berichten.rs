//! The message spec of a source (`berichten.yaml` at the root of the
//! source, the file its manifest names under `berichten`), with the text
//! layer of the authority (`berichtteksten.yaml`, under `berichtteksten`):
//! which message to the citizen follows from which record of the chronicle,
//! and how it is made from that record.
//!
//! A message spec holds `berichtsoorten`. Each binds a kind of message to a
//! record (`vastlegging`: a decision, a submission or a fact, of an article,
//! with its legal character and the outcome that carries it), names a
//! pattern of the generic Awb layer (`patroon`, one of [`PATRONEN`]) whose
//! parts the law requires, and lays out its parts (`delen`): per part its
//! place in the message (`plek`, one of [`PLEKKEN`], which also gives its
//! order), the article that requires it (`vereist_door`), where its content
//! comes from (`bron`), when it stands (`als`) and the element of the design
//! system that shows it (`nldd`, one of [`NLDD`]). The text itself is not in
//! the spec: it comes from the text layer, a register of the authority with
//! a template (`sjabloon`) per message, whose paragraphs carry keys
//! (`{bedrag}`) and conditions (`als: toekenning`). The spec says per key
//! where its value comes from (`sleutels`), so no text stands loose from the
//! record. Invariants (`invarianten`) say what must hold between the parts,
//! such as the amount in the decision being the total of its specification.
//!
//! This module reads both files and refuses what would lose a part without a
//! word: a part in no place, a pattern whose required part is absent, an
//! element the screen does not know, a key or condition of the template
//! without a source, a paragraph of the template that no part takes, an
//! invariant naming a key or part that is not there. What it cannot know
//! without a record, such as an empty value, the screen checks when it
//! makes the message (`gedeeld/bericht.ts`), and shows as a violation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The places of a message, in the order a message has them: the heading
/// with the authority, the addressee, the day and the reference (`kop`); the
/// opening (`inleiding`); what was decided or asked (`dictum`); under which
/// provision (`grondslag`, Awb 3:47 second paragraph); on which grounds
/// (`motivering`, 3:46 and 3:47 first paragraph, with `onderbouwing` for
/// what an executor adds); the calculation or the parts of the decision
/// (`specificatie`); what else the authority tells, such as payment or the
/// channel (`mededeling`); who signs and on whose behalf (`ondertekening`,
/// 10:10); and the legal remedy (`rechtsmiddelen`, 3:45).
pub const PLEKKEN: [&str; 11] = [
    "kop",
    "inleiding",
    "dictum",
    "grondslag",
    "motivering",
    "onderbouwing",
    "specificatie",
    "mededeling",
    "ondertekening",
    "rechtsmiddelen",
    "bijlage",
];

/// The elements of the design system a part may be shown with, as the
/// screen knows them (`gedeeld/Bericht.vue`): running text, a box (for the
/// decision and the remedy), a table (for a specification), a list (of
/// fields and values, or of articles), and the identity of the sender.
pub const NLDD: [&str; 6] = [
    "nldd-rich-text",
    "nldd-box",
    "nldd-table",
    "nldd-list",
    "nldd-identity",
    "nldd-text",
];

/// What the law gives a part that the text layer leaves open: the provision
/// from the article the decision rests on, the articles of its chain, the
/// remedy from the submissions the law allows against it, the signature with
/// the mandate.
pub const WET: [&str; 4] = ["grondslag", "keten", "rechtsmiddelen", "ondertekening"];

/// The channels a message goes out by, with the articles that govern them:
/// by e-mail after a declaration of reachability (Awb 2:8), placed in a
/// portal with a notice (2:8 with 2:10), or on paper (3:41).
pub const KANALEN: [(&str, &[&str]); 3] = [
    ("email", &["Awb 2:8"]),
    ("portaal", &["Awb 2:8", "Awb 2:10"]),
    ("papier", &["Awb 3:41"]),
];

/// The patterns of the generic Awb layer a kind of message follows, with
/// per pattern the places the law requires and the article that requires
/// each. A kind of message of a pattern has a part in each of them, with
/// `vereist_door`; what it leaves empty, the screen shows as missing.
pub const PATRONEN: [(&str, &[(&str, &str)]); 6] = [
    (
        "beschikking",
        &[
            ("dictum", "Awb 1:3"),
            ("grondslag", "Awb 3:47"),
            ("motivering", "Awb 3:46"),
            ("ondertekening", "Awb 10:10"),
            ("rechtsmiddelen", "Awb 3:45"),
        ],
    ),
    (
        "beslissing_op_bezwaar",
        &[
            ("dictum", "Awb 7:11"),
            ("motivering", "Awb 7:12"),
            ("ondertekening", "Awb 10:10"),
            ("rechtsmiddelen", "Awb 3:45"),
        ],
    ),
    (
        "buitenbehandelingstelling",
        &[
            ("dictum", "Awb 4:5"),
            ("motivering", "Awb 3:46"),
            ("ondertekening", "Awb 10:10"),
            ("rechtsmiddelen", "Awb 3:45"),
        ],
    ),
    ("ontvangstbevestiging", &[("dictum", "Awb 2:17")]),
    (
        "herstelverzoek",
        &[("dictum", "Awb 4:5"), ("specificatie", "Awb 4:5")],
    ),
    ("mededeling", &[]),
];

/// The message spec with its text layer, as the forms carry it
/// (`berichten` in `GET /api/formulier`). The spec is passed on as written,
/// so a screen reads the keys this module checked; what this module found
/// that a screen should show, stands under `meldingen`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Berichtenspec {
    /// How a message goes out: the fact of sending and of forwarding, as
    /// written (`verzending`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verzending: Option<Value>,
    pub berichtsoorten: Vec<Value>,
    /// The text layer: `afzender` and `sjablonen`, the rest as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tekstlaag: Option<Value>,
    /// What does not hold in the spec but costs no message, such as a
    /// paragraph of a template no part takes: the screen shows it beside
    /// each message of that kind.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meldingen: Vec<String>,
}

impl Berichtenspec {
    /// Whether the source has a message spec.
    pub fn is_empty(&self) -> bool {
        self.berichtsoorten.is_empty()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    verzending: Option<Verzending>,
    berichtsoorten: Vec<Soort>,
}

/// How a message goes out: sending it is a fact the sender records in its
/// own chronicle, with the message frozen as it went (`inhoud`, `hash`); a
/// later build of the message does not change it. Forwarding it to another
/// cell is a fact whose receipt the core records in the chronicle of that
/// cell (a fact of the same article with the same parameters).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Verzending {
    /// `<regulation>#<article>#<fact>`: the fact of sending.
    feit: String,
    /// `<regulation>#<article>#<fact>`: the fact of forwarding.
    doorsturen: Option<String>,
    /// Further parameters of the fact and where their value comes from,
    /// such as whom the message is about.
    #[serde(default)]
    #[allow(dead_code)]
    velden: BTreeMap<String, serde_yaml_ng::Value>,
}

/// The fields the fact of sending must hold: what the message is, what it
/// belongs to, how and when it went, and its frozen content.
pub const VERZENDVELDEN: [&str; 7] = [
    "berichtsoort",
    "vastlegging",
    "kanaal",
    "verzonden_op",
    "inhoud",
    "hash",
    "vervangt",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Soort {
    id: String,
    /// What the message is called; the screen reads it.
    #[allow(dead_code)]
    naam: Option<String>,
    patroon: String,
    vastlegging: Vastlegging,
    sjabloon: Option<String>,
    #[serde(default)]
    sleutels: BTreeMap<String, serde_yaml_ng::Value>,
    delen: Vec<Deel>,
    #[serde(default)]
    invarianten: Vec<Invariant>,
    status: Option<serde_yaml_ng::Value>,
    kanaal: Option<Kanaal>,
}

/// The record a kind of message follows from (`vastlegging`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vastlegging {
    /// `besluit`, `indiening` or `vastlegging`.
    rol: String,
    /// `<regelwerk>#<article>`.
    artikel: String,
    /// `<regelwerk>#<article>#<fact>`, with `rol: vastlegging`.
    feit: Option<String>,
    /// The legal character the article produces (`produces`), such as
    /// `BESCHIKKING`: the kind of the record. The screen checks it.
    #[allow(dead_code)]
    rechtskarakter: Option<String>,
    /// The outcome of the article that carries the decision.
    #[allow(dead_code)]
    uitkomst: Option<String>,
    /// The step of the process the record is, as the chronicle names it.
    #[allow(dead_code)]
    stap: Option<String>,
    /// When two kinds of message follow from records of one article, the
    /// condition on the record under which this one does, such as a decision
    /// not to deal with an application beside the decision on it.
    als: Option<serde_yaml_ng::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Deel {
    id: String,
    plek: String,
    vereist_door: Option<String>,
    #[serde(default)]
    bron: Bron,
    als: Option<String>,
    nldd: Option<String>,
    /// The heading of the part, where the template gives none; the screen reads it.
    #[allow(dead_code)]
    titel: Option<String>,
}

/// Where the content of a part comes from. More than one may be given; the
/// screen shows them in this order.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bron {
    /// Paragraphs of the template: by heading (a string, `''` for the
    /// paragraphs without one) or by position (a number).
    #[serde(default)]
    tekst: Vec<serde_yaml_ng::Value>,
    /// Fields of the record, or of the submission it answers, with their
    /// values.
    #[serde(default)]
    #[allow(dead_code)]
    regels: Vec<String>,
    /// A specification: rows of amounts and their total.
    tabel: Option<Tabel>,
    /// An annex in columns: the rows of a key whose value is a list.
    bijlage: Option<Bijlage>,
    /// What the law gives ([`WET`]).
    wet: Option<String>,
}

/// An annex in columns, such as a calculation per body: the rows of the key
/// `rijen` (a list of rows), grouped by the field `groep`, with the amount
/// `bedrag` summed per group and in total. Every text comes from the text
/// layer (`bijlagen.<labels>`); the spec names fields and label keys only.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bijlage {
    labels: String,
    rijen: Rijen,
    groep: Option<String>,
    /// The field of a row that is its amount; without it the annex has no
    /// sums.
    #[allow(dead_code)]
    bedrag: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    bedragen: Vec<String>,
    kolommen: Vec<Kolom>,
    /// A key whose value the total of the rows must be.
    totaal: Option<String>,
}

/// The rows of an annex: a key whose value is a list of rows, or rows the
/// spec builds from keys (per field of the row a key, under `als`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Rijen {
    Sleutel(String),
    Uit(Vec<Rij>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rij {
    als: Option<String>,
    velden: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Kolom {
    kop: String,
    tekst: String,
    onder: Option<String>,
    #[allow(dead_code)]
    onder_als: Option<serde_yaml_ng::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tabel {
    /// Per row a key of the spec (`sleutels`) whose value is a number.
    rijen: Vec<String>,
    /// The key whose value is the total of the rows.
    totaal: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invariant {
    soort: String,
    /// `bedrag_is_totaal`: the key of the amount in the decision.
    bedrag: Option<String>,
    /// `bedrag_is_totaal`: the part with the specification.
    specificatie: Option<String>,
    /// `oordeel_past`: per judgment (a key that is a condition) what must
    /// hold when it does.
    #[serde(default)]
    oordelen: BTreeMap<String, serde_yaml_ng::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Kanaal {
    /// A channel the authority always uses ([`KANALEN`]).
    vast: Option<String>,
    /// A key whose value names the channel.
    sleutel: Option<String>,
    /// Per value of that key the channel it is.
    #[serde(default)]
    waarden: BTreeMap<String, String>,
}

/// The kinds of source a key may have: an outcome of the record, a field of
/// the record or its submission, an outcome in the chain of the decision,
/// what the submission gives under its own article, the record itself, a
/// label from the UI spec, the sender from the text layer, or a sentence of
/// the text layer (`zinnen`) that stands only when its own keys have a
/// value, such as the sentence with the last day to object.
pub const BRONSOORTEN: [&str; 8] = [
    "uitkomst",
    "veld",
    "keten",
    "indiening",
    "record",
    "label",
    "afzender",
    "zin",
];

/// Beside its kind and test, a source may say how its value is written
/// (`vorm`) and, for a field, that it is read from the record alone and not
/// from the submission it answers (`eigen: true`), such as the grounds of a
/// decision on an objection, which are not those of the decision contested.
///
/// The tests a condition may make of a value.
pub const TOETSEN: [&str; 5] = ["is", "gelijk", "groter_dan", "gevuld", "niet_gevuld"];

/// The message spec in `spec`, with the text layer in `teksten`.
///
/// # Errors
///
/// When either is no YAML of its form, or the spec refuses as the module
/// says: a kind of message without a valid record, pattern or template, a
/// part in no place or with an unknown element, a required part absent, a
/// key without a source or with an unknown one, an invariant that names
/// what is not there.
pub fn lees(spec: &str, teksten: Option<&str>) -> Result<Berichtenspec, String> {
    let ruw: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(spec).map_err(|e| format!("the message spec: {e}"))?;
    let gelezen: Spec =
        serde_yaml_ng::from_value(ruw.clone()).map_err(|e| format!("the message spec: {e}"))?;
    let tekstlaag: Option<serde_yaml_ng::Value> = teksten
        .map(|t| serde_yaml_ng::from_str(t).map_err(|e| format!("the text layer: {e}")))
        .transpose()?;
    let sjablonen = sjablonen(tekstlaag.as_ref())?;
    let zinnen: BTreeMap<String, BTreeSet<String>> = tekstlaag
        .as_ref()
        .and_then(|t| t.get("zinnen"))
        .and_then(serde_yaml_ng::Value::as_mapping)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.as_str()?.to_string(), sleutels_in(v.as_str()?))))
                .collect()
        })
        .unwrap_or_default();
    let bijlagen: BTreeMap<String, BTreeSet<String>> = tekstlaag
        .as_ref()
        .and_then(|t| t.get("bijlagen"))
        .and_then(serde_yaml_ng::Value::as_mapping)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let labels = v
                        .as_mapping()?
                        .keys()
                        .filter_map(|l| l.as_str().map(str::to_string))
                        .collect();
                    Some((k.as_str()?.to_string(), labels))
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(v) = &gelezen.verzending {
        for f in std::iter::once(&v.feit).chain(&v.doorsturen) {
            if f.splitn(3, '#').filter(|d| !d.is_empty()).count() != 3 {
                return Err(format!(
                    "the message spec: the fact of sending '{f}' is no <regulation>#<article>#<fact>"
                ));
            }
        }
    }
    let mut meldingen = Vec::new();
    let mut ids = BTreeSet::new();
    for s in &gelezen.berichtsoorten {
        if !ids.insert(s.id.as_str()) {
            return Err(format!(
                "the message spec: two kinds of message are called '{}'",
                s.id
            ));
        }
        controleer(s, &sjablonen, &zinnen, &bijlagen, &mut meldingen)?;
    }
    let berichtsoorten = ruw
        .get("berichtsoorten")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .map(|v| v.iter().map(naar_json).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    Ok(Berichtenspec {
        verzending: ruw.get("verzending").map(naar_json).transpose()?,
        berichtsoorten,
        tekstlaag: tekstlaag.as_ref().map(naar_json).transpose()?,
        meldingen,
    })
}

fn naar_json(v: &serde_yaml_ng::Value) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| format!("the message spec: {e}"))
}

/// One paragraph of a template: its heading, its condition, and the keys its
/// text and heading name.
struct Alinea {
    kop: String,
    als: Option<String>,
    sleutels: BTreeSet<String>,
}

/// The templates of the text layer by id, each with its paragraphs and the
/// keys its other texts (`betreft`, `betreft_bij`) name.
type Sjablonen = BTreeMap<String, (Vec<Alinea>, BTreeSet<String>)>;

fn sjablonen(tekstlaag: Option<&serde_yaml_ng::Value>) -> Result<Sjablonen, String> {
    let mut uit = Sjablonen::new();
    let Some(lijst) = tekstlaag
        .and_then(|t| t.get("sjablonen"))
        .and_then(serde_yaml_ng::Value::as_sequence)
    else {
        return Ok(uit);
    };
    for s in lijst {
        let Some(id) = s.get("id").and_then(serde_yaml_ng::Value::as_str) else {
            return Err("the text layer: a template without `id`".into());
        };
        let alineas = s
            .get("alineas")
            .and_then(serde_yaml_ng::Value::as_sequence)
            .map(|a| {
                a.iter()
                    .map(|a| {
                        let tekst = |k: &str| {
                            a.get(k)
                                .and_then(serde_yaml_ng::Value::as_str)
                                .unwrap_or("")
                        };
                        let mut sleutels = sleutels_in(tekst("tekst"));
                        sleutels.extend(sleutels_in(tekst("kop")));
                        Alinea {
                            kop: tekst("kop").to_string(),
                            als: a
                                .get("als")
                                .and_then(serde_yaml_ng::Value::as_str)
                                .map(str::to_string),
                            sleutels,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut overig = BTreeSet::new();
        if let Some(b) = s.get("betreft").and_then(serde_yaml_ng::Value::as_str) {
            overig.extend(sleutels_in(b));
        }
        if let Some(serde_yaml_ng::Value::Mapping(bij)) = s.get("betreft_bij") {
            for (k, v) in bij {
                overig.extend(k.as_str().map(str::to_string));
                overig.extend(v.as_str().map(sleutels_in).unwrap_or_default());
            }
        }
        if uit.insert(id.to_string(), (alineas, overig)).is_some() {
            return Err(format!("the text layer: two templates are called '{id}'"));
        }
    }
    Ok(uit)
}

/// The keys `{naam}` in `tekst`. A key is a name of letters, digits and
/// underscores; other braces are text.
pub fn sleutels_in(tekst: &str) -> BTreeSet<String> {
    let mut uit = BTreeSet::new();
    let mut rest = tekst;
    while let Some(begin) = rest.find('{') {
        let na = &rest[begin + 1..];
        let Some(eind) = na.find('}') else { break };
        let naam = &na[..eind];
        if !naam.is_empty() && naam.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            uit.insert(naam.to_string());
        }
        rest = &na[eind + 1..];
    }
    uit
}

fn controleer(
    s: &Soort,
    sjablonen: &Sjablonen,
    zinnen: &BTreeMap<String, BTreeSet<String>>,
    bijlagen: &BTreeMap<String, BTreeSet<String>>,
    meldingen: &mut Vec<String>,
) -> Result<(), String> {
    let waar = format!("the message spec: kind '{}'", s.id);
    let v = &s.vastlegging;
    if !["besluit", "indiening", "vastlegging"].contains(&v.rol.as_str()) {
        return Err(format!(
            "{waar} follows from a '{}'; a message follows from a `besluit`, an `indiening` or a `vastlegging`",
            v.rol
        ));
    }
    if v.artikel
        .split_once('#')
        .is_none_or(|(w, a)| w.is_empty() || a.is_empty())
    {
        return Err(format!(
            "{waar}: the record's article '{}' is no <regelwerk>#<article>",
            v.artikel
        ));
    }
    match (&v.feit, v.rol.as_str()) {
        (None, "vastlegging") => {
            return Err(format!(
                "{waar} follows from a fact and names none (`feit`)"
            ));
        }
        (Some(f), _) if !f.starts_with(&format!("{}#", v.artikel)) => {
            return Err(format!(
                "{waar}: the fact '{f}' is not of the article '{}'",
                v.artikel
            ));
        }
        _ => {}
    }
    if let Some(als) = &v.als {
        let m = als.as_mapping();
        let op_record = m.is_some_and(|m| {
            m.keys()
                .filter_map(serde_yaml_ng::Value::as_str)
                .all(|k| ["uitkomst", "veld"].contains(&k) || TOETSEN.contains(&k))
        });
        if !op_record {
            return Err(format!(
                "{waar}: `vastlegging.als` tests an `uitkomst` or a `veld` of the record"
            ));
        }
        bron_van(als).map_err(|e| format!("{waar}: `vastlegging.als` {e}"))?;
    }
    let Some((_, vereist)) = PATRONEN.iter().find(|(p, _)| *p == s.patroon) else {
        return Err(format!(
            "{waar} follows the pattern '{}', which is none of {}",
            s.patroon,
            PATRONEN
                .iter()
                .map(|(p, _)| *p)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    // Every key has a source of a known kind, with a known test.
    for (naam, bron) in &s.sleutels {
        bron_van(bron).map_err(|e| format!("{waar}: the key '{naam}' {e}"))?;
    }
    let bekend = |n: &str| s.sleutels.contains_key(n);
    // A sentence of the text layer exists there, and its own keys have a source.
    for (naam, bron) in &s.sleutels {
        if let Some(zin) = bron.get("zin").and_then(serde_yaml_ng::Value::as_str) {
            let Some(eigen) = zinnen.get(zin) else {
                return Err(format!(
                    "{waar}: the key '{naam}' is the sentence '{zin}', which the text layer has not (`zinnen`)"
                ));
            };
            if let Some(n) = eigen.iter().find(|n| !bekend(n)) {
                return Err(format!(
                    "{waar}: the sentence '{zin}' names '{n}', a key without a source"
                ));
            }
        }
    }
    let mut delen = BTreeSet::new();
    for d in &s.delen {
        if !delen.insert(d.id.as_str()) {
            return Err(format!("{waar}: two parts are called '{}'", d.id));
        }
        if !PLEKKEN.contains(&d.plek.as_str()) {
            return Err(format!(
                "{waar}: part '{}' stands in '{}', which is no place of a message ({})",
                d.id,
                d.plek,
                PLEKKEN.join(", ")
            ));
        }
        if let Some(e) = &d.nldd
            && !NLDD.contains(&e.as_str())
        {
            return Err(format!(
                "{waar}: part '{}' is shown as '{e}', which the screen does not know ({})",
                d.id,
                NLDD.join(", ")
            ));
        }
        if let Some(w) = &d.bron.wet
            && !WET.contains(&w.as_str())
        {
            return Err(format!(
                "{waar}: part '{}' takes '{w}' from the law, which gives {}",
                d.id,
                WET.join(", ")
            ));
        }
        if let Some(als) = &d.als
            && !bekend(als)
        {
            return Err(format!(
                "{waar}: part '{}' stands when '{als}', a key without a source",
                d.id
            ));
        }
        if let Some(b) = &d.bron.bijlage {
            let sleutels: Vec<&String> = match &b.rijen {
                Rijen::Sleutel(k) => vec![k],
                Rijen::Uit(rijen) => rijen
                    .iter()
                    .flat_map(|r| r.velden.values().chain(&r.als))
                    .collect(),
            };
            for k in sleutels.into_iter().chain(&b.totaal) {
                if !bekend(k) {
                    return Err(format!(
                        "{waar}: the annex of part '{}' reads '{k}', a key without a source",
                        d.id
                    ));
                }
            }
            let Some(labels) = bijlagen.get(&b.labels) else {
                return Err(format!(
                    "{waar}: the annex of part '{}' takes its texts from '{}', which the text layer has not (`bijlagen`)",
                    d.id, b.labels
                ));
            };
            let nodig = b.kolommen.iter().flat_map(|k| {
                std::iter::once(&k.kop)
                    .chain(std::iter::once(&k.tekst))
                    .chain(&k.onder)
            });
            let vast = b.bedrag.as_ref().map(|_| "totaal".to_string());
            let groep = b.groep.as_ref().map(|_| "groep".to_string());
            for l in nodig.chain(&vast).chain(&groep) {
                if !labels.contains(l) {
                    return Err(format!(
                        "{waar}: the annex of part '{}' names the text '{l}', which '{}' of the text layer has not",
                        d.id, b.labels
                    ));
                }
            }
        }
        if let Some(t) = &d.bron.tabel {
            for r in t.rijen.iter().chain(&t.totaal) {
                if !bekend(r) {
                    return Err(format!(
                        "{waar}: the table of part '{}' names '{r}', a key without a source",
                        d.id
                    ));
                }
            }
        }
    }
    // The places the pattern requires each have a part that says so.
    for (plek, artikel) in *vereist {
        if !s
            .delen
            .iter()
            .any(|d| d.plek == *plek && d.vereist_door.is_some())
        {
            return Err(format!(
                "{waar}: the pattern '{}' requires a part in '{plek}' ({artikel}), with `vereist_door`",
                s.patroon
            ));
        }
    }
    // The template: every paragraph is taken by a part, and every key and
    // condition it names has a source.
    if let Some(id) = &s.sjabloon {
        let Some((alineas, overig)) = sjablonen.get(id) else {
            return Err(format!(
                "{waar}: the template '{id}' is not in the text layer"
            ));
        };
        for n in overig {
            if !bekend(n) {
                return Err(format!(
                    "{waar}: the template '{id}' names '{n}', a key without a source"
                ));
            }
        }
        for (i, a) in alineas.iter().enumerate() {
            let genomen = s.delen.iter().any(|d| {
                d.bron.tekst.iter().any(|t| match t {
                    serde_yaml_ng::Value::String(k) => *k == a.kop,
                    serde_yaml_ng::Value::Number(n) => n.as_u64() == Some(i as u64),
                    _ => false,
                })
            });
            let naam = if a.kop.is_empty() {
                format!("paragraph {}", i + 1)
            } else {
                format!("'{}'", a.kop)
            };
            if !genomen {
                // Reported, not refused: the paragraph shows in no message, and the screen says so.
                // Its keys need no source, since nothing reads them.
                meldingen.push(format!(
                    "{}: {naam} of the template '{id}' stands in no part, so no message of this kind shows it",
                    s.id
                ));
                continue;
            }
            for n in a.sleutels.iter().chain(&a.als) {
                if !bekend(n) {
                    return Err(format!(
                        "{waar}: {naam} of the template '{id}' names '{n}', a key without a source"
                    ));
                }
            }
        }
    } else if s.delen.iter().any(|d| !d.bron.tekst.is_empty()) {
        return Err(format!(
            "{waar} takes paragraphs of a template and names none (`sjabloon`)"
        ));
    }
    for (i, inv) in s.invarianten.iter().enumerate() {
        let w = format!("{waar}: invariant {} ({})", i + 1, inv.soort);
        match inv.soort.as_str() {
            "bedrag_is_totaal" => {
                let bedrag = inv
                    .bedrag
                    .as_deref()
                    .ok_or_else(|| format!("{w} names no `bedrag`"))?;
                if !bekend(bedrag) {
                    return Err(format!("{w}: '{bedrag}' is a key without a source"));
                }
                let spec = inv
                    .specificatie
                    .as_deref()
                    .ok_or_else(|| format!("{w} names no `specificatie`"))?;
                if !s
                    .delen
                    .iter()
                    .any(|d| d.id == spec && (d.bron.tabel.is_some() || d.bron.bijlage.is_some()))
                {
                    return Err(format!("{w}: '{spec}' is no part with a table or an annex"));
                }
            }
            "oordeel_past" => {
                if inv.oordelen.is_empty() {
                    return Err(format!("{w} names no `oordelen`"));
                }
                for (oordeel, eis) in &inv.oordelen {
                    if !bekend(oordeel) {
                        return Err(format!(
                            "{w}: the judgment '{oordeel}' is a key without a source"
                        ));
                    }
                    voorwaarde(eis, &bekend)
                        .map_err(|e| format!("{w}: what '{oordeel}' requires {e}"))?;
                }
            }
            ander => {
                return Err(format!(
                    "{waar}: an invariant of kind '{ander}'; the kinds are `bedrag_is_totaal` and `oordeel_past`"
                ));
            }
        }
    }
    if let Some(k) = &s.kanaal {
        let kanaal = |c: &str| KANALEN.iter().any(|(k, _)| *k == c);
        match (&k.vast, &k.sleutel) {
            (Some(c), None) if kanaal(c) => {}
            (None, Some(n))
                if bekend(n) && k.waarden.values().all(|c| kanaal(c)) && !k.waarden.is_empty() => {}
            _ => {
                return Err(format!(
                    "{waar}: the channel is `vast` (one of {}) or a `sleutel` with `waarden` to those",
                    KANALEN
                        .iter()
                        .map(|(k, _)| *k)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }
    if let Some(st) = &s.status {
        let Some(b) = st.get("bekendgemaakt") else {
            return Err(format!(
                "{waar}: `status` says when the message is made known (`bekendgemaakt`)"
            ));
        };
        voorwaarde(b, &bekend).map_err(|e| format!("{waar}: `status.bekendgemaakt` {e}"))?;
    }
    Ok(())
}

/// Checks the source of a key: one kind of [`BRONSOORTEN`] with a name, and
/// at most one test of [`TOETSEN`]; or a condition over keys.
fn bron_van(b: &serde_yaml_ng::Value) -> Result<(), String> {
    let serde_yaml_ng::Value::Mapping(m) = b else {
        return Err("has no source: a key is `{<kind>: <name>}`".into());
    };
    if ["alle", "een_van", "niet"]
        .iter()
        .any(|k| m.contains_key(*k))
    {
        // A condition over other keys; checked as a condition with every key known.
        return voorwaarde(b, &|_| true);
    }
    let soorten: Vec<&str> = m
        .keys()
        .filter_map(serde_yaml_ng::Value::as_str)
        .filter(|k| BRONSOORTEN.contains(k))
        .collect();
    if soorten.len() != 1 {
        return Err(format!(
            "has {} sources; a key has one of {}",
            soorten.len(),
            BRONSOORTEN.join(", ")
        ));
    }
    let toetsen = m
        .keys()
        .filter_map(serde_yaml_ng::Value::as_str)
        .filter(|k| TOETSEN.contains(k))
        .count();
    if toetsen > 1 {
        return Err("makes more than one test".into());
    }
    for k in m.keys().filter_map(serde_yaml_ng::Value::as_str) {
        if !BRONSOORTEN.contains(&k) && !TOETSEN.contains(&k) && k != "vorm" && k != "eigen" {
            return Err(format!("has the unknown key `{k}`"));
        }
    }
    Ok(())
}

/// Checks a condition: `{sleutel: <key>, <test>}`, a source with a test, or
/// `alle`, `een_van` and `niet` over conditions.
fn voorwaarde(v: &serde_yaml_ng::Value, bekend: &dyn Fn(&str) -> bool) -> Result<(), String> {
    let serde_yaml_ng::Value::Mapping(m) = v else {
        return Err("is no condition".into());
    };
    if let Some(lijst) = m.get("alle").or_else(|| m.get("een_van")) {
        let Some(lijst) = lijst.as_sequence() else {
            return Err("lists no conditions".into());
        };
        return lijst.iter().try_for_each(|c| voorwaarde(c, bekend));
    }
    if let Some(c) = m.get("niet") {
        return voorwaarde(c, bekend);
    }
    if let Some(n) = m.get("sleutel").and_then(serde_yaml_ng::Value::as_str) {
        if !bekend(n) {
            return Err(format!("names '{n}', a key without a source"));
        }
        let mut zonder = m.clone();
        zonder.remove("sleutel");
        zonder.insert("record".into(), "id".into());
        return bron_van(&serde_yaml_ng::Value::Mapping(zonder));
    }
    bron_van(v)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const TEKSTEN: &str = "
afzender: {organisatie: Het college}
sjablonen:
  - id: besluit
    betreft_bij: {toekenning: 'Toekenning {jaar}', afwijzing: Afwijzing}
    alineas:
      - tekst: 'U vroeg op {dag}.'
      - {kop: Besluit, als: toekenning, tekst: 'Wij kennen {bedrag} toe.'}
      - {kop: Besluit, als: afwijzing, tekst: Wij wijzen af.}
      - {kop: Bezwaar, tekst: Binnen zes weken.}
";

    fn spec(delen: &str, extra: &str) -> String {
        format!(
            "
berichtsoorten:
  - id: beschikking
    patroon: beschikking
    vastlegging: {{rol: besluit, artikel: 'w#1', rechtskarakter: BESCHIKKING, uitkomst: toegekend}}
    sjabloon: besluit
    sleutels:
      toekenning: {{uitkomst: toegekend, is: true}}
      afwijzing: {{uitkomst: toegekend, is: false}}
      bedrag: {{uitkomst: bedrag, vorm: bedrag}}
      jaar: {{veld: jaar}}
      dag: {{record: recorded_on, vorm: datum}}
      deel_a: {{keten: a}}
      deel_b: {{keten: b}}
{extra}
    delen:
{delen}
"
        )
    }

    const DELEN: &str = "
      - {id: inleiding, plek: inleiding, bron: {tekst: ['']}}
      - {id: dictum, plek: dictum, vereist_door: 'Awb 1:3', bron: {tekst: [Besluit]}, nldd: nldd-box}
      - {id: grondslag, plek: grondslag, vereist_door: 'Awb 3:47', bron: {wet: grondslag}}
      - {id: motivering, plek: motivering, vereist_door: 'Awb 3:46', bron: {wet: keten}}
      - {id: specificatie, plek: specificatie, als: toekenning, bron: {tabel: {rijen: [deel_a, deel_b], totaal: bedrag}}, nldd: nldd-table}
      - {id: ondertekening, plek: ondertekening, vereist_door: 'Awb 10:10', bron: {wet: ondertekening}}
      - {id: rechtsmiddelen, plek: rechtsmiddelen, vereist_door: 'Awb 3:45', bron: {tekst: [Bezwaar], wet: rechtsmiddelen}, nldd: nldd-box}";

    #[test]
    fn a_spec_with_its_text_layer_reads_and_passes_on_as_written() {
        let s = lees(
            &spec(
                DELEN,
                "    invarianten:
      - {soort: bedrag_is_totaal, bedrag: bedrag, specificatie: specificatie}
      - soort: oordeel_past
        oordelen:
          toekenning: {sleutel: bedrag, groter_dan: 0}
          afwijzing: {een_van: [{sleutel: bedrag, gelijk: 0}, {sleutel: bedrag, niet_gevuld: true}]}
    kanaal: {vast: portaal}
    status: {bekendgemaakt: {uitkomst: bekendgemaakt, is: true}}",
            ),
            Some(TEKSTEN),
        )
        .unwrap();
        assert_eq!(s.berichtsoorten.len(), 1);
        assert_eq!(s.berichtsoorten[0]["delen"][1]["nldd"], "nldd-box");
        assert_eq!(
            s.tekstlaag.as_ref().unwrap()["afzender"]["organisatie"],
            "Het college"
        );
        assert!(s.meldingen.is_empty(), "{:?}", s.meldingen);
    }

    #[test]
    fn a_paragraph_no_part_takes_is_reported_not_dropped() {
        let zonder_bezwaar = DELEN.replace(
            "bron: {tekst: [Bezwaar], wet: rechtsmiddelen}",
            "bron: {wet: rechtsmiddelen}",
        );
        let s = lees(&spec(&zonder_bezwaar, ""), Some(TEKSTEN)).unwrap();
        assert_eq!(s.meldingen.len(), 1);
        assert!(s.meldingen[0].contains("'Bezwaar'"), "{:?}", s.meldingen);
        // A paragraph no part takes needs no source for its keys: nothing reads them.
        let zonder_besluit = DELEN.replace("bron: {tekst: [Besluit]}", "bron: {}");
        let zonder_bron = spec(&zonder_besluit, "")
            .replace("      bedrag: {uitkomst: bedrag, vorm: bedrag}\n", "");
        let s = lees(
            &zonder_bron.replace(
                "rijen: [deel_a, deel_b], totaal: bedrag",
                "rijen: [deel_a, deel_b]",
            ),
            Some(TEKSTEN),
        )
        .unwrap();
        assert_eq!(s.meldingen.len(), 2, "{:?}", s.meldingen);
        assert!(
            s.meldingen.iter().all(|m| m.contains("'Besluit'")),
            "{:?}",
            s.meldingen
        );
    }

    #[test]
    fn what_would_lose_a_part_or_a_key_is_refused() {
        let fout = |delen: &str, extra: &str| lees(&spec(delen, extra), Some(TEKSTEN)).unwrap_err();
        // A required part of the pattern is absent.
        let e = fout(&DELEN.replace("vereist_door: 'Awb 3:45', ", ""), "");
        assert!(
            e.contains("requires a part in 'rechtsmiddelen' (Awb 3:45)"),
            "{e}"
        );
        // A place no message has, an element the screen does not know.
        let e = fout(&DELEN.replace("plek: specificatie", "plek: bijlage_x"), "");
        assert!(e.contains("which is no place"), "{e}");
        let e = fout(
            &DELEN.replace("nldd: nldd-table", "nldd: nldd-spreadsheet"),
            "",
        );
        assert!(e.contains("does not know"), "{e}");
        // A key of the template without a source.
        let e = lees(
            &spec(DELEN, "").replace("      jaar: {veld: jaar}\n", ""),
            Some(TEKSTEN),
        )
        .unwrap_err();
        assert!(e.contains("'jaar', a key without a source"), "{e}");
        // An invariant that names a part without a table.
        let e = fout(
            DELEN,
            "    invarianten: [{soort: bedrag_is_totaal, bedrag: bedrag, specificatie: dictum}]",
        );
        assert!(e.contains("no part with a table"), "{e}");
        // A source of an unknown kind; two sources.
        let e = lees(
            &spec(DELEN, "").replace("{veld: jaar}", "{tabblad: jaar}"),
            Some(TEKSTEN),
        )
        .unwrap_err();
        assert!(e.contains("has 0 sources"), "{e}");
        // A fact message without its fact, a template not in the text layer.
        let e = lees(
            &spec(DELEN, "").replace("rol: besluit", "rol: vastlegging"),
            Some(TEKSTEN),
        )
        .unwrap_err();
        assert!(e.contains("names none (`feit`)"), "{e}");
        let e = lees(
            &spec(DELEN, "").replace("sjabloon: besluit", "sjabloon: brief"),
            Some(TEKSTEN),
        )
        .unwrap_err();
        assert!(e.contains("not in the text layer"), "{e}");
        // An unknown key in a part is refused, not left aside.
        let e = fout(
            &DELEN.replace("{id: inleiding,", "{id: inleiding, plaats: x,"),
            "",
        );
        assert!(e.contains("unknown field `plaats`"), "{e}");
    }

    #[test]
    fn an_annex_takes_its_rows_from_a_key_and_every_text_from_the_text_layer() {
        let bijlage = "\n      - {id: berekening, plek: bijlage, bron: {bijlage: {labels: per_deel, rijen: regels, groep: deel, bedrag: bedrag, kolommen: [{kop: kop_deel, tekst: cel_deel}], totaal: bedrag}}}";
        let delen = format!("{DELEN}{bijlage}");
        let extra = "      regels: {uitkomst: regels}";
        let teksten = format!(
            "{TEKSTEN}bijlagen:\n  per_deel: {{kop_deel: Deel, cel_deel: '{{deel}}', groep: '{{deel}}', totaal: Totaal}}\n"
        );
        let met = spec(&delen, "").replace(
            "      deel_a: {keten: a}\n",
            &format!("{extra}\n      deel_a: {{keten: a}}\n"),
        );
        let s = lees(&met, Some(&teksten)).unwrap();
        assert_eq!(
            s.berichtsoorten[0]["delen"][7]["bron"]["bijlage"]["labels"],
            "per_deel"
        );
        // A text of the annex the text layer has not, and a row key without a source, are refused.
        let e = lees(
            &met,
            Some(&teksten.replace("totaal: Totaal", "eind: Totaal")),
        )
        .unwrap_err();
        assert!(e.contains("names the text 'totaal'"), "{e}");
        let e = lees(&met.replace(extra, ""), Some(&teksten)).unwrap_err();
        assert!(e.contains("reads 'regels', a key without a source"), "{e}");
        // Rows the spec builds from keys: each key needs a source too.
        let uit = met.replace(
            "rijen: regels",
            "rijen: [{als: toekenning, velden: {deel: jaar, bedrag: bedrag}}]",
        );
        lees(&uit, Some(&teksten)).unwrap();
        let e = lees(&uit.replace("deel: jaar", "deel: onbekend"), Some(&teksten)).unwrap_err();
        assert!(e.contains("'onbekend', a key without a source"), "{e}");
    }

    #[test]
    fn sending_names_a_fact_and_passes_on_as_written() {
        let met = format!(
            "verzending: {{feit: 'w#2#bericht_verzonden', doorsturen: 'w#2#bericht_doorgestuurd'}}\n{}",
            spec(DELEN, "")
        );
        let s = lees(&met, Some(TEKSTEN)).unwrap();
        assert_eq!(
            s.verzending.as_ref().unwrap()["feit"],
            "w#2#bericht_verzonden"
        );
        let e = lees(
            &met.replace("w#2#bericht_verzonden", "bericht_verzonden"),
            Some(TEKSTEN),
        )
        .unwrap_err();
        assert!(e.contains("is no <regulation>#<article>#<fact>"), "{e}");
    }

    #[test]
    fn the_keys_of_a_text_are_its_names_in_braces() {
        assert_eq!(
            sleutels_in("Op {dag} {bedrag}, niet {een zin} of {}."),
            BTreeSet::from(["dag".to_string(), "bedrag".to_string()])
        );
    }
}
