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
//! the group it stands in ([`Veldtekst`]). Those are the texts of the form,
//! which speaks to whoever fills it in. A screen that shows a recorded
//! submission speaks to someone else, or about the past: the applicant's own
//! dossier, the authority's work queue and chronicle. Under `record` a field
//! gives the same three texts for those screens ([`Stuktekst`]); a key it
//! leaves out falls back on the form's. `summary: true` there puts the field
//! among the few such a screen shows first, before the rest of the record.
//! `when_true` and `when_false` there name a yes-or-no value in words, such
//! as "aangevraagd" for a card that was asked for, where a bare "ja" would
//! leave the reader to work out what it answers. The same two keys on the
//! field itself name the answers on a form, such as the findings a decider
//! gives ("Onderzoek nodig" rather than "nee"); a record falls back on them. Without them a screen names the
//! field after the regelwerk. They count by field name over every element
//! and every form, so the same field is named the same on the application,
//! the objection and the decision; two elements that name one field
//! differently are refused. The order the fields are named in, over the
//! elements, is the order a screen shows them in ([`UiSpec::volgorde`]); a
//! field the UI spec does not name follows, in the order of the law.
//!
//! A field the applicant types may carry `required: true`: the form does not
//! submit without it. That is the screen's own demand, not the law's. A law
//! that lets a field be empty does so because a submission can be incomplete
//! (on paper you can forget the address, and Awb 4:5 says what follows), and
//! the regelwerk keeps that. A screen tells the applicant at once what is
//! missing, so it need not take an incomplete one in. A field the system
//! fills or the form hides has nothing for the applicant to give, so
//! `required` on it is refused. It counts over every element, as `parts` does.
//!
//! A message to the citizen is no element of the UI spec: the message spec
//! beside it says which message follows from which record
//! ([`crate::berichten`]). An element with `letter`, the form of before, is
//! refused, so a letter element does not stand unread.
//!
//! An element may also show a table: `table: {rows: {field: <name>}}` (an
//! array of records the submission holds), `{outcome: <name>}` (an array its
//! article gives on the records as they are now) or `{fact:
//! <regelwerk>#<article>#<fact>}` (every record of that fact), with `columns`
//! (the keys of a row, each with a `label`) and `provenance` (the records of
//! a fact of another cell a row comes from, matched on the keys of `match`),
//! and the element's `role` ([`Tabel`]). A screen shows it beside the
//! submission, with per row where it comes from: which cell, which record,
//! on which day.
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
    /// The letter element of before ([`crate::berichten`] took its place);
    /// present here, it is refused.
    letter: Option<IgnoredAny>,
    /// Set on an element that shows a table of records ([`Tabel`]).
    table: Option<TableOf>,
    /// Who sees the element, as the register names the role.
    role: Option<String>,
    /// The role of the generic element this one fills in, set when the
    /// layers merge ([`lagen`]): a screen tells the citizen's side by it.
    generic_role: Option<String>,
    /// The generic element this one fills in (`builds_on`); merged before
    /// anything else reads it ([`lagen`]).
    builds_on: Option<String>,
    /// The parts an element adds to its generic element, each with the
    /// fields it shows and the facts whose records it shows beside them.
    #[serde(default)]
    adds: Vec<Add>,
    /// The facts whose records the element shows as a table.
    #[serde(default)]
    facts: Vec<String>,
}

/// A part an element adds (`adds`): its name, its place, the fields of the
/// element in it and the facts whose records it shows beside them. Other
/// keys, such as its `basis`, are left to the register.
#[derive(Debug, Deserialize)]
struct Add {
    part: Option<String>,
    place: Option<String>,
    #[serde(default)]
    fields: Vec<String>,
    #[serde(default)]
    facts: Vec<String>,
}

/// Where the rows of a table element come from (`table.rows`): one of
/// `field` (an array of records held by the submission, as a field or an
/// output), `outcome` (an array the submission's article gives on the
/// records as they are now) or `fact` (every record of a fact,
/// `<regelwerk>#<article>#<fact>`). An unknown key is refused, as in
/// [`Fill`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowsOf {
    field: Option<String>,
    outcome: Option<String>,
    fact: Option<String>,
}

/// A table element (`table`): its rows, its columns, and the records of
/// other cells a row comes from. An unknown key is refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableOf {
    rows: RowsOf,
    #[serde(default)]
    columns: Vec<TableColumn>,
    #[serde(default)]
    provenance: Vec<TableProvenance>,
    label: Option<String>,
    help: Option<String>,
}

/// A column of a table element: the key in a row, and its heading.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableColumn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Where a row of a table comes from besides the record that holds it: the
/// records of `fact` whose fields equal the row on the keys of `match`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableProvenance {
    pub fact: String,
    #[serde(rename = "match", default)]
    pub on: Vec<String>,
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
    when_true: Option<String>,
    when_false: Option<String>,
    record: Option<Record>,
    parts: Option<Vec<Part>>,
    /// How many lines a text field shows ([`Veldtekst::lines`]).
    lines: Option<u8>,
    #[serde(default)]
    required: bool,
    editable: Option<bool>,
    to_build: Option<IgnoredAny>,
}

/// The texts of a field on a screen that shows a recorded submission. An
/// unknown key is refused, as in [`Fill`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    label: Option<String>,
    help: Option<String>,
    group: Option<String>,
    #[serde(default)]
    summary: bool,
    when_true: Option<String>,
    when_false: Option<String>,
}

/// One input of a field the applicant answers in parts: the law asks one
/// value, the screen asks it in pieces. An unknown key is refused, as in
/// [`Fill`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Part {
    name: Option<String>,
    label: Option<String>,
    help: Option<String>,
    autocomplete: Option<String>,
    keyboard: Option<String>,
    width: Option<String>,
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
    /// Per field that the screen asks in parts, those parts in order.
    pub delen: BTreeMap<String, Vec<Velddeel>>,
    /// The fields the form does not submit without (`required`).
    pub verplicht: BTreeSet<String>,
    /// The tables a screen shows beside a submission ([`Tabel`]), in the
    /// order of the UI spec.
    pub tabellen: Vec<Tabel>,
    /// Every element as the UI spec has it after the layers are merged
    /// ([`Overzicht`]): who sees it, its fields and its parts.
    pub elementen: Vec<Overzicht>,
}

/// An element of the UI spec as a screen lists it per role: its id, its name,
/// the role that sees it, the generic element it fills in, its fields (each
/// with what the UI spec says of how it is shown and filled) and the parts it
/// adds.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Overzicht {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bouwt_op: Option<String>,
    /// The role of the generic element it fills in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol_generiek: Option<String>,
    pub velden: Vec<Overzichtveld>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub delen: Vec<Overzichtdeel>,
}

/// A field of an [`Overzicht`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Overzichtveld {
    pub naam: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zichtbaarheid: Option<String>,
    /// `editable` of the UI spec: whether the user may change it here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wijzigbaar: Option<bool>,
    /// Whether the system fills it (`by: system`), prefills it (`from`
    /// alone) or neither.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vulling: Option<String>,
    /// Set when the model does not carry the name yet (`to_build`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub te_bouwen: bool,
}

/// A part an element adds, in an [`Overzicht`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Overzichtdeel {
    pub naam: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plek: Option<String>,
    pub velden: Vec<String>,
    pub feiten: Vec<String>,
}

/// An element that shows a table (`table` in the UI spec): rows of records,
/// a column per key, and per row where it comes from. The rows come from an
/// array the submission holds (`veld`), from an array its article gives
/// (`uitkomst`), or from every record of a fact (`feit`); `herkomst` names
/// the records of other cells a row comes from, matched on keys of the row.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Tabel {
    /// The element's id, or its name: how a screen asks for it.
    pub naam: String,
    /// The element's name in the register.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// Who sees it, as the register names the role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol: Option<String>,
    /// The role of the generic element its element fills in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol_generiek: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veld: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uitkomst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feit: Option<String>,
    pub kolommen: Vec<TableColumn>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub herkomst: Vec<TableProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
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
    /// The answer `true` in words on a form, such as "onderzoek achterwege".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_true: Option<String>,
    /// The answer `false` in words on a form, such as "onderzoek nodig".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_false: Option<String>,
    /// The texts on a screen that shows a recorded submission, where they differ.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<Stuktekst>,
    /// How many lines a text field shows when empty: a field for a few
    /// sentences, such as the content of a letter, says so before anyone
    /// types. Without it the field is one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<u8>,
}

/// How a screen that shows a recorded submission names a field, where that
/// differs from the form: the dossier, the work queue and the chronicle speak
/// about the submission, not to the one filling it in.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Stuktekst {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Whether the screen shows the field first, among the few that say what
    /// the submission is about.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub summary: bool,
    /// The value `true` in words, such as "aangevraagd".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_true: Option<String>,
    /// The value `false` in words, such as "niet aangevraagd".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when_false: Option<String>,
}

/// One input of a field the screen asks in parts. The law asks one value, so
/// the form joins what the applicant types here, in this order, with a comma
/// between: an address is one value of the law and three questions on screen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Velddeel {
    pub naam: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// The browser's autofill hint, such as `postal-code`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocomplete: Option<String>,
    /// The on-screen keyboard a phone raises, such as `numeric`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<String>,
    /// How wide the input is, as a CSS length: a field as wide as what it takes,
    /// such as a postcode, says what goes in it before anyone types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
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
        serde_yaml_ng::from_value(lagen(tekst)?).map_err(|e| format!("the UI spec: {e}"))?;
    let elementen = register
        .elements
        .as_deref()
        .map(overzicht)
        .unwrap_or_default();
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
    let delen = register
        .elements
        .as_deref()
        .map(delen)
        .transpose()?
        .unwrap_or_default();
    let verplicht = register
        .elements
        .as_deref()
        .map(verplicht)
        .transpose()?
        .unwrap_or_default();
    let tabellen = register
        .elements
        .as_deref()
        .map(tabellen)
        .transpose()?
        .unwrap_or_default();
    if let Some((i, e)) = register
        .elements
        .iter()
        .flatten()
        .enumerate()
        .find(|(_, e)| e.letter.is_some())
    {
        return Err(format!(
            "the UI spec: {} shows a record as a letter; a message is a kind of message \
             in the message spec beside the UI spec (`berichten`), not an element",
            element_name(e.id.as_ref(), i)
        ));
    }
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
    if let Some(veld) = delen.keys().find(|v| systeem.contains_key(*v)) {
        return Err(format!(
            "the UI spec asks '{veld}' in parts and fills it by the system"
        ));
    }
    if let Some(veld) = delen.keys().find(|v| verborgen.contains(*v)) {
        return Err(format!("the UI spec asks '{veld}' in parts and hides it"));
    }
    if let Some(veld) = verplicht.iter().find(|v| systeem.contains_key(*v)) {
        return Err(format!(
            "the UI spec requires '{veld}' and fills it by the system; \
             the applicant cannot give what the system fills"
        ));
    }
    if let Some(veld) = verplicht.iter().find(|v| verborgen.contains(*v)) {
        return Err(format!(
            "the UI spec requires '{veld}' and hides it; \
             the applicant cannot give what the form does not show"
        ));
    }
    Ok(UiSpec {
        prefill,
        systeem,
        verborgen,
        weergaven,
        teksten,
        volgorde,
        delen,
        verplicht,
        tabellen,
        elementen,
    })
}

/// The UI spec in `tekst` as one register: every YAML document in it (a
/// layered UI spec is the generic layer, then the layer of one executor, as
/// separate documents), and every element that fills in another
/// (`builds_on`) merged with it. The merged element takes the generic
/// element's fields and outcomes by name (its own keys of a field go over the
/// generic ones, a field of its own follows them) and every other key it does
/// not give itself, such as the pattern and the parts; the generic element
/// itself then drops out. A generic element nobody fills in stays as it is.
/// One document without `builds_on` reads as before.
fn lagen(tekst: &str) -> Result<Value, String> {
    use serde::Deserialize as _;
    let mut documenten = Vec::new();
    for doc in serde_yaml_ng::Deserializer::from_str(tekst) {
        let v = Value::deserialize(doc).map_err(|e| format!("the UI spec: {e}"))?;
        if !v.is_null() {
            documenten.push(v);
        }
    }
    if documenten.len() == 1 {
        let enige = documenten.remove(0);
        let bouwt = enige
            .get("elements")
            .and_then(Value::as_sequence)
            .is_some_and(|es| es.iter().any(|e| e.get("builds_on").is_some()));
        if !bouwt {
            return Ok(enige);
        }
        documenten.push(enige);
    }
    let mut elementen: Vec<Value> = Vec::new();
    let mut oud = None;
    for doc in documenten {
        if let Some(es) = doc.get("elements").and_then(Value::as_sequence) {
            elementen.extend(es.iter().cloned());
        }
        if let Some(o) = doc.get("elementen") {
            oud = Some(o.clone());
        }
    }
    if let Some(oud) = oud {
        if !elementen.is_empty() {
            return Err("the UI spec has both `elements` and `elementen`; keep one form".into());
        }
        let mut m = serde_yaml_ng::Mapping::new();
        m.insert("elementen".into(), oud);
        return Ok(Value::Mapping(m));
    }
    let id = |e: &Value| e.get("id").and_then(Value::as_str).map(str::to_string);
    let opgebouwd: BTreeSet<String> = elementen
        .iter()
        .filter_map(|e| e.get("builds_on").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    let generiek: BTreeMap<String, Value> = elementen
        .iter()
        .filter_map(|e| {
            id(e)
                .filter(|i| opgebouwd.contains(i))
                .map(|i| (i, e.clone()))
        })
        .collect();
    let mut uit = Vec::new();
    for e in &elementen {
        let eigen = id(e);
        let bouwt_op = e.get("builds_on").and_then(Value::as_str);
        if eigen.as_ref().is_some_and(|i| opgebouwd.contains(i)) && bouwt_op.is_none() {
            continue;
        }
        match bouwt_op.and_then(|g| generiek.get(g)) {
            Some(g) => uit.push(samengevoegd(g, e)),
            None => uit.push(e.clone()),
        }
    }
    let mut m = serde_yaml_ng::Mapping::new();
    m.insert("elements".into(), Value::Sequence(uit));
    Ok(Value::Mapping(m))
}

/// The element `eigen` merged with the generic element `g` it builds on
/// ([`lagen`]).
fn samengevoegd(g: &Value, eigen: &Value) -> Value {
    let (Some(g), Some(e)) = (g.as_mapping(), eigen.as_mapping()) else {
        return eigen.clone();
    };
    let mut uit = e.clone();
    if let Some(rol) = g.get("role") {
        uit.insert("generic_role".into(), rol.clone());
    }
    for (k, v) in g {
        let sleutel = k.as_str().unwrap_or_default();
        if ["id", "element", "text"].contains(&sleutel) {
            continue;
        }
        if ["fields", "outcomes"].contains(&sleutel) {
            uit.insert(k.clone(), per_naam(v, e.get(k)));
        } else if !uit.contains_key(k) {
            uit.insert(k.clone(), v.clone());
        }
    }
    Value::Mapping(uit)
}

/// Two lists of entries with a `name`, the second refining the first by name.
fn per_naam(eerst: &Value, dan: Option<&Value>) -> Value {
    let mut uit: Vec<Value> = eerst.as_sequence().cloned().unwrap_or_default();
    for d in dan.and_then(Value::as_sequence).into_iter().flatten() {
        let naam = d.get("name");
        match uit
            .iter_mut()
            .find(|u| naam.is_some() && u.get("name") == naam)
        {
            Some(u) => {
                if let (Some(u), Some(d)) = (u.as_mapping_mut(), d.as_mapping()) {
                    for (k, v) in d {
                        u.insert(k.clone(), v.clone());
                    }
                }
            }
            None => uit.push(d.clone()),
        }
    }
    Value::Sequence(uit)
}

/// Every element as a screen lists it per role ([`Overzicht`]).
fn overzicht(elements: &[Element]) -> Vec<Overzicht> {
    elements
        .iter()
        .enumerate()
        .map(|(i, e)| Overzicht {
            id: element_name(e.id.as_ref(), i),
            element: e.element.clone(),
            rol: e.role.clone(),
            bouwt_op: e.builds_on.clone(),
            rol_generiek: e.generic_role.clone(),
            velden: e
                .fields
                .iter()
                .filter_map(|f| {
                    Some(Overzichtveld {
                        naam: f.name.clone()?,
                        zichtbaarheid: f.visibility.clone(),
                        wijzigbaar: f.editable,
                        vulling: f.fill.as_ref().map(|v| {
                            if v.by.is_some() {
                                "systeem"
                            } else {
                                "voorinvulling"
                            }
                            .to_string()
                        }),
                        te_bouwen: f.to_build.is_some(),
                    })
                })
                .collect(),
            delen: e
                .adds
                .iter()
                .map(|a| Overzichtdeel {
                    naam: a.part.clone().unwrap_or_default(),
                    plek: a.place.clone(),
                    velden: a.fields.clone(),
                    feiten: a.facts.clone(),
                })
                .collect(),
        })
        .collect()
}

/// The tables of `elements`: every element with `table`. A table whose rows
/// name not exactly one source, a fact that is no
/// `<regelwerk>#<article>#<fact>`, and two tables of one name are refused.
fn tabellen(elements: &[Element]) -> Result<Vec<Tabel>, String> {
    let mut uit: Vec<Tabel> = Vec::new();
    for (i, element) in elements.iter().enumerate() {
        let Some(table) = &element.table else {
            uit.extend(afgeleid(element, i));
            continue;
        };
        let naam = element_name(element.id.as_ref(), i);
        let rows = &table.rows;
        let bronnen = [&rows.field, &rows.outcome, &rows.fact]
            .iter()
            .filter(|b| b.as_deref().is_some_and(|b| !b.is_empty()))
            .count();
        if bronnen != 1 {
            return Err(format!(
                "the UI spec: the table '{naam}' takes its rows from {bronnen} sources; \
                 name one of `field`, `outcome` and `fact`"
            ));
        }
        let feiten = rows
            .fact
            .iter()
            .chain(table.provenance.iter().map(|h| &h.fact));
        for feit in feiten {
            if feit.split('#').count() != 3 || feit.split('#').any(str::is_empty) {
                return Err(format!(
                    "the UI spec: the table '{naam}' names the fact '{feit}', which is no \
                     <regelwerk>#<article>#<fact>"
                ));
            }
        }
        if uit.iter().any(|t| t.naam == naam) {
            return Err(format!(
                "the UI spec: two tables are called '{naam}'; give each its own id"
            ));
        }
        uit.push(Tabel {
            naam,
            element: element.element.clone(),
            rol: element.role.clone(),
            rol_generiek: element.generic_role.clone(),
            veld: rows.field.clone(),
            uitkomst: rows.outcome.clone(),
            feit: rows.fact.clone(),
            kolommen: table.columns.clone(),
            herkomst: table.provenance.clone(),
            label: table.label.clone(),
            help: table.help.clone(),
        });
    }
    Ok(uit)
}

/// The tables an element without `table` implies ([`Tabel`]): per part it
/// adds that names both fields and facts, a table per field (its rows the
/// array the submission holds there) with those facts as the provenance of a
/// row, matched on the keys a row and a record share; and per fact the
/// element names itself, a table of the records of that fact that belong to
/// the submission (see `table.rs` of RegelHet).
fn afgeleid(element: &Element, i: usize) -> Vec<Tabel> {
    let id = element_name(element.id.as_ref(), i);
    let basis = |naam: String, part: Option<&str>| Tabel {
        naam,
        element: part.map(str::to_string).or_else(|| element.element.clone()),
        rol: element.role.clone(),
        rol_generiek: element.generic_role.clone(),
        veld: None,
        uitkomst: None,
        feit: None,
        kolommen: Vec::new(),
        herkomst: Vec::new(),
        label: None,
        help: None,
    };
    let mut uit = Vec::new();
    for a in element
        .adds
        .iter()
        .filter(|a| !a.fields.is_empty() && !a.facts.is_empty())
    {
        for veld in &a.fields {
            let mut t = basis(format!("{id}.{veld}"), a.part.as_deref());
            t.veld = Some(veld.clone());
            t.herkomst = a
                .facts
                .iter()
                .map(|f| TableProvenance {
                    fact: f.clone(),
                    on: Vec::new(),
                })
                .collect();
            uit.push(t);
        }
    }
    for feit in &element.facts {
        let mut t = basis(format!("{id}.{feit}"), None);
        t.feit = Some(feit.clone());
        uit.push(t);
    }
    uit
}

/// The fields an element requires (`required: true`), over every element.
fn verplicht(elements: &[Element]) -> Result<BTreeSet<String>, String> {
    let mut uit = BTreeSet::new();
    for (i, element) in elements.iter().enumerate() {
        for field in element.fields.iter().filter(|f| f.required) {
            let Some(veld) = field.name.as_deref().filter(|v| !v.is_empty()) else {
                return Err(format!(
                    "the UI spec: {} requires a field without a `name`",
                    element_name(element.id.as_ref(), i)
                ));
            };
            uit.insert(veld.to_owned());
        }
    }
    Ok(uit)
}

/// Per field that an element asks in parts, those parts in order. Two
/// elements that give one field different parts are refused, as they are for
/// a `label`.
fn delen(elements: &[Element]) -> Result<BTreeMap<String, Vec<Velddeel>>, String> {
    let mut uit: BTreeMap<String, (Vec<Velddeel>, String)> = BTreeMap::new();
    for (i, element) in elements.iter().enumerate() {
        let naam = element_name(element.id.as_ref(), i);
        for field in &element.fields {
            let Some(parts) = &field.parts else { continue };
            let Some(veld) = field.name.as_deref().filter(|v| !v.is_empty()) else {
                return Err(format!(
                    "the UI spec: {naam} gives `parts` to a field without a `name`"
                ));
            };
            if parts.is_empty() {
                return Err(format!(
                    "the UI spec: {naam} gives '{veld}' an empty `parts`; leave it out, \
                     or name the inputs the applicant answers it in"
                ));
            }
            if field.fill.is_some() {
                return Err(format!(
                    "the UI spec: {naam} gives '{veld}' `parts` and fills it; the applicant \
                     does not type what the system fills"
                ));
            }
            if field.visibility.as_deref() == Some("hidden") {
                return Err(format!(
                    "the UI spec: {naam} gives '{veld}' `parts` and hides it; a field that is \
                     not on the form has no inputs"
                ));
            }
            let mut gezien: BTreeSet<&str> = BTreeSet::new();
            let mut deze = Vec::with_capacity(parts.len());
            for part in parts {
                let Some(deel) = part.name.as_deref().filter(|d| !d.is_empty()) else {
                    return Err(format!(
                        "the UI spec: {naam} gives '{veld}' a part without a `name`"
                    ));
                };
                if !gezien.insert(deel) {
                    return Err(format!(
                        "the UI spec: {naam} gives '{veld}' the part '{deel}' twice"
                    ));
                }
                deze.push(Velddeel {
                    naam: deel.to_owned(),
                    label: part.label.clone(),
                    help: part.help.clone(),
                    autocomplete: part.autocomplete.clone(),
                    keyboard: part.keyboard.clone(),
                    width: part.width.clone(),
                });
            }
            match uit.get(veld) {
                Some((eerder, door)) if *eerder != deze => {
                    return Err(format!(
                        "the UI spec: {door} and {naam} give '{veld}' different `parts`"
                    ));
                }
                Some(_) => {}
                None => {
                    uit.insert(veld.to_owned(), (deze, naam.clone()));
                }
            }
        }
    }
    Ok(uit.into_iter().map(|(v, (d, _))| (v, d)).collect())
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

/// Per field the `label`, `help` and `group` the elements give it, and
/// whether one of them puts it in the summary of a record. Two elements that
/// give one field a different label, help or group are refused.
fn teksten(elements: &[Element]) -> Result<BTreeMap<String, Veldtekst>, String> {
    // Per field and per key (`label`, `help`, `group`, and each of them under
    // `record`): the text, and the element that gave it.
    let mut gegeven: BTreeMap<(String, &'static str), (String, String)> = BTreeMap::new();
    let mut samenvatting: BTreeSet<String> = BTreeSet::new();
    // Per field the number of lines, and the element that gave it.
    let mut regels: BTreeMap<String, (u8, String)> = BTreeMap::new();
    for (i, element) in elements.iter().enumerate() {
        let naam = element_name(element.id.as_ref(), i);
        for field in &element.fields {
            let record = field.record.as_ref();
            let teksten = [
                ("label", field.label.as_ref()),
                ("help", field.help.as_ref()),
                ("group", field.group.as_ref()),
                ("when_true", field.when_true.as_ref()),
                ("when_false", field.when_false.as_ref()),
                ("record.label", record.and_then(|r| r.label.as_ref())),
                ("record.help", record.and_then(|r| r.help.as_ref())),
                ("record.group", record.and_then(|r| r.group.as_ref())),
                (
                    "record.when_true",
                    record.and_then(|r| r.when_true.as_ref()),
                ),
                (
                    "record.when_false",
                    record.and_then(|r| r.when_false.as_ref()),
                ),
            ];
            let in_samenvatting = record.is_some_and(|r| r.summary);
            if teksten.iter().all(|(_, t)| t.is_none()) && !in_samenvatting && field.lines.is_none()
            {
                continue;
            }
            let Some(veld) = field.name.as_deref().filter(|v| !v.is_empty()) else {
                return Err(format!(
                    "the UI spec: {naam} gives a `label`, `help`, `group`, `when_true`, `when_false`, `record` or `lines` to a field without a `name`"
                ));
            };
            if in_samenvatting {
                samenvatting.insert(veld.to_string());
            }
            if let Some(n) = field.lines {
                if n < 2 {
                    return Err(format!(
                        "the UI spec: {naam} gives '{veld}' `lines: {n}`; a field without `lines` is one line already"
                    ));
                }
                match regels.get(veld) {
                    Some((eerder, door)) if *eerder != n => {
                        return Err(format!(
                            "the UI spec: {door} and {naam} give '{veld}' a different `lines`"
                        ));
                    }
                    Some(_) => {}
                    None => {
                        regels.insert(veld.to_string(), (n, naam.clone()));
                    }
                }
            }
            for (wat, tekst) in teksten {
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
            "group" => t.group = Some(tekst),
            "when_true" => t.when_true = Some(tekst),
            "when_false" => t.when_false = Some(tekst),
            "record.label" => t.record.get_or_insert_default().label = Some(tekst),
            "record.help" => t.record.get_or_insert_default().help = Some(tekst),
            "record.when_true" => t.record.get_or_insert_default().when_true = Some(tekst),
            "record.when_false" => t.record.get_or_insert_default().when_false = Some(tekst),
            _ => t.record.get_or_insert_default().group = Some(tekst),
        }
    }
    for veld in samenvatting {
        uit.entry(veld)
            .or_default()
            .record
            .get_or_insert_default()
            .summary = true;
    }
    for (veld, (n, _)) in regels {
        uit.entry(veld).or_default().lines = Some(n);
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

    /// A generic layer and the layer of one executor that fills it in, as two
    /// documents: the elements merge, and the tables follow from the parts.
    const GELAAGD: &str = r#"
status: concept
elements:
  - id: G1
    element: aanvraag
    role: aanvrager
    pattern: {name: form}
    fields:
      - {name: naam_aanvrager}
      - {name: adres_aanvrager}
  - id: G2
    element: ontvangst
    role: aanvrager
    fields: [{name: datum_ontvangst}]
---
elements:
  - id: E1
    element: aanvraagformulier
    builds_on: G1
    role: deelnemer
    basis: {policy: [{article: '3'}]}
    fields:
      - name: naam_aanvrager
        visibility: asked
        editable: true
        fill: {from: 'register#2#naam'}
        fill_as: suggestion
      - {name: plaatsen, visibility: shown, editable: false, fill: {by: system, from: 'regeling#4#plaatsen_per_wijk'}}
      - {name: nog_te_bouwen, to_build: {kind: outcome, article: 'regeling#4', reason: 'later'}}
    adds:
      - {part: plaatsen_per_wijk, place: gegevens, fields: [plaatsen], facts: ['telling#3#telling_ontvangen']}
    outcomes: [{name: aanvraag_volledig}]
  - id: E2
    element: telling_per_wijk
    role: behandelaar
    facts: ['telling#3#telling_ontvangen']
"#;

    #[test]
    fn a_layered_ui_spec_merges_an_element_with_the_one_it_builds_on() {
        let spec = lees(GELAAGD).unwrap();
        let ids: Vec<&str> = spec.elementen.iter().map(|e| e.id.as_str()).collect();
        // G1 is filled in by E1 and drops out; G2 nobody fills in, so it stays.
        assert_eq!(ids, ["G2", "E1", "E2"]);
        let e1 = &spec.elementen[1];
        assert_eq!(e1.rol.as_deref(), Some("deelnemer"));
        assert_eq!(e1.bouwt_op.as_deref(), Some("G1"));
        assert_eq!(e1.rol_generiek.as_deref(), Some("aanvrager"));
        let velden: Vec<&str> = e1.velden.iter().map(|v| v.naam.as_str()).collect();
        assert_eq!(
            velden,
            [
                "naam_aanvrager",
                "adres_aanvrager",
                "plaatsen",
                "nog_te_bouwen"
            ]
        );
        assert_eq!(e1.velden[0].vulling.as_deref(), Some("voorinvulling"));
        assert_eq!(e1.velden[0].wijzigbaar, Some(true));
        assert!(e1.velden[3].te_bouwen);
        // The prefill and the system fill read as before.
        assert_eq!(spec.prefill["naam_aanvrager"], "register#2#naam");
        assert!(spec.systeem.contains_key("plaatsen"));
        assert_eq!(e1.delen[0].feiten, ["telling#3#telling_ontvangen"]);
    }

    #[test]
    fn a_part_with_fields_and_facts_and_an_element_with_facts_are_tables() {
        let spec = lees(GELAAGD).unwrap();
        let namen: Vec<&str> = spec.tabellen.iter().map(|t| t.naam.as_str()).collect();
        assert_eq!(namen, ["E1.plaatsen", "E2.telling#3#telling_ontvangen"]);
        let veld = &spec.tabellen[0];
        assert_eq!(veld.veld.as_deref(), Some("plaatsen"));
        assert_eq!(veld.rol.as_deref(), Some("deelnemer"));
        assert_eq!(
            veld.rol_generiek.as_deref(),
            Some("aanvrager"),
            "the role of the generic element tells the side"
        );
        assert_eq!(veld.herkomst[0].fact, "telling#3#telling_ontvangen");
        assert!(
            veld.herkomst[0].on.is_empty(),
            "matched on the keys a row and a record share"
        );
        let feit = &spec.tabellen[1];
        assert_eq!(feit.feit.as_deref(), Some("telling#3#telling_ontvangen"));
        assert_eq!(feit.rol.as_deref(), Some("behandelaar"));
    }

    #[test]
    fn one_document_without_builds_on_reads_as_before() {
        let een = "elements:\n  - id: UI1\n    fields:\n      - {name: a, fill: {from: 'r#1#a'}}\n";
        let spec = lees(een).unwrap();
        assert_eq!(spec.prefill["a"], "r#1#a");
        assert_eq!(spec.elementen.len(), 1);
        assert!(spec.tabellen.is_empty());
    }

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
                        record: None,
                        ..Veldtekst::default()
                    }
                ),
                (
                    "naam".to_string(),
                    Veldtekst {
                        label: Some("Uw naam".into()),
                        help: Some("Zoals in uw paspoort".into()),
                        group: None,
                        record: None,
                        ..Veldtekst::default()
                    }
                ),
            ])
        );
        // The older form has no fields, so no texts.
        assert!(lees("elementen: []").unwrap().teksten.is_empty());
    }

    #[test]
    fn a_field_may_say_how_many_lines_it_shows() {
        let spec = lees(
            "elements:
              - id: UI1
                fields: [{name: inhoud, lines: 6}]
              - id: UI2
                fields: [{name: inhoud, label: Inhoud, lines: 6}]",
        )
        .unwrap();
        let t = &spec.teksten["inhoud"];
        assert_eq!(t.lines, Some(6));
        assert_eq!(t.label.as_deref(), Some("Inhoud"));
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, lines: 3}]},
                        {id: UI2, fields: [{name: x, lines: 4}]}]",
        )
        .unwrap_err();
        assert!(
            e.contains("UI1 and UI2 give 'x' a different `lines`"),
            "{e}"
        );
        let e = lees("elements: [{id: UI1, fields: [{name: x, lines: 1}]}]").unwrap_err();
        assert!(e.contains("UI1 gives 'x' `lines: 1`"), "{e}");
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
            e.contains(
                "UI1 gives a `label`, `help`, `group`, `when_true`, `when_false`, `record` or `lines` to a field without a `name`"
            ),
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
    fn a_field_names_itself_otherwise_where_a_submission_is_shown() {
        let spec = lees(
            "
elements:
  - id: UI1
    fields:
      - name: x
        group: Vraag de kaart aan als
        label: Bestuurder
        record: {group: Gevraagde kaart, label: Kaart als bestuurder}
      - {name: y, label: Naam}
",
        )
        .unwrap();
        let x = &spec.teksten["x"];
        assert_eq!(x.label.as_deref(), Some("Bestuurder"));
        let record = x.record.as_ref().unwrap();
        assert_eq!(record.label.as_deref(), Some("Kaart als bestuurder"));
        assert_eq!(record.group.as_deref(), Some("Gevraagde kaart"));
        // What `record` leaves out, the screen takes from the form.
        assert_eq!(record.help, None);
        assert!(!record.summary);
        assert_eq!(spec.teksten["y"].record, None);
        // A field in the summary of a record says so, also without a text of its own there.
        let spec =
            lees("elements: [{id: UI1, fields: [{name: x, record: {summary: true}}, {name: y}]}]")
                .unwrap();
        assert!(spec.teksten["x"].record.as_ref().unwrap().summary);
        assert!(!spec.teksten.contains_key("y"));
        // A yes-or-no value has its own words on such a screen.
        let spec = lees(
            "elements: [{id: UI1, fields: [{name: x, record: {when_true: Aangevraagd, when_false: Niet aangevraagd}}]}]",
        )
        .unwrap();
        let record = spec.teksten["x"].record.as_ref().unwrap();
        assert_eq!(record.when_true.as_deref(), Some("Aangevraagd"));
        assert_eq!(record.when_false.as_deref(), Some("Niet aangevraagd"));
        // And on a form, on the field itself; two elements that word it differently are refused.
        let spec = lees(
            "elements: [{id: UI1, fields: [{name: x, when_true: Achterwege, when_false: Nodig}]}]",
        )
        .unwrap();
        assert_eq!(spec.teksten["x"].when_true.as_deref(), Some("Achterwege"));
        assert_eq!(spec.teksten["x"].when_false.as_deref(), Some("Nodig"));
        assert!(spec.teksten["x"].record.is_none());
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, when_true: Een}]},
                        {id: UI2, fields: [{name: x, when_true: Twee}]}]",
        )
        .unwrap_err();
        assert!(e.contains("`when_true`"), "{e}");
        // Two elements that name one field differently on such a screen are refused.
        let e = lees(
            "elements: [{id: UI1, fields: [{name: x, record: {label: Een}}]},
                        {id: UI2, fields: [{name: x, record: {label: Twee}}]}]",
        )
        .unwrap_err();
        assert!(
            e.contains("UI1 and UI2 give 'x' a different `record.label`"),
            "{e}"
        );
        let e =
            lees("elements: [{id: UI1, fields: [{name: x, record: {titel: Een}}]}]").unwrap_err();
        assert!(e.contains("titel"), "{e}");
        let e = lees("elements: [{id: UI1, fields: [{record: {label: Een}}]}]").unwrap_err();
        assert!(
            e.contains("`record` or `lines` to a field without a `name`"),
            "{e}"
        );
    }

    #[test]
    fn a_field_the_screen_asks_in_parts_names_its_inputs() {
        let spec = lees(
            "
elements:
  - id: UI1
    fields:
      - name: adres
        label: Adres
        parts:
          - {name: straat_huisnummer, label: Straat en huisnummer, autocomplete: street-address}
          - {name: postcode, label: Postcode, autocomplete: postal-code, keyboard: text, width: 12ch}
",
        )
        .unwrap();
        let delen = &spec.delen["adres"];
        assert_eq!(delen.len(), 2);
        assert_eq!(delen[0].naam, "straat_huisnummer");
        assert_eq!(delen[0].autocomplete.as_deref(), Some("street-address"));
        assert_eq!(delen[1].keyboard.as_deref(), Some("text"));
        assert_eq!(delen[1].width.as_deref(), Some("12ch"));
        // The field itself stays one field of the law; only the screen splits it.
        assert_eq!(spec.volgorde, ["adres"]);
        assert_eq!(spec.teksten["adres"].label.as_deref(), Some("Adres"));
    }

    #[test]
    fn parts_on_a_field_the_applicant_does_not_type_are_refused() {
        let gevuld = lees(
            "
elements:
  - id: UI1
    fields:
      - {name: adres, fill: {by: system}, parts: [{name: postcode}]}
",
        )
        .unwrap_err();
        assert!(
            gevuld.contains("gives 'adres' `parts` and fills it"),
            "{gevuld}"
        );
        let verborgen = lees(
            "
elements:
  - id: UI1
    fields:
      - {name: adres, visibility: hidden, parts: [{name: postcode}]}
",
        )
        .unwrap_err();
        assert!(
            verborgen.contains("gives 'adres' `parts` and hides it"),
            "{verborgen}"
        );
        let naamloos =
            lees("elements: [{id: UI1, fields: [{parts: [{name: postcode}]}]}]").unwrap_err();
        assert!(
            naamloos.contains("`parts` to a field without a `name`"),
            "{naamloos}"
        );
        let dubbel = lees(
            "elements: [{id: UI1, fields: [{name: adres, parts: [{name: postcode}, {name: postcode}]}]}]",
        )
        .unwrap_err();
        assert!(dubbel.contains("the part 'postcode' twice"), "{dubbel}");
    }

    #[test]
    fn a_field_the_form_requires_is_named_whatever_the_law_lets_be_empty() {
        let spec = lees(
            "
elements:
  - id: UI1
    fields:
      - {name: adres, required: true, parts: [{name: postcode}]}
      - {name: telefoon}
  - id: UI2
    fields:
      - {name: naam, required: true}
",
        )
        .unwrap();
        assert_eq!(
            spec.verplicht
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["adres", "naam"]
        );
    }

    #[test]
    fn a_required_field_the_applicant_does_not_give_is_refused() {
        let gevuld = lees(
            "elements: [{id: UI1, fields: [{name: naam, required: true, fill: {by: system}}]}]",
        )
        .unwrap_err();
        assert!(
            gevuld.contains("requires 'naam' and fills it by the system"),
            "{gevuld}"
        );
        // Required in one element, filled by the system in another: still refused.
        let elders = lees(
            "elements: [{id: UI1, fields: [{name: naam, required: true}]},
                        {id: UI2, fields: [{name: naam, fill: {by: system}}]}]",
        )
        .unwrap_err();
        assert!(elders.contains("requires 'naam'"), "{elders}");
        let verborgen = lees(
            "elements: [{id: UI1, fields: [{name: naam, required: true, visibility: hidden}]}]",
        )
        .unwrap_err();
        assert!(
            verborgen.contains("requires 'naam' and hides it"),
            "{verborgen}"
        );
        let naamloos = lees("elements: [{id: UI1, fields: [{required: true}]}]").unwrap_err();
        assert!(
            naamloos.contains("requires a field without a `name`"),
            "{naamloos}"
        );
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

    #[test]
    fn a_letter_element_is_refused_for_the_message_spec() {
        let e =
            lees("elements: [{id: B, letter: {of: decision}, parts: [{id: x, place: dictum}]}]")
                .unwrap_err();
        assert!(e.contains("in the message spec"), "{e}");
    }
}
