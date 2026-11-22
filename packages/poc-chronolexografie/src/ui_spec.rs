//! What the poc reads from the UI spec of a source (`ui_spec.yaml` at the
//! root of the source, the file its manifest names): per field of the
//! application, how the form fills and shows it ([`UiSpec`]), and the views
//! the authority sees beside each of its decisions ([`Weergave`]).
//!
//! A UI spec is a register of elements, each with what a screen shows, asks
//! and fills, and on what grounds. It comes in two forms:
//!
//! - per field: `elements`, each with `fields`, and a field the form
//!   prefills carries `fill: {from: <source>}`;
//! - the older form: `elementen`, each with a map `prefill` from field to
//!   source.
//!
//! In the per-field form a field says more about the application form:
//!
//! - `fill: {from: <source>}`: the form prefills it, and the applicant may
//!   change it;
//! - `fill: {by: system}`: the system fills it and the applicant does not
//!   type it ([`Systeemvulling`]). Without more, the login fills it, with the
//!   claim of that name; with `login: <claim>` the login fills it with that
//!   claim, such as the name. With `from: <source>` the cell computes it, as
//!   a prefill, and `parameters` may give the source what the system knows
//!   of the submission under the names of its parameters
//!   (`{datum_indiening: date}`). With `value` it is that value; with
//!   `submission: date` it is the day the form submits on, with
//!   `submission: requested_decision` the article whose decision the
//!   application asks for (`requests`), and with `submission: login_method`
//!   the means the applicant logged in with ([`Indieningsgegeven`]);
//! - `visibility: hidden`: the form does not show it. It still submits what
//!   fills it. `visibility: shown` on a field the system fills shows it
//!   without letting the applicant change it.
//!
//! What a field says counts over every element, as a prefill does, so an
//! element of another screen that names a field of the application says it
//! for the form too.
//!
//! A field of the per-field form may also carry `answer: {from: <source>,
//! decision: <parameter>}`: the element then shows, beside each decision of
//! the case's cell, what the cell answers for that source, with the id of the
//! decision as the parameter `decision`. The fields of such an element that
//! it asks (`visibility: asked`) are what the authority gives those sources
//! besides, such as a rate of interest no regelwerk holds. Such an element
//! is no part of the application form, so its `visibility` and `by` say
//! nothing about it.
//!
//! A field may also say how a screen names it: `label`, the name of the
//! field on a screen, `help`, a line under it, and `group`, the heading of
//! the group it stands in ([`Veldtekst`]). Without them a screen names the
//! field after the regelwerk. They count by field name over every element
//! and every form, so the same field is named the same on the application,
//! the objection and the decision; two elements that name one field
//! differently are refused. The order the fields are named in, over the
//! elements, is the order a screen shows them in ([`UiSpec::volgorde`]); a
//! field the UI spec does not name follows, in the order of the law.
//!
//! The poc reads only those keys and leaves every other key to the register
//! and its own checks, so a register that grows a key does not stop a case
//! from loading. A source is `<regelwerk>#<article>#<output>`; what it gives
//! the cell computes when the form opens ([`crate::kern::voorinvullen`]).
//!
//! A UI spec that has neither list is refused. If it were read as empty, a
//! renamed key would turn every prefill off without a word. A UI spec that mixes the two
//! forms is refused too.

use std::collections::{BTreeMap, BTreeSet};

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

/// A field of an element; the poc reads only how the form fills and shows
/// it. A field without a name is left to the register, unless it says what
/// fills it (`from`, `value` or `submission`).
#[derive(Debug, Deserialize)]
struct Field {
    name: Option<String>,
    fill: Option<Fill>,
    visibility: Option<String>,
    answer: Option<Answer>,
    label: Option<String>,
    help: Option<String>,
    group: Option<String>,
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

/// What the poc reads from a UI spec: how the application form fills and
/// shows its fields, and the views beside a decision.
#[derive(Debug, Default, Clone)]
pub struct UiSpec {
    /// Per field of the application, the source the form prefills it from.
    pub prefill: BTreeMap<String, String>,
    /// Per field of the application the system fills, how.
    pub systeem: BTreeMap<String, Systeemvulling>,
    /// The fields of the application the form does not show.
    pub verborgen: BTreeSet<String>,
    /// The elements that show answers beside each decision of the case's cell.
    pub weergaven: Vec<Weergave>,
    /// Per field, how a screen names it, where the UI spec says so.
    pub teksten: BTreeMap<String, Veldtekst>,
    /// The fields in the order the UI spec names them, each once: the order
    /// a screen shows them in.
    pub volgorde: Vec<String>,
}

/// How a screen names a field (`label`), what it says under it (`help`) and
/// the heading of the group it stands in (`group`), where the UI spec gives
/// them; a screen falls back on the field's name in the regelwerk.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Veldtekst {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// How the system fills a field of the application (`fill: {by: system}`).
/// The applicant does not type it, and what fills it goes over what a
/// scenario or the case's example gives.
#[derive(Debug, Clone, PartialEq)]
pub enum Systeemvulling {
    /// The login, with the claim of the field's name; without a login, or
    /// without such a claim, nothing.
    Inloggen,
    /// The login, with the claim this names (`login`), such as the name of
    /// who logged in; without such a claim, nothing.
    Inlogclaim(String),
    /// What the cell answers for a source, `<regelwerk>#<article>#<output>`,
    /// on what the form holds, as it prefills a field (`from`), and on what
    /// the system knows of the submission (`parameters`).
    Bron {
        bron: String,
        /// Per parameter of the source, what of the submission it takes.
        parameters: BTreeMap<String, Indieningsgegeven>,
    },
    /// This value (`value`).
    Waarde(serde_json::Value),
    /// What the system knows of the submission itself (`submission`).
    Indiening(Indieningsgegeven),
}

/// What the system knows of a submission it sends, apart from what is in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Indieningsgegeven {
    /// The day it submits on (`date`).
    Datum,
    /// The article whose decision the application asks for, as the law
    /// says it (`requested_decision`; `requests`, RFC-045 §1).
    GevraagdBesluit,
    /// The means the applicant logged in with to submit it
    /// (`login_method`), such as `DigiD`; empty when the system knows of no
    /// login, as for a form without one.
    Inlogmiddel,
    /// The id of the record the submission refers to (`referred_record`),
    /// such as the decision that granted what an application for a
    /// duplicate is about; empty for a submission that refers to none.
    Verwezen,
}

impl Indieningsgegeven {
    /// The name a UI spec gives it.
    pub fn naam(self) -> &'static str {
        match self {
            Self::Datum => "date",
            Self::GevraagdBesluit => "requested_decision",
            Self::Inlogmiddel => "login_method",
            Self::Verwezen => "referred_record",
        }
    }

    /// The datum a UI spec names `naam`, if it is one.
    fn lees(naam: &str) -> Option<Self> {
        [
            Self::Datum,
            Self::GevraagdBesluit,
            Self::Inlogmiddel,
            Self::Verwezen,
        ]
        .into_iter()
        .find(|g| g.naam() == naam)
    }
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

/// How a field is filled: from a source (`from`), or by the system (`by:
/// system`), which may name what it fills the field with (`from`, `value` or
/// `submission`). An unknown key is refused, so that a misspelt `from` does
/// not drop a prefill.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    from: Option<String>,
    by: Option<Value>,
    /// Absent is `None`; `value: null` is `Some(Null)`, an empty value.
    #[serde(default, deserialize_with = "aanwezig")]
    value: Option<Value>,
    submission: Option<String>,
    /// The claim of the login that fills the field.
    login: Option<String>,
    /// Per parameter of the source `from`, what of the submission it takes.
    parameters: Option<BTreeMap<String, String>>,
}

/// A key that is present, `null` included: serde reads a `null` into an
/// `Option` as absent, and `value: null` is a value.
fn aanwezig<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
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

/// What one element says of the fields of the application form, with the
/// name it goes by in a message.
#[derive(Default)]
struct ElementVelden {
    name: String,
    /// Per prefilled field its source.
    sources: Vec<(String, String)>,
    /// Per field the system fills, how.
    systeem: Vec<(String, Systeemvulling)>,
    /// The fields it hides.
    verborgen: Vec<String>,
    /// The fields it asks (`visibility: asked`).
    gevraagd: Vec<String>,
}

/// Per field the source the form prefills it from, over every element of the
/// UI spec `tekst` ([`lees`]).
///
/// # Errors
///
/// As [`lees`].
#[cfg(test)]
pub fn prefill(tekst: &str) -> Result<BTreeMap<String, String>, String> {
    lees(tekst).map(|spec| spec.prefill)
}

/// What the poc reads from the UI spec `tekst`: per field how the form fills
/// and shows it, over every element (two elements may name the same source
/// for a field, as the form and the sheet that edits a field do), and the
/// views beside a decision.
///
/// # Errors
///
/// The text when it is no UI spec; a UI spec with neither `elements` nor
/// `elementen`, or with both; an element of one form that carries the key of
/// the other; a `fill` with an unknown key, with neither `from` nor `by`,
/// with `value` or `submission` but no `by: system`, with more than one of
/// `from`, `value` and `submission` under `by: system`, or with a
/// `submission` other than `date` or `requested_decision`; a field with both
/// `fill` and `answer`, or an `answer` without a `name`; a source that is no
/// `<regelwerk>#<article>#<output>`; a field two elements prefill from
/// different sources, that one element prefills and another fills by the
/// system, that two elements fill by the system differently, or that one
/// element asks while it or another hides it or fills it by the system. A
/// `by` other than `system` is no error: the field is left to the register.
pub fn lees(tekst: &str) -> Result<UiSpec, String> {
    let register: Register =
        serde_yaml_ng::from_str(tekst).map_err(|e| format!("the UI spec: {e}"))?;
    let teksten = register
        .elements
        .as_deref()
        .map(teksten)
        .transpose()?
        .unwrap_or_default();
    let volgorde = register
        .elements
        .as_deref()
        .map(volgorde)
        .unwrap_or_default();
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
    let (systeem, verborgen) = formulier(&elements)?;
    let prefill = merge(elements)?;
    if let Some(veld) = prefill.keys().find(|v| systeem.contains_key(*v)) {
        return Err(format!(
            "the UI spec both prefills '{veld}' and fills it by the system"
        ));
    }
    Ok(UiSpec {
        prefill,
        systeem,
        verborgen,
        weergaven,
        teksten,
        volgorde,
    })
}

/// The names of the fields of `elements`, each once, in the order they are
/// named.
fn volgorde(elements: &[Element]) -> Vec<String> {
    let mut gezien = BTreeSet::new();
    elements
        .iter()
        .flat_map(|e| &e.fields)
        .filter_map(|f| f.name.as_deref().filter(|n| !n.is_empty()))
        .filter(|n| gezien.insert(*n))
        .map(str::to_string)
        .collect()
}

/// Per field the `label`, `help` and `group` the elements give it. Two
/// elements that give one field a different label, help or group are
/// refused.
fn teksten(elements: &[Element]) -> Result<BTreeMap<String, Veldtekst>, String> {
    // Per field and per key (`label`, `help`, `group`): the text, and the element that gave it.
    let mut gegeven: BTreeMap<(String, &'static str), (String, String)> = BTreeMap::new();
    for (i, element) in elements.iter().enumerate() {
        let naam = element_name(element.id.as_ref(), i);
        for field in &element.fields {
            if field.label.is_none() && field.help.is_none() && field.group.is_none() {
                continue;
            }
            let Some(veld) = field.name.as_deref().filter(|v| !v.is_empty()) else {
                return Err(format!(
                    "the UI spec: {naam} gives a `label`, `help` or `group` to a field without a `name`"
                ));
            };
            for (wat, tekst) in [
                ("label", &field.label),
                ("help", &field.help),
                ("group", &field.group),
            ] {
                let Some(tekst) = tekst else { continue };
                match gegeven.get(&(veld.to_string(), wat)) {
                    Some((eerder, door)) if eerder != tekst => {
                        return Err(format!(
                            "the UI spec: {door} and {naam} give '{veld}' a different `{wat}`"
                        ));
                    }
                    Some(_) => {}
                    None => {
                        gegeven.insert((veld.to_string(), wat), (tekst.clone(), naam.clone()));
                    }
                }
            }
        }
    }
    let mut uit: BTreeMap<String, Veldtekst> = BTreeMap::new();
    for ((veld, wat), (tekst, _)) in gegeven {
        let t = uit.entry(veld).or_default();
        match wat {
            "label" => t.label = Some(tekst),
            "help" => t.help = Some(tekst),
            _ => t.group = Some(tekst),
        }
    }
    Ok(uit)
}

/// The prefill and the rest of the application form of the elements of the
/// per-field form, and the views beside a decision among them.
fn per_field(elements: Vec<Element>) -> Result<(Vec<ElementVelden>, Vec<Weergave>), String> {
    let mut uit = Vec::new();
    let mut weergaven = Vec::new();
    for (i, element) in elements.into_iter().enumerate() {
        let name = element_name(element.id.as_ref(), i);
        let weergave = weergave(&name, element.element.as_deref(), &element.fields)?;
        let is_weergave = weergave.is_some();
        if let Some(weergave) = weergave {
            if weergaven.iter().any(|w: &Weergave| w.naam == weergave.naam) {
                return Err(format!(
                    "the UI spec: two elements show answers beside a decision as '{}'; \
                     give each its own name",
                    weergave.naam
                ));
            }
            weergaven.push(weergave);
        }
        if element.prefill.is_some() {
            return Err(format!(
                "the UI spec: {name} has `prefill`, a key of the older form; \
                 in `elements` a field says `fill: {{from: ...}}`"
            ));
        }
        let mut velden = ElementVelden {
            name,
            ..ElementVelden::default()
        };
        for field in element.fields {
            lees_veld(&mut velden, field, is_weergave)?;
        }
        uit.push(velden);
    }
    Ok((uit, weergaven))
}

/// Add what `field` says of the application form to `velden`. A field of an
/// element that shows answers beside a decision (`weergave`) says nothing of
/// it, apart from a prefill.
fn lees_veld(velden: &mut ElementVelden, field: Field, weergave: bool) -> Result<(), String> {
    let name = velden.name.clone();
    let veld = field.name.unwrap_or_default();
    if !weergave && !veld.is_empty() {
        match field.visibility.as_deref() {
            Some("hidden") => velden.verborgen.push(veld.clone()),
            Some("asked") => velden.gevraagd.push(veld.clone()),
            _ => {}
        }
    }
    let Some(fill) = field.fill else {
        return Ok(());
    };
    let systeem = match &fill.by {
        None => false,
        Some(Value::String(by)) if by == "system" => true,
        // Filled by something the poc does not know: left to the register,
        // as an unknown key is.
        Some(_) => return Ok(()),
    };
    let vult = fill.from.is_some()
        || fill.value.is_some()
        || fill.submission.is_some()
        || fill.login.is_some();
    if veld.is_empty() && (vult || !systeem) {
        return Err(format!(
            "the UI spec: {name} fills a field without a `name`"
        ));
    }
    if !systeem {
        if fill.value.is_some()
            || fill.submission.is_some()
            || fill.login.is_some()
            || fill.parameters.is_some()
        {
            return Err(format!(
                "the UI spec: {name} gives '{veld}' a `value`, `submission`, `login` or \
                 `parameters` without `by: system`"
            ));
        }
        let Some(from) = fill.from else {
            return Err(format!(
                "the UI spec: {name} fills '{veld}' with neither `from` nor `by`"
            ));
        };
        velden.sources.push((veld, from));
        return Ok(());
    }
    if veld.is_empty() {
        return Ok(());
    }
    // Read in a view beside a decision too, so that a misspelling there is
    // refused as well; only the form uses it.
    let vulling = systeemvulling(&name, &veld, fill)?;
    if weergave {
        return Ok(());
    }
    if field.visibility.as_deref() == Some("asked") {
        return Err(format!(
            "the UI spec: {name} asks '{veld}' and fills it by the system; the applicant \
             does not type what the system fills"
        ));
    }
    velden.systeem.push((veld, vulling));
    Ok(())
}

/// How the system fills `veld` by `fill`, a `fill` with `by: system`.
fn systeemvulling(name: &str, veld: &str, fill: Fill) -> Result<Systeemvulling, String> {
    if fill.parameters.is_some() && fill.from.is_none() {
        return Err(format!(
            "the UI spec: {name} gives '{veld}' `parameters` without a source (`from`)"
        ));
    }
    match (fill.from, fill.value, fill.submission, fill.login) {
        (None, None, None, None) => Ok(Systeemvulling::Inloggen),
        (None, None, None, Some(claim)) if !claim.is_empty() => {
            Ok(Systeemvulling::Inlogclaim(claim))
        }
        (None, None, None, Some(_)) => Err(format!(
            "the UI spec: {name} fills '{veld}' from the login with an empty `login`"
        )),
        (Some(from), None, None, None) => {
            if !regelrecht_law_model::is_fact_reference(&from) {
                return Err(format!(
                    "the UI spec: {name} fills '{veld}' from '{from}', which is no \
                     <regelwerk>#<article>#<output>"
                ));
            }
            let parameters = fill
                .parameters
                .unwrap_or_default()
                .into_iter()
                .map(|(parameter, gegeven)| {
                    indieningsgegeven(name, veld, &gegeven).map(|g| (parameter, g))
                })
                .collect::<Result<_, _>>()?;
            Ok(Systeemvulling::Bron {
                bron: from,
                parameters,
            })
        }
        (None, Some(value), None, None) => serde_json::to_value(&value)
            .map(Systeemvulling::Waarde)
            .map_err(|e| {
                format!("the UI spec: {name} fills '{veld}' with a value JSON cannot hold: {e}")
            }),
        (None, None, Some(gegeven), None) => {
            indieningsgegeven(name, veld, &gegeven).map(Systeemvulling::Indiening)
        }
        _ => Err(format!(
            "the UI spec: {name} fills '{veld}' by the system with more than one of \
             `from`, `value`, `submission` and `login`"
        )),
    }
}

/// What of the submission `gegeven` names, as `veld` of the element `name`
/// takes it.
fn indieningsgegeven(name: &str, veld: &str, gegeven: &str) -> Result<Indieningsgegeven, String> {
    Indieningsgegeven::lees(gegeven).ok_or_else(|| {
        format!(
            "the UI spec: {name} fills '{veld}' with `{gegeven}` of the submission; \
             the system knows `date`, `requested_decision` and `login_method`"
        )
    })
}

/// Per field the system fills, how, and the fields the form hides, over
/// every element. Two elements that fill a field by the system fill it the
/// same way, and no element asks a field another hides.
fn formulier(
    elements: &[ElementVelden],
) -> Result<(BTreeMap<String, Systeemvulling>, BTreeSet<String>), String> {
    let mut systeem: BTreeMap<String, (Systeemvulling, &str)> = BTreeMap::new();
    for element in elements {
        for (veld, vulling) in &element.systeem {
            match systeem.get(veld) {
                Some((eerder, door)) if eerder != vulling => {
                    return Err(format!(
                        "the UI spec: {door} and {} fill '{veld}' by the system in different ways",
                        element.name
                    ));
                }
                Some(_) => {}
                None => {
                    systeem.insert(veld.clone(), (vulling.clone(), &element.name));
                }
            }
        }
    }
    let verborgen: BTreeSet<String> = elements
        .iter()
        .flat_map(|e| e.verborgen.iter().cloned())
        .collect();
    for element in elements {
        if let Some(veld) = element.gevraagd.iter().find(|v| verborgen.contains(*v)) {
            return Err(format!(
                "the UI spec: {} asks '{veld}', which another element hides",
                element.name
            ));
        }
        if let Some(veld) = element.gevraagd.iter().find(|v| systeem.contains_key(*v)) {
            return Err(format!(
                "the UI spec: {} asks '{veld}', which the system fills",
                element.name
            ));
        }
    }
    let systeem = systeem.into_iter().map(|(v, (s, _))| (v, s)).collect();
    Ok((systeem, verborgen))
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
        if answer.decision.is_empty() {
            return Err(format!(
                "the UI spec: {name} shows '{veld}' without the parameter that takes the decision"
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
fn older_form(elementen: Vec<OldElement>) -> Result<Vec<ElementVelden>, String> {
    let mut uit = Vec::new();
    for (i, element) in elementen.into_iter().enumerate() {
        let name = element_name(element.id.as_ref(), i);
        if element.fields.is_some() {
            return Err(format!(
                "the UI spec: {name} has `fields`, a key of the per-field form; \
                 put it under `elements`"
            ));
        }
        uit.push(ElementVelden {
            name,
            sources: element.prefill.into_iter().collect(),
            ..ElementVelden::default()
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
fn merge(elements: Vec<ElementVelden>) -> Result<BTreeMap<String, String>, String> {
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
    fn the_system_fills_from_the_login_a_source_a_value_or_the_submission() {
        let tekst = "
elements:
  - id: UI1
    fields:
      - {name: naam, visibility: asked, editable: true, fill: {from: 'r#5h#naam'}}
      - {name: nummer, visibility: shown, fill: {by: system}}
      - {name: elektronisch, visibility: hidden, fill: {by: system, from: 'r#6#langs_portaal'}}
      - {name: kanaal, visibility: hidden, fill: {by: system, value: portaal}}
      - {name: dagtekening, visibility: shown, fill: {by: system, submission: date}}
      - {name: gevraagd, visibility: shown, fill: {by: system, submission: requested_decision}}
      - {name: termijn, visibility: hidden}
";
        let spec = lees(tekst).unwrap();
        // A field the system fills is no prefill: the applicant does not change it.
        assert_eq!(
            spec.prefill,
            BTreeMap::from([("naam".to_string(), "r#5h#naam".to_string())])
        );
        assert_eq!(
            spec.systeem,
            BTreeMap::from([
                ("nummer".to_string(), Systeemvulling::Inloggen),
                (
                    "elektronisch".to_string(),
                    Systeemvulling::Bron {
                        bron: "r#6#langs_portaal".into(),
                        parameters: BTreeMap::new(),
                    }
                ),
                (
                    "kanaal".to_string(),
                    Systeemvulling::Waarde(serde_json::json!("portaal"))
                ),
                (
                    "dagtekening".to_string(),
                    Systeemvulling::Indiening(Indieningsgegeven::Datum)
                ),
                (
                    "gevraagd".to_string(),
                    Systeemvulling::Indiening(Indieningsgegeven::GevraagdBesluit)
                ),
            ])
        );
        assert_eq!(
            spec.verborgen,
            BTreeSet::from([
                "elektronisch".to_string(),
                "kanaal".to_string(),
                "termijn".to_string()
            ])
        );
    }

    #[test]
    fn the_system_fills_from_a_claim_of_the_login_and_a_source_on_the_submission() {
        let spec = lees(
            "elements:
              - id: UI1
                fields:
                  - {name: naam, visibility: shown, fill: {by: system, login: naam}}
                  - {name: middel, visibility: hidden, fill: {by: system, submission: login_method}}
                  - name: dagtekening
                    visibility: shown
                    fill:
                      by: system
                      from: 'b#9#dagtekening'
                      parameters: {datum_indiening: date, middel: login_method}",
        )
        .unwrap();
        assert_eq!(
            spec.systeem,
            BTreeMap::from([
                (
                    "dagtekening".to_string(),
                    Systeemvulling::Bron {
                        bron: "b#9#dagtekening".into(),
                        parameters: BTreeMap::from([
                            ("datum_indiening".to_string(), Indieningsgegeven::Datum),
                            ("middel".to_string(), Indieningsgegeven::Inlogmiddel),
                        ]),
                    }
                ),
                (
                    "middel".to_string(),
                    Systeemvulling::Indiening(Indieningsgegeven::Inlogmiddel)
                ),
                (
                    "naam".to_string(),
                    Systeemvulling::Inlogclaim("naam".into())
                ),
            ])
        );
        assert_eq!(Indieningsgegeven::Inlogmiddel.naam(), "login_method");
    }

    #[test]
    fn the_order_and_the_groups_of_the_fields_come_from_the_ui_spec() {
        let spec = lees(
            "elements:
              - id: UI1
                fields:
                  - {name: b, group: Uw gegevens}
                  - {name: a, group: Uw gegevens}
                  - {name: c}
              - id: UI2
                fields:
                  - {name: a}
                  - {name: d, group: Overig}",
        )
        .unwrap();
        assert_eq!(spec.volgorde, ["b", "a", "c", "d"]);
        assert_eq!(spec.teksten["a"].group.as_deref(), Some("Uw gegevens"));
        assert_eq!(spec.teksten["d"].group.as_deref(), Some("Overig"));
        assert!(!spec.teksten.contains_key("c"));
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, group: Een}]},
                        {id: UI2, fields: [{name: x, group: Twee}]}]",
        )
        .unwrap_err();
        assert!(
            e.contains("UI1 and UI2 give 'x' a different `group`"),
            "{e}"
        );
        assert!(lees("elementen: []").unwrap().volgorde.is_empty());
    }

    #[test]
    fn a_system_fill_that_does_not_read_is_refused() {
        let fout = |fill: &str| {
            lees(&format!(
                "elements: [{{id: UI1, fields: [{{name: x, fill: {fill}}}]}}]"
            ))
            .unwrap_err()
        };
        let e = fout("{by: system, from: 'r#6#a', value: 1}");
        assert!(e.contains("more than one of"), "{e}");
        let e = fout("{by: system, from: 'r#6'}");
        assert!(e.contains("which is no <regelwerk>"), "{e}");
        let e = fout("{by: system, submission: dag}");
        assert!(e.contains("`dag` of the submission"), "{e}");
        let e = fout("{value: portaal}");
        assert!(e.contains("without `by: system`"), "{e}");
        let e = fout("{login: naam}");
        assert!(e.contains("without `by: system`"), "{e}");
        let e = fout("{from: 'r#6#a', parameters: {d: date}}");
        assert!(e.contains("without `by: system`"), "{e}");
        let e = fout("{by: system, parameters: {d: date}}");
        assert!(e.contains("`parameters` without a source"), "{e}");
        let e = fout("{by: system, from: 'r#6#a', parameters: {d: dag}}");
        assert!(e.contains("`dag` of the submission"), "{e}");
        let e = fout("{by: system, login: naam, value: 1}");
        assert!(e.contains("more than one of"), "{e}");
        let e = fout("{by: system, login: ''}");
        assert!(e.contains("empty `login`"), "{e}");
        // A `by` the poc does not know is left to the register.
        assert!(lees("elements: [{id: UI1, fields: [{name: x, fill: {by: user}}]}]").is_ok());
        // `value: null` is an empty value, not the login.
        let spec =
            lees("elements: [{id: UI1, fields: [{name: x, fill: {by: system, value: null}}]}]")
                .unwrap();
        assert_eq!(
            spec.systeem["x"],
            Systeemvulling::Waarde(serde_json::Value::Null)
        );
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, visibility: asked, fill: {by: system}}]}]",
        )
        .unwrap_err();
        assert!(e.contains("UI1 asks 'x' and fills it by the system"), "{e}");
        // A view beside a decision is no form, but a misspelling there is refused all the same.
        let e = lees(
            "elements: [{id: UI8, fields: [{name: s, answer: {from: 'b#2#s', decision: d}},
                                          {name: x, fill: {by: system, submission: dag}}]}]",
        )
        .unwrap_err();
        assert!(e.contains("`dag` of the submission"), "{e}");
    }

    #[test]
    fn two_elements_that_disagree_on_a_field_of_the_form_are_refused() {
        let twee = |a: &str, b: &str| {
            lees(&format!(
                "elements: [{{id: UI1, fields: [{a}]}}, {{id: UI5, fields: [{b}]}}]"
            ))
            .unwrap_err()
        };
        let e = twee(
            "{name: x, fill: {by: system, value: 1}}",
            "{name: x, fill: {by: system, value: 2}}",
        );
        assert!(
            e.contains("UI1 and UI5 fill 'x' by the system in different ways"),
            "{e}"
        );
        let e = twee(
            "{name: x, fill: {by: system}}",
            "{name: x, fill: {from: 'r#5#x'}}",
        );
        assert!(
            e.contains("both prefills 'x' and fills it by the system"),
            "{e}"
        );
        let e = twee(
            "{name: x, fill: {by: system}}",
            "{name: x, visibility: asked}",
        );
        assert!(e.contains("UI5 asks 'x', which the system fills"), "{e}");
        let e = twee(
            "{name: x, visibility: hidden}",
            "{name: x, visibility: asked}",
        );
        assert!(
            e.contains("UI5 asks 'x', which another element hides"),
            "{e}"
        );
        // The same fill twice, as the form and the sheet that edits it, is one.
        let spec = lees(
            "elements: [{id: UI1, fields: [{name: x, fill: {by: system}}]},
                        {id: UI5, fields: [{name: x, fill: {by: system}}]}]",
        )
        .unwrap();
        assert_eq!(spec.systeem.len(), 1);
    }

    #[test]
    fn a_view_beside_a_decision_says_nothing_of_the_application_form() {
        let spec = lees(
            "elements: [{id: UI8, fields: [
                {name: s, answer: {from: 'b#2#s', decision: d}},
                {name: x, visibility: hidden, fill: {by: system}}]},
              {id: UI1, fields: [{name: x, visibility: asked}]}]",
        )
        .unwrap();
        assert!(spec.systeem.is_empty() && spec.verborgen.is_empty());
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
    fn a_field_may_say_how_a_screen_names_it_over_every_element() {
        let spec = lees(
            "elements:
              - id: UI1
                fields:
                  - {name: naam, label: Uw naam, fill: {from: 'r#5h#naam'}}
                  - {name: kenteken, help: Zoals op het kentekenbewijs}
              - id: UI2
                fields:
                  - {name: naam, help: Zoals in uw paspoort}
                  - {name: zonder}",
        )
        .unwrap();
        assert_eq!(
            spec.teksten,
            BTreeMap::from([
                (
                    "kenteken".to_string(),
                    Veldtekst {
                        label: None,
                        help: Some("Zoals op het kentekenbewijs".into()),
                        group: None,
                    }
                ),
                (
                    "naam".to_string(),
                    Veldtekst {
                        label: Some("Uw naam".into()),
                        help: Some("Zoals in uw paspoort".into()),
                        group: None,
                    }
                ),
            ])
        );
        // The older form has no fields, so no texts.
        assert!(lees("elementen: []").unwrap().teksten.is_empty());
    }

    #[test]
    fn two_elements_that_name_a_field_differently_are_refused() {
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, label: Een}]},
                        {id: UI2, fields: [{name: x, label: Twee}]}]",
        )
        .unwrap_err();
        assert!(
            e.contains("UI1 and UI2 give 'x' a different `label`"),
            "{e}"
        );
        // The element named is the one that gave the label, not one that gave only help.
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, help: Hulp}]},
                        {id: UI2, fields: [{name: x, label: Een}]},
                        {id: UI3, fields: [{name: x, label: Twee}]}]",
        )
        .unwrap_err();
        assert!(
            e.contains("UI2 and UI3 give 'x' a different `label`"),
            "{e}"
        );
        let e = lees("elements: [{id: UI1, fields: [{label: Een}]}]").unwrap_err();
        assert!(
            e.contains("UI1 gives a `label`, `help` or `group` to a field without a `name`"),
            "{e}"
        );
        // The same text twice is one.
        let spec = lees(
            "elements: [{id: UI1, fields: [{name: x, label: Een}]},
                        {id: UI2, fields: [{name: x, label: Een}]}]",
        )
        .unwrap();
        assert_eq!(spec.teksten["x"].label.as_deref(), Some("Een"));
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
        let e = lees(
            "elements: [{id: UI8, fields: [{name: s, answer: {from: 'b#2#s', decision: ''}}]}]",
        )
        .unwrap_err();
        assert!(e.contains("without the parameter"), "{e}");
        let twee = "elements: [{id: A, element: x, fields: [{name: s, answer: {from: 'b#2#s', decision: d}}]},
                               {id: B, element: x, fields: [{name: t, answer: {from: 'b#2#t', decision: d}}]}]";
        assert!(lees(twee).unwrap_err().contains("two elements"));
    }
}
