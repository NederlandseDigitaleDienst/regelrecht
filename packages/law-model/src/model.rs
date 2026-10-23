//! Article-based law document model.
//!
//! These structs/enums are the canonical Rust representation of the law-YAML
//! format (the files under `corpus/regulation/`). They carry only the document
//! shape and allocation-free accessors — no YAML loading, security limits or
//! evaluation logic. The engine owns loading (`LawLoad`) and execution and
//! re-exports these types at `regelrecht_engine::article`.
use crate::{Operation, ParameterType, RegulatoryLayer, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Represents a competent authority: a reference or one named authority on an
/// article (RFC-002), or, on an open term or a fact (schema v0.8.8, RFC-051),
/// one named authority or a list of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompetentAuthority {
    /// Simple string reference (e.g., "#bevoegd_gezag")
    String(String),
    /// One named authority
    Structured(NamedAuthority),
    /// Several named authorities, each competent, on an open term or a
    /// fact: whether a parking card was issued before is for any college
    /// and for the CBR (BABW art. 49 lid 1 and 3).
    List(Vec<NamedAuthority>),
}

/// An authority the law names: an organisation or a category of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedAuthority {
    pub name: String,
    /// `INSTANCE` (a named organisation, the schema default) or `CATEGORY`
    /// (must be resolved per context)
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub authority_type: Option<AuthorityType>,
}

impl NamedAuthority {
    /// The type, `INSTANCE` when the law leaves it out (the schema default).
    pub fn kind(&self) -> AuthorityType {
        self.authority_type.unwrap_or(AuthorityType::Instance)
    }
}

impl CompetentAuthority {
    /// The authorities this names, in the law's order. `None` for a
    /// reference to an output (`#...`) or a bare name, which name no
    /// authority with a type.
    pub fn named(&self) -> Option<&[NamedAuthority]> {
        match self {
            CompetentAuthority::String(_) => None,
            CompetentAuthority::Structured(one) => Some(std::slice::from_ref(one)),
            CompetentAuthority::List(all) => Some(all),
        }
    }
}

/// Whether a competent authority names one organisation or a category of them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorityType {
    /// A specific organisation (schema default)
    #[serde(rename = "INSTANCE")]
    Instance,
    /// A category that must be resolved per context
    #[serde(rename = "CATEGORY")]
    Category,
}

/// How a field's value relates to time
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Temporal {
    /// `period` or `point_in_time`
    #[serde(rename = "type")]
    pub temporal_type: String,
    /// Granularity of the period (`year`, `month`, `continuous`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    /// Reference date for point-in-time values (a `$`-prefixed variable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Legal basis reference to another law
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegalBasis {
    pub law_id: String,
    pub article: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// A citation of the provision an element carries out, in the shape the schema
/// gives `legal_basis` on a field or an action.
///
/// Deliberately not [`LegalBasis`]: that one addresses a whole law from the
/// document header with `law_id`/`article`, while this one cites a provision
/// down to the lid and carries the wording that ties it to the text. The two
/// name different things and the schema spells them differently, so folding
/// them together would mean one of the two lying about its field names.
///
/// Every field is optional: a citation that names only an article is still a
/// citation, and the schema demands none of them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProvisionReference {
    /// Name of the law, as the law writes it ("Wet op de zorgtoeslag").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law: Option<String>,
    /// BWB identification number (`BWBR` + 7 digits).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwb_id: Option<String>,
    /// Article number, as a string: article numbers are not integers ("1a").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    /// Lid number within the article.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<String>,
    /// Sentence number, for a reference finer than a lid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
    /// Link to the provision on wetten.overheid.nl.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Juriconnect BWB 1.3 reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub juriconnect: Option<String>,
    /// How this element relates to the words of the provision, in Dutch. This
    /// is the part a reader of a trace actually reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// Type specification for input/output/definition fields.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TypeSpec {
    /// Unit of measurement (e.g., "eurocent", "euro", "ratio", "percentage", "days").
    /// A unit is a label, never a computational constraint (RFC-023).
    #[serde(default)]
    pub unit: Option<String>,
    /// Number of decimal places for the value, in its own unit (RFC-023 §2,
    /// issue #444). Parsed metadata; rounding itself is an explicit operation
    /// (RFC-024), never inferred from this field.
    #[serde(default)]
    pub precision: Option<i64>,
    /// Minimum allowed value (issue #444). Parsed metadata, not yet enforced.
    ///
    /// Serialized as a JSON number, because that is what the schema types it
    /// as; `Decimal`'s default is a string, which makes the re-serialized
    /// document schema-invalid. A bound beyond f64 precision is out of range
    /// for what these fields describe.
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub min: Option<rust_decimal::Decimal>,
    /// Maximum allowed value (issue #444). Parsed metadata, not yet enforced.
    /// Serialized as a JSON number; see [`TypeSpec::min`].
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub max: Option<rust_decimal::Decimal>,
}

/// Source specification for input fields
///
/// Defines where an input value comes from. Can be:
/// - Simple regulation reference: `regulation: "other_law"` + `output: "field_name"`
/// - Same-law reference: `output: "field_name"` (resolved within the same law)
/// - The decisions taken under an article: `decisions: "<regulation>#<article>"`
///   (schema v0.8.2, RFC-048), or a `$`-reference to a parameter that holds
///   one (schema v0.8.11), only in a `REDUCTIE`
/// - The recorded instances of a fact an article requires: `facts:
///   "<regulation>#<article>#<fact>"` (schema v0.8.7, RFC-051), only in a
///   `REDUCTIE`
/// - The named answer of a cell: `reduction: "<answer>"` with `parameters`
///   (schema v0.8.9, RFC-048), only in a policy article that fills an open
///   term (`implements`)
/// - The decision of an article: `article: "<regulation>#<article>"` or a
///   `$`-reference that holds one, with `parameters_from` (schema v0.8.4,
///   RFC-045)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// Simple cross-law reference (law ID)
    #[serde(default)]
    pub regulation: Option<String>,
    /// Output field to retrieve from the source.
    /// When None (e.g. `source: {}`), the input is resolved from the DataSourceRegistry.
    #[serde(default)]
    pub output: Option<String>,
    /// Parameters to pass to the source execution
    #[serde(default)]
    pub parameters: Option<BTreeMap<String, String>>,
    /// Endpoint to call on the delegated regulation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Human-readable description or legal reference for this data source
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The decisions taken under this article, `<regulation>#<article>`
    /// (schema v0.8.2, RFC-048), or a `$`-reference to a parameter of the
    /// reading article that holds one (schema v0.8.11): a list with one row
    /// per decision. Only a data source answers it, such as the cell on
    /// whose chronicle the reduction is applied, or the caller with a
    /// parameter of that name; no regulation is executed for it. Only a
    /// rulework with `regulatory_layer: REDUCTIE` may read it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_article_target"
    )]
    pub decisions: Option<String>,
    /// The recorded instances of a fact an article requires,
    /// `<regulation>#<article>#<fact>` (schema v0.8.7, RFC-051): a list with
    /// one row per instance, as `decisions` gives one per decision. The article's
    /// `requires` entry with that `fact` says what each holds. Only a data
    /// source answers it, such as the cell that records the facts, or the
    /// caller with a parameter of that name; no regulation is executed for
    /// it. Only a rulework with `regulatory_layer: REDUCTIE` may read it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_fact_reference"
    )]
    pub facts: Option<String>,
    /// The named answer of a cell this input takes (schema v0.8.9,
    /// RFC-048): the name under which a cell gives it in its catalogue
    /// (`cel.yaml`, `answers`), with `parameters` as the values the cell's
    /// reduction gets under its own names. Only a policy article that fills
    /// an open term (`implements`) takes one: it binds the term to the
    /// answer. The engine asks one cell of the call for it: the one the
    /// filled term names in its `competent_authority`, or else the cell of
    /// the policy's holder. Without a cell that answers, the value is null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduction: Option<String>,
    /// The article whose decision this input takes (schema v0.8.4, RFC-045):
    /// an article reference `<regulation>#<article>`, or a `$`-reference to
    /// a value that holds one, such as the `basis` of a `decision`
    /// value. Without `output` the input is the decision value of that
    /// article; with `output`, that one output of its decision item. Stands
    /// apart from `regulation`, `decisions` and `endpoint`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_article_target"
    )]
    pub article: Option<String>,
    /// `$`-references to objects whose fields are passed to `article` as
    /// parameters, merged in order; `parameters` goes on top (schema v0.8.4,
    /// RFC-045). Only with `article`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_variable_references"
    )]
    pub parameters_from: Option<Vec<String>>,
    /// Schema v0.8.6's way to read the recorded instances of a fact, which
    /// v0.8.7 replaced by `facts` (RFC-051). The model reads it only so the
    /// engine can refuse it at load and name `facts` instead of dropping it
    /// silently; it is never written back.
    #[serde(default, skip_serializing, deserialize_with = "present")]
    pub executograms: Option<serde_json::Value>,
}

/// Parameter definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: ParameterType,
    #[serde(default)]
    pub required: Option<bool>,
    /// Whether `null` (absence) is a legitimate value of this field (RFC-036).
    /// Defaults to false: the engine refuses a `null` that reaches a field not
    /// declared nullable, and the static type check refuses a law that tests a
    /// non-nullable field for absence. Unknown (a fact nobody has) is not
    /// governed by this flag.
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<Temporal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<ProvisionReference>,
}

impl Parameter {
    /// Whether the caller may pass `null` for this parameter (RFC-036).
    pub fn is_nullable(&self) -> bool {
        self.nullable.unwrap_or(false)
    }
}

/// Input definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub input_type: ParameterType,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub type_spec: Option<TypeSpec>,
    /// Whether `null` (absence) is a legitimate value of this field (RFC-036).
    /// Defaults to false: the engine refuses a `null` that reaches a field not
    /// declared nullable, and the static type check refuses a law that tests a
    /// non-nullable field for absence. Unknown (a fact nobody has) is not
    /// governed by this flag.
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<Temporal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<ProvisionReference>,
}

impl Input {
    /// Whether a data source or another law may deliver `null` for this input
    /// (RFC-036).
    pub fn is_nullable(&self) -> bool {
        self.nullable.unwrap_or(false)
    }
}

/// Output definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    #[serde(rename = "type")]
    pub output_type: ParameterType,
    #[serde(default)]
    pub type_spec: Option<TypeSpec>,
    /// Whether `null` (absence) is a legitimate value of this field (RFC-036).
    /// Defaults to false: the engine refuses a `null` that reaches a field not
    /// declared nullable, and the static type check refuses a law that tests a
    /// non-nullable field for absence. Unknown (a fact nobody has) is not
    /// governed by this flag.
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<Temporal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<ProvisionReference>,
}

impl Output {
    /// Whether this output may evaluate to `null` (RFC-036).
    pub fn is_nullable(&self) -> bool {
        self.nullable.unwrap_or(false)
    }
}

/// What an article produces: one object, the form most laws write, or a
/// list of produced things, such as an application and the decision on it
/// (schema v0.8.1, RFC-045). Serializes back in the form it was written in.
///
/// Read it through [`Article::get_produced`], [`Article::get_decision`],
/// [`Article::get_submission`], [`Article::outputs_of`] and
/// [`Article::parameters_of`], so a reader
/// handles both forms alike.
///
/// Deserialized by the shape of the value, a sequence or a map, so an error
/// inside either form reaches the reader as written instead of as "matches
/// no variant".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ProducesList {
    /// A list of produced things, each with its own `type`.
    Many(Vec<ProducedItem>),
    /// One produced thing, the object form.
    One(Box<Produces>),
}

impl ProducesList {
    /// Every produced thing, in the order written; one for the object form.
    pub fn items(&self) -> Vec<Produced<'_>> {
        match self {
            Self::Many(items) => items.iter().map(Produced::Item).collect(),
            Self::One(item) => vec![Produced::Object(item.as_ref())],
        }
    }
}

impl<'de> Deserialize<'de> for ProducesList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FormVisitor;

        impl<'de> serde::de::Visitor<'de> for FormVisitor {
            type Value = ProducesList;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a list of produced items or one produces object")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Self::Value, A::Error> {
                Vec::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))
                    .map(ProducesList::Many)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                Produces::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(|p| ProducesList::One(Box::new(p)))
            }
        }

        deserializer.deserialize_any(FormVisitor)
    }
}

impl From<Produces> for ProducesList {
    fn from(item: Produces) -> Self {
        Self::One(Box::new(item))
    }
}

/// The kinds of submission a produced item's `type` can name (RFC-045). Any
/// other `type` is a legal character.
pub const SUBMISSION_KINDS: &[&str] = &["AANVRAAG", "BEZWAAR", "BEROEP"];

/// One item of `produces` in the list form (schema v0.8.1, RFC-045): a
/// submission, such as an application, or a decision, each with what it
/// contains or yields.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducedItem {
    /// A submission kind (`AANVRAAG`) or a legal character (`BESCHIKKING`).
    #[serde(rename = "type")]
    pub item_type: String,
    /// The article's parameters this item contains, such as the fields of
    /// an application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<String>>,
    /// The article's outputs this item yields. Without it, a submission
    /// yields none and the decision every output no other item lists or
    /// derives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<String>>,
    /// For a decision: the article's outputs that are data derived for it
    /// (schema v0.8.4, RFC-045 §1), computed at the decision's stages and
    /// recorded with it, but not among its outputs, so a call to the
    /// article leaves them out of the decision value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<Vec<String>>,
    /// For a submission: the article whose decision it asks for,
    /// `<regulation>#<article>`, also when that is this article (Awb 1:3
    /// lid 3, 4:2 lid 1 onder c). The target is the decision item of that
    /// article, or the article itself in the object form.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_article_reference"
    )]
    pub requests: Option<String>,
    /// For a decision: the kind of decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_type: Option<String>,
    /// For a decision: the Awb procedure variant (RFC-008).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procedure_id: Option<String>,
    /// The moment that counts in law for this item (RFC-045).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moment: Option<Moment>,
}

impl ProducedItem {
    /// Whether `type` names a submission kind rather than a legal character.
    pub fn is_submission(&self) -> bool {
        SUBMISSION_KINDS.contains(&self.item_type.as_str())
    }
}

/// One thing an article produces, in either form of `produces`: the object
/// form, which may carry a legal character and a submission at once, or an
/// item of the list form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Produced<'a> {
    /// The object form of `produces`.
    Object(&'a Produces),
    /// An item of the list form.
    Item(&'a ProducedItem),
}

impl<'a> Produced<'a> {
    /// The legal character, such as `BESCHIKKING`, when this is a decision.
    pub fn legal_character(self) -> Option<&'a str> {
        match self {
            Self::Object(p) => p.legal_character.as_deref(),
            Self::Item(i) => (!i.is_submission()).then_some(i.item_type.as_str()),
        }
    }

    /// The kind of submission, such as `AANVRAAG`, when this establishes one.
    pub fn submission_kind(self) -> Option<&'a str> {
        match self {
            Self::Object(p) => p.submission.as_ref().map(|s| s.kind.as_str()),
            Self::Item(i) => i.is_submission().then_some(i.item_type.as_str()),
        }
    }

    /// The kind of decision, within its legal character.
    pub fn decision_type(self) -> Option<&'a str> {
        match self {
            Self::Object(p) => p.decision_type.as_deref(),
            Self::Item(i) => i.decision_type.as_deref(),
        }
    }

    /// The Awb procedure variant asked for.
    pub fn procedure_id(self) -> Option<&'a str> {
        match self {
            Self::Object(p) => p.procedure_id.as_deref(),
            Self::Item(i) => i.procedure_id.as_deref(),
        }
    }

    /// The moment that counts in law (RFC-045).
    pub fn moment(self) -> Option<&'a Moment> {
        match self {
            Self::Object(p) => p.moment.as_ref(),
            Self::Item(i) => i.moment.as_ref(),
        }
    }

    /// The outputs the item lists; `None` when it lists none, and always for
    /// the object form. Read [`Article::outputs_of`] for what it yields.
    pub fn listed_outputs(self) -> Option<&'a [String]> {
        match self {
            Self::Object(_) => None,
            Self::Item(i) => i.outputs.as_deref(),
        }
    }

    /// The outputs the item lists as data derived for it (schema v0.8.4,
    /// RFC-045 §1); `None` when it lists none, and always for the object form.
    pub fn listed_derived(self) -> Option<&'a [String]> {
        match self {
            Self::Object(_) => None,
            Self::Item(i) => i.derived.as_deref(),
        }
    }

    /// For a submission in the list form: the article whose decision it
    /// asks for, `<regulation>#<article>`.
    pub fn requests(self) -> Option<&'a str> {
        match self {
            Self::Object(_) => None,
            Self::Item(i) => i.requests.as_deref(),
        }
    }

    /// Whether this is the same produced thing as `other`.
    pub fn same_as(self, other: Produced<'_>) -> bool {
        match (self, other) {
            (Self::Object(a), Produced::Object(b)) => std::ptr::eq(a, b),
            (Self::Item(a), Produced::Item(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }
}

/// The object form of what an article produces: the legal character of what
/// it yields, the kind of decision, the procedure, and a submission it
/// establishes.
/// May be extended with additional metadata (appeal_period, notification_requirement) as schema evolves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Produces {
    /// Legal character of the output (e.g., "BESCHIKKING", "TOETS")
    #[serde(default)]
    pub legal_character: Option<String>,
    /// Type of decision (e.g., "TOEKENNING", "GOEDKEURING")
    #[serde(default)]
    pub decision_type: Option<String>,
    /// Selects a specific AWB procedure variant (RFC-008).
    /// When absent, the default procedure for the legal_character is used.
    #[serde(default)]
    pub procedure_id: Option<String>,
    /// The article establishes something a belanghebbende submits, such as
    /// an application (Awb 1:3 lid 3), that the general law hooks onto with
    /// `applies_to.submission` (RFC-045). Next to the legal character of the
    /// article's own outputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission: Option<Submission>,
    /// The moment that counts in law for what this article produces, such as
    /// the receipt of an application (Awb 4:13 lid 1). Metadata for a process
    /// runtime; the engine does not read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moment: Option<Moment>,
}

/// What an article establishes that a belanghebbende submits (RFC-045).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    /// The kind of submission, as the law names it: one of
    /// [`SUBMISSION_KINDS`], `AANVRAAG` (Awb 1:3 lid 3), `BEZWAAR` (Awb 1:5
    /// lid 1) or `BEROEP` (Awb 8:1, schema v0.8.10).
    pub kind: String,
}

/// The moment that counts in law for what an article produces (RFC-045).
/// With `parameter`, that parameter's value is the moment; without it, the
/// moment is the one at which a runtime records the fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Moment {
    /// The parameter whose value is the moment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    /// The provision that says why this moment counts, in the schema's
    /// `legalBasis` shape. Required by the schema.
    pub legal_basis: ProvisionReference,
}

/// A single case in an IF operation (cases/default syntax)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// Condition to evaluate
    pub when: ActionValue,
    /// Value to return if condition is true
    pub then: ActionValue,
}

/// Represents a value in an action - can be a literal, variable reference, or nested operation.
///
/// Uses `#[serde(untagged)]` for flexible YAML parsing. The Operation variant is tried first,
/// but this is safe because `ActionOperation` is an internally-tagged enum keyed on `"operation"` -
/// any YAML object lacking an `operation` key will fail to deserialize as ActionOperation and
/// fall through to the Literal variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionValue {
    /// Nested operation (tried first; requires `operation` field to match)
    Operation(Box<ActionOperation>),
    /// Literal value (number, string, boolean, variable reference like "$var", etc.)
    Literal(Value),
}

/// Represents an operation within an action.
///
/// Uses an internally-tagged enum (`"operation"` field) so that each variant
/// only carries the fields it actually needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum ActionOperation {
    // Comparison (subject + value)
    #[serde(rename = "EQUALS")]
    Equals {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "NOT_EQUALS")]
    NotEquals {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "GREATER_THAN")]
    GreaterThan {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "LESS_THAN")]
    LessThan {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "GREATER_THAN_OR_EQUAL")]
    GreaterThanOrEqual {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "LESS_THAN_OR_EQUAL")]
    LessThanOrEqual {
        subject: ActionValue,
        value: ActionValue,
    },

    // Arithmetic (values)
    #[serde(rename = "ADD")]
    Add { values: Vec<ActionValue> },
    #[serde(rename = "SUBTRACT")]
    Subtract { values: Vec<ActionValue> },
    #[serde(rename = "MULTIPLY")]
    Multiply { values: Vec<ActionValue> },
    #[serde(rename = "DIVIDE")]
    Divide { values: Vec<ActionValue> },

    // Aggregate (values)
    #[serde(rename = "MAX")]
    Max { values: Vec<ActionValue> },
    #[serde(rename = "MIN")]
    Min { values: Vec<ActionValue> },

    // Rounding (unary value + precision; RFC-024)
    #[serde(rename = "ROUND")]
    Round { value: ActionValue, precision: i64 },
    #[serde(rename = "CEIL")]
    Ceil { value: ActionValue, precision: i64 },
    #[serde(rename = "FLOOR")]
    Floor { value: ActionValue, precision: i64 },

    // Logical
    #[serde(rename = "AND")]
    And { conditions: Vec<ActionValue> },
    #[serde(rename = "OR")]
    Or { conditions: Vec<ActionValue> },
    #[serde(rename = "NOT")]
    Not { value: ActionValue },

    // Conditional
    #[serde(rename = "IF", alias = "SWITCH")]
    If {
        cases: Vec<Case>,
        #[serde(default)]
        default: Option<ActionValue>,
    },

    // Null checking
    #[serde(rename = "IS_NULL")]
    IsNull { subject: ActionValue },
    #[serde(rename = "NOT_NULL")]
    NotNull { subject: ActionValue },

    // Collection
    #[serde(rename = "IN")]
    In {
        subject: ActionValue,
        #[serde(default)]
        value: Option<ActionValue>,
        #[serde(default)]
        values: Option<Vec<ActionValue>>,
    },
    #[serde(rename = "NOT_IN")]
    NotIn {
        subject: ActionValue,
        #[serde(default)]
        value: Option<ActionValue>,
        #[serde(default)]
        values: Option<Vec<ActionValue>>,
    },
    #[serde(rename = "LIST")]
    List { items: Vec<ActionValue> },
    /// Iterate over a collection, evaluating `body` per element (RFC-016).
    ///
    /// The only operation that *defines* a variable rather than reading one:
    /// `as_name` is bound in a child scope that covers `filter` and `body`.
    /// `collection` is evaluated in the scope where the FOREACH itself appears,
    /// which is what lets a nested FOREACH reach an outer binding.
    ///
    /// The field names match the schema exactly. Earlier drafts of RFC-016 used
    /// `subject`/`value`/`where`, and accepting those as aliases was considered,
    /// but no law was ever written with them: FOREACH was a schema-less
    /// `otherOperation` fallback in v0.2.0-v0.4.0 with no fixed field names. An
    /// alias the schema rejects would only make the model accept documents
    /// `just validate` refuses, which is the soundness gap the conformance suite
    /// exists to prevent.
    #[serde(rename = "FOREACH")]
    Foreach {
        collection: ActionValue,
        /// The binding as written. `None` when the law omits `as`: the
        /// element is then bound as `item` and an object element's fields are
        /// also bound under their own names. With `as` given, only that name
        /// is bound (RFC-049). See [`foreach_binding`].
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        as_name: Option<String>,
        body: ActionValue,
        #[serde(default)]
        filter: Option<ActionValue>,
        #[serde(default)]
        combine: Option<CombineOp>,
    },

    // Date
    #[serde(rename = "AGE")]
    Age {
        date_of_birth: ActionValue,
        reference_date: ActionValue,
    },
    #[serde(rename = "DATE_ADD")]
    DateAdd {
        date: ActionValue,
        #[serde(default)]
        years: Option<ActionValue>,
        #[serde(default)]
        months: Option<ActionValue>,
        #[serde(default)]
        weeks: Option<ActionValue>,
        #[serde(default)]
        days: Option<ActionValue>,
    },
    #[serde(rename = "DATE")]
    Date {
        year: ActionValue,
        month: ActionValue,
        day: ActionValue,
    },
    #[serde(rename = "DAY_OF_WEEK")]
    DayOfWeek { date: ActionValue },
    #[serde(rename = "DATE_DIFF")]
    DateDiff {
        from: ActionValue,
        to: ActionValue,
        /// Unit the difference is expressed in: "days", "months", or "years".
        /// Named `in` in YAML so the operation reads as one clause
        /// ("date diff from X to Y in days").
        #[serde(rename = "in")]
        unit: ActionValue,
    },
    #[serde(rename = "DATE_PART")]
    DatePart {
        date: ActionValue,
        /// Calendar component to read: "year", "month" or "day".
        ///
        /// A plain `String` and not an `ActionValue`: the component to read is
        /// never itself the outcome of a computation, so the schema fixes it to
        /// an enum and the model does not widen that to a variable reference.
        #[serde(rename = "in")]
        part: String,
    },
    #[serde(rename = "START_OF")]
    StartOf {
        date: ActionValue,
        /// Calendar unit to truncate to: "year" or "month".
        #[serde(rename = "in")]
        unit: String,
    },
}

/// Aggregation applied to the per-element results of a FOREACH (RFC-016).
///
/// A typed enum rather than a string, so an unknown `combine` is rejected when
/// the law is loaded instead of surfacing as a runtime error mid-evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombineOp {
    Add,
    Or,
    And,
    Min,
    Max,
    /// The one result: null when the filter left none, an error when it
    /// left more than one (schema v0.8.6). It picks a single element, such
    /// as the record in force on a day, where a lowest or highest of a
    /// string or a date is not defined.
    Only,
}

impl CombineOp {
    /// The name as the law writes it (`ADD`, `OR`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            CombineOp::Add => "ADD",
            CombineOp::Or => "OR",
            CombineOp::And => "AND",
            CombineOp::Min => "MIN",
            CombineOp::Max => "MAX",
            CombineOp::Only => "ONLY",
        }
    }
}

/// Default name for the FOREACH element binding when `as` is omitted.
///
/// Neutral and language-independent, so it does not collide with the Dutch
/// domain names laws use for their own variables.
pub const DEFAULT_FOREACH_AS: &str = "item";

/// The name a FOREACH binds its element to: its `as`, or
/// [`DEFAULT_FOREACH_AS`] when it has none.
pub fn foreach_binding(as_name: Option<&str>) -> &str {
    as_name.unwrap_or(DEFAULT_FOREACH_AS)
}

impl ActionOperation {
    /// Every operand of the operation, in the order the fields are written.
    ///
    /// Exhaustive over the variants, so a new operation cannot hide an operand
    /// from the walkers built on this (the load check for literal unknowns,
    /// the dependency closure of RFC-043).
    pub fn operands(&self) -> Vec<&ActionValue> {
        match self {
            ActionOperation::Equals { subject, value }
            | ActionOperation::NotEquals { subject, value }
            | ActionOperation::GreaterThan { subject, value }
            | ActionOperation::LessThan { subject, value }
            | ActionOperation::GreaterThanOrEqual { subject, value }
            | ActionOperation::LessThanOrEqual { subject, value } => vec![subject, value],
            ActionOperation::Add { values }
            | ActionOperation::Subtract { values }
            | ActionOperation::Multiply { values }
            | ActionOperation::Divide { values }
            | ActionOperation::Max { values }
            | ActionOperation::Min { values } => values.iter().collect(),
            ActionOperation::Round { value, .. }
            | ActionOperation::Ceil { value, .. }
            | ActionOperation::Floor { value, .. }
            | ActionOperation::Not { value } => vec![value],
            ActionOperation::And { conditions } | ActionOperation::Or { conditions } => {
                conditions.iter().collect()
            }
            ActionOperation::If { cases, default } => cases
                .iter()
                .flat_map(|case| [&case.when, &case.then])
                .chain(default)
                .collect(),
            ActionOperation::IsNull { subject } | ActionOperation::NotNull { subject } => {
                vec![subject]
            }
            ActionOperation::In {
                subject,
                value,
                values,
            }
            | ActionOperation::NotIn {
                subject,
                value,
                values,
            } => std::iter::once(subject)
                .chain(value)
                .chain(values.iter().flatten())
                .collect(),
            ActionOperation::List { items } => items.iter().collect(),
            ActionOperation::Foreach {
                collection,
                body,
                filter,
                ..
            } => [collection, body].into_iter().chain(filter).collect(),
            ActionOperation::Age {
                date_of_birth,
                reference_date,
            } => vec![date_of_birth, reference_date],
            ActionOperation::DateAdd {
                date,
                years,
                months,
                weeks,
                days,
            } => std::iter::once(date)
                .chain([years, months, weeks, days].into_iter().flatten())
                .collect(),
            ActionOperation::Date { year, month, day } => vec![year, month, day],
            ActionOperation::DayOfWeek { date }
            | ActionOperation::DatePart { date, .. }
            | ActionOperation::StartOf { date, .. } => vec![date],
            ActionOperation::DateDiff { from, to, unit } => vec![from, to, unit],
        }
    }

    /// Get the operation name as a static uppercase string (for tracing).
    pub fn operation_name(&self) -> &'static str {
        match self {
            ActionOperation::Equals { .. } => "EQUALS",
            ActionOperation::NotEquals { .. } => "NOT_EQUALS",
            ActionOperation::GreaterThan { .. } => "GREATER_THAN",
            ActionOperation::LessThan { .. } => "LESS_THAN",
            ActionOperation::GreaterThanOrEqual { .. } => "GREATER_THAN_OR_EQUAL",
            ActionOperation::LessThanOrEqual { .. } => "LESS_THAN_OR_EQUAL",
            ActionOperation::Add { .. } => "ADD",
            ActionOperation::Subtract { .. } => "SUBTRACT",
            ActionOperation::Multiply { .. } => "MULTIPLY",
            ActionOperation::Divide { .. } => "DIVIDE",
            ActionOperation::Max { .. } => "MAX",
            ActionOperation::Min { .. } => "MIN",
            ActionOperation::Round { .. } => "ROUND",
            ActionOperation::Ceil { .. } => "CEIL",
            ActionOperation::Floor { .. } => "FLOOR",
            ActionOperation::And { .. } => "AND",
            ActionOperation::Or { .. } => "OR",
            ActionOperation::Not { .. } => "NOT",
            ActionOperation::If { .. } => "IF",
            ActionOperation::IsNull { .. } => "IS_NULL",
            ActionOperation::NotNull { .. } => "NOT_NULL",
            ActionOperation::In { .. } => "IN",
            ActionOperation::NotIn { .. } => "NOT_IN",
            ActionOperation::List { .. } => "LIST",
            ActionOperation::Foreach { .. } => "FOREACH",
            ActionOperation::Age { .. } => "AGE",
            ActionOperation::DateAdd { .. } => "DATE_ADD",
            ActionOperation::Date { .. } => "DATE",
            ActionOperation::DayOfWeek { .. } => "DAY_OF_WEEK",
            ActionOperation::DateDiff { .. } => "DATE_DIFF",
            ActionOperation::DatePart { .. } => "DATE_PART",
            ActionOperation::StartOf { .. } => "START_OF",
        }
    }
}

/// Action definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub operation: Option<Operation>,
    /// Single value (can be literal, variable reference, or nested operation)
    #[serde(default)]
    pub value: Option<ActionValue>,
    /// Multiple values for aggregate/arithmetic operations
    #[serde(default)]
    pub values: Option<Vec<ActionValue>>,
    /// Subject for comparison operations
    #[serde(default)]
    pub subject: Option<ActionValue>,
    /// Conditions for AND/OR operations
    #[serde(default)]
    pub conditions: Option<Vec<ActionValue>>,
    /// Decimal places for rounding operations (ROUND/CEIL/FLOOR; RFC-024)
    #[serde(default)]
    pub precision: Option<i64>,
    /// Delegation resolution: take the value from an implementing regulation.
    /// Parsed but not yet executed by the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve: Option<ResolveSpec>,
    /// The provision this action carries out (RFC-039). Read onto the action's
    /// trace step, so a result can cite the sentence it follows from rather
    /// than only the article it was reached through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<ProvisionReference>,
}

impl Action {
    /// The operands written on the action itself: `value`, and for the
    /// action-level operation shape `subject`, `values` and `conditions`.
    pub fn operands(&self) -> impl Iterator<Item = &ActionValue> {
        self.value
            .iter()
            .chain(self.subject.iter())
            .chain(self.values.iter().flatten())
            .chain(self.conditions.iter().flatten())
    }
}

/// Delegation resolution on an action: which implementing regulation supplies
/// the value.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ResolveSpec {
    /// Regulatory layer to search for an implementation
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub resolve_type: Option<String>,
    /// Output field to retrieve from the implementation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Matching criteria for selecting the right implementation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#match: Option<BTreeMap<String, Value>>,
}

/// Execution specification within machine_readable section
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Execution {
    #[serde(default)]
    pub produces: Option<ProducesList>,
    #[serde(default)]
    pub parameters: Option<Vec<Parameter>>,
    #[serde(default)]
    pub input: Option<Vec<Input>>,
    #[serde(default)]
    pub output: Option<Vec<Output>>,
    #[serde(default)]
    pub actions: Option<Vec<Action>>,
}

/// Definition value in definitions section.
///
/// A constant may be a bare value (`naam: 123`, backward-compatible) or the
/// optionally-structured form carrying a `type` and `type_spec` so it can
/// declare its quantity-kind (RFC-023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Definition {
    /// Structured form: `value` plus optional `type`/`type_spec` (RFC-023).
    Structured {
        value: Value,
        #[serde(rename = "type", default)]
        def_type: Option<ParameterType>,
        #[serde(default)]
        type_spec: Option<TypeSpec>,
    },
    /// Simple value (for backward compatibility)
    Simple(Value),
}

impl Definition {
    /// Get the value from this definition
    pub fn value(&self) -> &Value {
        match self {
            Definition::Structured { value, .. } => value,
            Definition::Simple(v) => v,
        }
    }

    /// The declared unit of this constant, if any (RFC-023). Bare constants and
    /// structured constants without a `type_spec.unit` return `None`.
    pub fn unit(&self) -> Option<&str> {
        match self {
            Definition::Structured { type_spec, .. } => {
                type_spec.as_ref().and_then(|t| t.unit.as_deref())
            }
            Definition::Simple(_) => None,
        }
    }
}

/// Default execution block for an open term (used when no implementing regulation exists)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenTermDefault {
    #[serde(default)]
    pub actions: Option<Vec<Action>>,
}

/// Open term declared by an article — a value that can or must be filled by
/// implementing regulations at a lower level.
///
/// Any regulatory layer can declare open_terms. A law (`WET`) typically has
/// `required: true` with no default, while lower layers often provide defaults
/// that can be refined further down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenTerm {
    /// Identifier for this open term (e.g., "standaardpremie")
    pub id: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// Data type of the expected value
    #[serde(rename = "type")]
    pub term_type: ParameterType,
    /// Whether an implementation is mandatory (default: true)
    #[serde(default = "default_true")]
    pub required: bool,
    /// Who is authorized to fill this term (e.g., "minister")
    #[serde(default)]
    pub delegated_to: Option<String>,
    /// Expected regulatory layer of the implementation
    #[serde(default)]
    pub delegation_type: Option<String>,
    /// The regulation the article itself names as the one that fills this term
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_source: Option<String>,
    /// Marks the term as judged per individual case (Awb 3:46), with in
    /// words who judges it. Who gives the finding is not read from these
    /// words: it is [`Self::competent_authority`], or else the authority
    /// that executes the article (RFC-048 §2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_per_case_by: Option<String>,
    /// Legal basis text
    #[serde(default)]
    pub legal_basis: Option<String>,
    /// Default execution if no implementing regulation exists
    #[serde(default)]
    pub default: Option<OpenTermDefault>,
    /// Who may answer this term for a reader that reaches the article
    /// through `source.regulation` (schema v0.8.8, RFC-051): one named
    /// authority or a list. The article's own `competent_authority` says
    /// who decides (RFC-002), which is another question. On a term judged
    /// per case it also says who gives the finding (RFC-048 §2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competent_authority: Option<CompetentAuthority>,
}

impl OpenTerm {
    /// Whether the term carries a default the engine can run. `default: {}`
    /// or `default: {actions: []}` resolves to `null` ("using empty
    /// default"), and counts as no default.
    pub fn has_default(&self) -> bool {
        self.default
            .as_ref()
            .is_some_and(|d| d.actions.as_ref().is_some_and(|a| !a.is_empty()))
    }
}

fn default_true() -> bool {
    true
}

/// A dependency of an article on another article, law, ministerial regulation
/// or royal decree.
///
/// Every field is optional in the schema (`machineReadableSection.requires`),
/// so an entry only names the dimensions it needs: an intra-law dependency
/// carries just `article`, a cross-law dependency `law` + `values`. Since
/// schema v0.8.7 an entry may instead name a fact the article requires to
/// exist (`fact`), with the parameters it holds (RFC-051).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ArticleRequirement {
    /// Article number within this law
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    /// Name of the external law depended on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law: Option<String>,
    /// Name of the ministerial regulation depended on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regeling: Option<String>,
    /// Name of the royal decree depended on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub koninklijk_besluit: Option<String>,
    /// Values/outputs required from the dependency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    /// A fact the article requires to exist (schema v0.8.7, RFC-051), by a
    /// name unique within the article. The law says what it holds, not when
    /// it arises.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact: Option<String>,
    /// With `fact`: the article's parameters the fact holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<String>>,
    /// With `fact`: what the fact is, in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// With `fact`: who establishes it (schema v0.8.8, RFC-051), one named
    /// authority or a list, such as the GGD for a medical examination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competent_authority: Option<CompetentAuthority>,
}

/// Declares that this article fills an open term from a higher-level law.
/// Maps to the "Gelet op" clause in Dutch legislation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplementsDeclaration {
    /// The $id of the higher-level law being implemented
    pub law: String,
    /// Article number in the higher law that declares the open_term
    pub article: String,
    /// The open_term id being filled
    pub open_term: String,
    /// Legal reference text (e.g., "Gelet op artikel 4 van de Wet op de zorgtoeslag")
    #[serde(default)]
    pub gelet_op: Option<String>,
}

/// Lifecycle point at which a hook fires
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    /// Fires between open-term resolution and action execution
    PreActions,
    /// Fires between action execution and result return
    PostActions,
}

impl HookPoint {
    /// Returns the hook point as a lowercase static string.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookPoint::PreActions => "pre_actions",
            HookPoint::PostActions => "post_actions",
        }
    }
}

/// Filter that determines when a hook fires
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookFilter {
    /// Match articles that produce this legal character (e.g., "BESCHIKKING")
    #[serde(default)]
    pub legal_character: Option<String>,
    /// Optionally narrow to a specific decision type (e.g., "TOEKENNING")
    #[serde(default)]
    pub decision_type: Option<String>,
    /// Lifecycle stage at which this hook fires (e.g., "BESLUIT", "BEKENDMAKING")
    /// When absent, defaults to BESLUIT for backward compatibility.
    #[serde(default)]
    pub stage: Option<String>,
    /// Match articles that establish a submission of this kind (RFC-045),
    /// such as `AANVRAAG`: Awb 4:2 applies to every application. A hook names
    /// either this or `legal_character`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission: Option<String>,
    /// With `submission`: only the submission that this article establishes
    /// (`<regulation>#<article>`, without a paragraph). A hook without it
    /// applies to every submission of the kind.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_article_reference"
    )]
    pub established_by: Option<String>,
}

/// Whether `s` is an article reference of the schema (`articleReference`):
/// `<regulation>#<article>` with no ` lid <n>` (schema pattern
/// `^[a-z0-9_]+#[A-Za-z0-9]([A-Za-z0-9:._ -]*[A-Za-z0-9])?$`, `not` the
/// pattern ` lid [0-9]`). An article with a space, such as Kieswet `G 1`, is
/// one.
pub fn is_article_reference(s: &str) -> bool {
    let Some((regulation, article)) = s.split_once('#') else {
        return false;
    };
    let alnum = |c: char| c.is_ascii_alphanumeric();
    let names_paragraph = s
        .match_indices(" lid ")
        .any(|(i, m)| s[i + m.len()..].starts_with(|c: char| c.is_ascii_digit()));
    !regulation.is_empty()
        && regulation
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && article.starts_with(alnum)
        && article.ends_with(alnum)
        && article
            .chars()
            .all(|c| alnum(c) || matches!(c, ':' | '.' | '_' | ' ' | '-'))
        && !names_paragraph
}

/// Whether `s` names a fact an article requires, `<regulation>#<article>#<fact>`
/// (schema v0.8.7): an article reference, `#`, and a fact name in lowercase
/// letters, digits and underscores. An article number holds no `#`, so the
/// last one separates the fact.
pub fn is_fact_reference(s: &str) -> bool {
    let Some((article, fact)) = s.rsplit_once('#') else {
        return false;
    };
    is_article_reference(article)
        && fact.starts_with(|c: char| c.is_ascii_lowercase())
        && fact
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Deserialize an optional string that, when present, must be a fact
/// reference ([`is_fact_reference`]).
fn optional_fact_reference<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|s| {
            if is_fact_reference(&s) {
                Ok(s)
            } else {
                Err(serde::de::Error::custom(format!(
                    "'{s}' is not <regulation>#<article>#<fact>"
                )))
            }
        })
        .transpose()
}

/// Deserialize an optional string that, when present, must be an
/// [`is_article_reference`].
fn optional_article_reference<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|s| {
            if is_article_reference(&s) {
                Ok(s)
            } else {
                Err(serde::de::Error::custom(format!(
                    "'{s}' is not <regulation>#<article> without a paragraph"
                )))
            }
        })
        .transpose()
}

/// Whether `s` is a variable reference of the schema (`variableReference`):
/// `$` and a name, optionally followed by `.field` steps (schema pattern
/// `^\$[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*$`).
pub fn is_variable_reference(s: &str) -> bool {
    let Some(path) = s.strip_prefix('$') else {
        return false;
    };
    path.split('.').all(|step| {
        step.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && step.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Deserialize an optional `source.article`: an [`is_article_reference`] or
/// an [`is_variable_reference`].
fn optional_article_target<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|s| {
            if is_article_reference(&s) || is_variable_reference(&s) {
                Ok(s)
            } else {
                Err(serde::de::Error::custom(format!(
                    "'{s}' is neither <regulation>#<article> without a paragraph nor a \
                     $-reference"
                )))
            }
        })
        .transpose()
}

/// Deserialize an optional, non-empty list of [`is_variable_reference`]s.
fn optional_variable_references<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(references) = Option::<Vec<String>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    if references.is_empty() {
        return Err(serde::de::Error::custom(
            "parameters_from lists at least one $-reference",
        ));
    }
    if let Some(bad) = references.iter().find(|r| !is_variable_reference(r)) {
        return Err(serde::de::Error::custom(format!(
            "'{bad}' in parameters_from is no $-reference"
        )));
    }
    Ok(Some(references))
}

/// Record that a key is present, whatever it holds: also a bare key or an
/// explicit `null`, which plain `Option` would read as absent.
fn present<'de, D>(deserializer: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde_json::Value::deserialize(deserializer).map(Some)
}

/// Declaration that an article fires as a hook on matching lifecycle events (RFC-007)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookDeclaration {
    /// When in the lifecycle this hook fires
    pub hook_point: HookPoint,
    /// What triggers this hook
    pub applies_to: HookFilter,
}

/// Declaration that an article overrides another article's output (RFC-007, lex specialis)
///
/// Used for "in afwijking van artikel X" patterns where one law unilaterally
/// replaces another law's output value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OverrideDeclarationFields")]
pub struct OverrideDeclaration {
    /// The $id of the law being overridden
    pub law: String,
    /// The article number being overridden
    pub article: String,
    /// The specific output being replaced
    pub output: String,
    /// The overridden output does not arise at all, rather than taking another
    /// value.
    ///
    /// "Bestaat geen aanspraak" is not an entitlement of zero. With an
    /// entitlement of zero there is a decision carrying legal remedies and a
    /// ground for recovery; with no entitlement there is neither. An engine
    /// reads this flag and needs no knowledge of administrative law to act on
    /// it, which is why the ground sits in a quotation beside it rather than in
    /// a vocabulary the engine would have to interpret.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub voids: bool,
    /// The words of this article that establish the override, verbatim.
    ///
    /// Required when `voids` is set, optional otherwise: see
    /// [`OverrideDeclarationFields`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_text_excerpt: Option<String>,
}

/// The wire shape of an [`OverrideDeclaration`], before the one rule the field
/// types cannot carry.
///
/// A void without a ground is the shape that must not be expressible. Voiding
/// says the entitlement does not arise at all, and the only thing a person
/// gets back for that is the words that exclude them. A replacement needs no
/// quotation, so the field stays optional and the requirement is conditional,
/// which no field type expresses. The schema states it as a conditional, and
/// the model has to reject the same file or the two disagree about what a
/// valid law is.
#[derive(Deserialize)]
struct OverrideDeclarationFields {
    law: String,
    article: String,
    output: String,
    #[serde(default)]
    voids: bool,
    #[serde(default)]
    legal_text_excerpt: Option<String>,
}

impl TryFrom<OverrideDeclarationFields> for OverrideDeclaration {
    type Error = String;

    fn try_from(fields: OverrideDeclarationFields) -> Result<Self, Self::Error> {
        if fields.voids && fields.legal_text_excerpt.is_none() {
            return Err(format!(
                "override on {}:{} voids `{}` without `legal_text_excerpt`: \
                 a void must quote the words that exclude the entitlement",
                fields.law, fields.article, fields.output
            ));
        }
        Ok(Self {
            law: fields.law,
            article: fields.article,
            output: fields.output,
            voids: fields.voids,
            legal_text_excerpt: fields.legal_text_excerpt,
        })
    }
}

/// A required input for a procedure stage
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageRequirement {
    /// Name of the required input
    pub name: String,
    /// Data type of the required input
    #[serde(rename = "type")]
    pub req_type: ParameterType,
}

/// A stage in an AWB-defined procedure lifecycle (RFC-008)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    /// Stage name (e.g., "AANVRAAG", "BESLUIT", "BEKENDMAKING")
    pub name: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// External inputs required to enter this stage
    #[serde(default)]
    pub requires: Option<Vec<StageRequirement>>,
}

/// Filter for which legal character a procedure applies to
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcedureAppliesTo {
    /// Legal character (e.g., "BESCHIKKING")
    pub legal_character: String,
}

/// A procedure definition — an AWB-defined lifecycle for a legal character (RFC-008)
///
/// Procedures are defined by the AWB, not by specific laws. Laws declare which
/// procedure they participate in via `produces.legal_character`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcedureDefinition {
    /// Unique identifier for this procedure (e.g., "beschikking", "beschikking_uov")
    pub id: String,
    /// Whether this is the default procedure for its legal_character
    #[serde(default)]
    pub default: Option<bool>,
    /// Which legal character this procedure governs
    pub applies_to: ProcedureAppliesTo,
    /// Ordered sequence of lifecycle stages
    pub stages: Vec<Stage>,
}

/// What has to change before a marked article can be translated in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkingResolution {
    /// The operation does not exist and must be built (statutory rounding,
    /// extracting the year from a date). Building one reaches past the engine:
    /// schema, model, evaluation, the BDD grammar and the frontend all carry
    /// it, so the value names what has to be added rather than where it runs.
    Operation,
    /// The operation set is not the problem; the format has no shape for this
    /// construct (quantification over persons, a rule about a set rather than a
    /// value, a legal fiction).
    Model,
}

impl MarkingResolution {
    /// The value as it is written in YAML.
    pub fn as_str(&self) -> &'static str {
        match self {
            MarkingResolution::Operation => "operation",
            MarkingResolution::Model => "model",
        }
    }
}

/// A construct in an article that the format itself cannot express (schema v0.7.0).
///
/// A marking is a flag on an article that is otherwise worked out: it names the
/// one thing that does not fit and leaves everything that does fit standing. It
/// is the opposite of an [`OpenTerm`], which says the language expresses this
/// fine and the content is filled elsewhere.
///
/// The engine acts on markings, and it acts on them the way it acts on an
/// [`UntranslatableEntry`]. The four RFC-012 modes read both channels: an
/// unaccepted marking stops execution, an accepted one runs the partial logic.
/// That has to be so, because the laws migrated to v0.7.0 carry markings where
/// they used to carry untranslatables, and reading only the old channel would
/// let a flagged article execute as if nothing were flagged. See
/// `flagged_constructs` in the engine's service layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marking {
    /// The construct that cannot be expressed, in the words the article uses.
    pub about: String,
    /// Why the construct does not fit, in terms of what the format does have.
    ///
    /// The diagnosis, and the half that cannot be recovered from the other
    /// fields: `resolved_by` follows from it and not the other way round.
    /// Without it a marking states a wish, and a gap someone examined reads
    /// the same as one nobody did.
    pub reason: String,
    /// Whether resolving this needs a new engine operation or a new model shape.
    pub resolution: MarkingResolution,
    /// The change that would resolve it, named concretely enough to become work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// The values in this article that cannot be produced because of this
    /// marking. An empty list is a statement and not an omission: it says the
    /// marking is a flag on an article that is otherwise worked out, and that
    /// only the explanation is incomplete.
    ///
    /// Read by nobody here. The engine decides what a marked article does from
    /// `accepted` and the modes of RFC-012, which do not consult this field;
    /// the rule that a named value is absent from the article's actions is
    /// enforced by the enrichment gate when the file is written. The schema is
    /// the canonical statement of both.
    pub target: Vec<String>,
    /// The words from this article's own legal text that the marking hangs on.
    pub legal_text_excerpt: String,
    /// Whether a human has reviewed and acknowledged this gap.
    #[serde(default)]
    pub accepted: bool,
}

/// A top-level document property an article establishes (schema v0.7.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredProperty {
    Name,
    OfficieleTitel,
    ValidFrom,
    ValidTo,
    RegulatoryLayer,
    LegalBasis,
}

/// Declaration that this article fixes a document property (schema v0.7.0).
///
/// A citation title, a commencement date or a scope in time is not a
/// calculation, and it is not nothing either: it fixes a value the rest of the
/// corpus and every trace depend on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Declaration {
    /// The document property this article fixes.
    pub property: DeclaredProperty,
    /// The value the article gives it, verbatim from the text.
    pub value: Value,
    /// Where the article limits the property in time beyond the value itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_from: Option<String>,
}

/// One container in an article's [`Placement`]: its number and its opschrift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementContainer {
    /// Number of this container as the law gives it (e.g. "3.3").
    pub number: String,
    /// Opschrift of this container (e.g. "Advisering").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
}

/// Where an article sits in the document: the containers that enclose it
/// (schema v0.7.0).
///
/// The opschrift is condensed legal classification written by the legislator and
/// decides questions the article text alone cannot answer. Absent for an article
/// that no container encloses, which is normal in a short law.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Placement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boek: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deel: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titeldeel: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<PlacementContainer>,
}

/// Structured reference to another law/article for runtime resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleReference {
    /// Reference ID used in markdown links (e.g. "ref1").
    pub id: String,
    /// BWB identifier of the referenced law.
    pub bwb_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artikel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onderdeel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<String>,
}

/// A legal construct that cannot be expressed with the engine's current operation set (RFC-012)
///
/// Superseded by [`Marking`] in schema v0.7.0. It stays on the model because the
/// model is one struct for every version in
/// `regelrecht_engine::config::SUPPORTED_SCHEMAS`, and every law in the corpus
/// is still on v0.5.x; dropping the field would silently discard what those
/// laws declare and disable the RFC-012 taint modes that run off it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UntranslatableEntry {
    /// The legal construct that cannot be translated
    pub construct: String,
    /// Why this construct is untranslatable
    pub reason: String,
    /// Suggested engine operation or approach to resolve this
    #[serde(default)]
    pub suggestion: Option<String>,
    /// Relevant excerpt from the article's legal text
    #[serde(default)]
    pub legal_text_excerpt: Option<String>,
    /// Whether a human has reviewed and acknowledged this gap
    #[serde(default)]
    pub accepted: bool,
}

/// Machine-readable section of an article
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MachineReadable {
    /// Named endpoint for this article, making it callable from other regulations
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub definitions: Option<HashMap<String, Definition>>,
    #[serde(default)]
    pub execution: Option<Execution>,
    /// Dependencies on other articles, regelingen or external sources
    #[serde(default)]
    pub requires: Option<Vec<ArticleRequirement>>,
    #[serde(default)]
    pub competent_authority: Option<CompetentAuthority>,
    /// Open terms that can or must be filled by implementing regulations
    #[serde(default)]
    pub open_terms: Option<Vec<OpenTerm>>,
    /// Declares which open terms from higher-level laws this article fills
    #[serde(default)]
    pub implements: Option<Vec<ImplementsDeclaration>>,
    /// Hook declarations: this article fires when matching lifecycle events occur (RFC-007)
    #[serde(default)]
    pub hooks: Option<Vec<HookDeclaration>>,
    /// Override declarations: this article replaces another article's output (RFC-007)
    #[serde(default)]
    pub overrides: Option<Vec<OverrideDeclaration>>,
    /// Legal constructs that cannot be expressed with the current operation set (RFC-012).
    /// Superseded by [`MachineReadable::markings`] in schema v0.7.0.
    #[serde(default)]
    pub untranslatables: Option<Vec<UntranslatableEntry>>,
    /// Constructs that the format itself cannot express (schema v0.7.0)
    #[serde(default)]
    pub markings: Option<Vec<Marking>>,
    /// Document properties this article establishes (schema v0.7.0)
    #[serde(default)]
    pub declares: Option<Vec<Declaration>>,
}

/// Represents a single article in a law
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Article {
    pub number: String,
    pub text: String,
    /// URL to the official source (also supports 'ref' for backward compatibility)
    #[serde(default, alias = "ref")]
    pub url: Option<String>,
    /// The containers that enclose this article, each with its number and
    /// opschrift (schema v0.7.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    #[serde(default)]
    pub machine_readable: Option<MachineReadable>,
    /// Structured references to other laws/articles for runtime resolution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<ArticleReference>>,
}

impl Article {
    /// Extract execution specification from machine_readable section
    pub fn get_execution_spec(&self) -> Option<&Execution> {
        self.machine_readable.as_ref()?.execution.as_ref()
    }

    /// Get definitions from this article.
    ///
    /// Returns a reference to avoid unnecessary allocations.
    pub fn get_definitions(&self) -> Option<&HashMap<String, Definition>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.definitions.as_ref())
    }

    /// Get the declared dependencies of this article
    pub fn get_requires(&self) -> &[ArticleRequirement] {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.requires.as_deref())
            .unwrap_or_default()
    }

    /// Get all output names from this article - these are the public endpoints
    pub fn get_output_names(&self) -> Vec<&str> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
            .and_then(|exec| exec.output.as_ref())
            .map(|outputs| outputs.iter().map(|o| o.name.as_str()).collect())
            .unwrap_or_default()
    }

    /// The fact named `fact` this article requires to exist (schema v0.8.7,
    /// RFC-051), as its `requires` entry.
    pub fn required_fact(&self, fact: &str) -> Option<&ArticleRequirement> {
        self.get_requires()
            .iter()
            .find(|r| r.fact.as_deref() == Some(fact))
    }

    /// Check if this article produces a specific output (allocation-free).
    ///
    /// More efficient than `get_output_names().contains(&name)` as it
    /// doesn't allocate a Vec.
    pub fn has_output(&self, output_name: &str) -> bool {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
            .and_then(|exec| exec.output.as_ref())
            .is_some_and(|outputs| outputs.iter().any(|o| o.name == output_name))
    }

    /// Check if this article is publicly callable (has outputs)
    pub fn is_public(&self) -> bool {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
            .and_then(|exec| exec.output.as_ref())
            .is_some_and(|outputs| !outputs.is_empty())
    }

    /// Get inputs from this article's execution spec.
    pub fn get_inputs(&self) -> &[Input] {
        self.get_execution_spec()
            .and_then(|exec| exec.input.as_deref())
            .unwrap_or(&[])
    }

    /// Get parameters from this article's execution spec.
    pub fn get_parameters(&self) -> &[Parameter] {
        self.get_execution_spec()
            .and_then(|exec| exec.parameters.as_deref())
            .unwrap_or(&[])
    }

    /// Get outputs from this article's execution spec.
    pub fn get_outputs(&self) -> &[Output] {
        self.get_execution_spec()
            .and_then(|exec| exec.output.as_deref())
            .unwrap_or(&[])
    }

    /// The declaration of the output named `name`, if this article declares it.
    pub fn find_output(&self, name: &str) -> Option<&Output> {
        self.get_outputs().iter().find(|o| o.name == name)
    }

    /// The declaration of the parameter named `name`, if this article declares it.
    pub fn find_parameter(&self, name: &str) -> Option<&Parameter> {
        self.get_parameters().iter().find(|p| p.name == name)
    }

    /// Get open terms declared by this article.
    pub fn get_open_terms(&self) -> Option<&Vec<OpenTerm>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.open_terms.as_ref())
    }

    /// Get implements declarations from this article.
    pub fn get_implements(&self) -> Option<&Vec<ImplementsDeclaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.implements.as_ref())
    }

    /// Get hook declarations from this article.
    pub fn get_hooks(&self) -> Option<&Vec<HookDeclaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.hooks.as_ref())
    }

    /// Get override declarations from this article.
    pub fn get_overrides(&self) -> Option<&Vec<OverrideDeclaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.overrides.as_ref())
    }

    /// Get the markings declared by this article (schema v0.7.0).
    ///
    /// The engine reads these beside `untranslatables`: a marking drives the
    /// four RFC-012 modes exactly as an untranslatable entry does.
    pub fn get_markings(&self) -> Option<&Vec<Marking>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.markings.as_ref())
    }

    /// Get the document properties this article declares (schema v0.7.0).
    pub fn get_declares(&self) -> Option<&Vec<Declaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.declares.as_ref())
    }

    /// Everything this article produces, in the order written: one item for
    /// the object form of `produces`, none when it declares nothing.
    pub fn get_produced(&self) -> Vec<Produced<'_>> {
        self.get_execution_spec()
            .and_then(|exec| exec.produces.as_ref())
            .map(ProducesList::items)
            .unwrap_or_default()
    }

    /// Whether `produces` is written as a list (schema v0.8.1), so its
    /// outputs and parameters are divided over the items.
    pub fn produces_list(&self) -> bool {
        matches!(
            self.get_execution_spec().and_then(|e| e.produces.as_ref()),
            Some(ProducesList::Many(_))
        )
    }

    /// What this article produces with a legal character, such as the
    /// decision on an application: the first such item.
    pub fn get_decision(&self) -> Option<Produced<'_>> {
        self.get_produced()
            .into_iter()
            .find(|p| p.legal_character().is_some())
    }

    /// The submission this article establishes (RFC-045), as the item that
    /// carries it.
    pub fn get_submission(&self) -> Option<Produced<'_>> {
        self.get_produced()
            .into_iter()
            .find(|p| p.submission_kind().is_some())
    }

    /// The article whose decision the submission of this article asks for,
    /// `<regulation>#<article>`: the one its item names with `requests`
    /// (RFC-045 §1), or `own`, this article, in the object form when that also
    /// takes a decision. `None` when it establishes no submission or its
    /// submission names no decision. Whether that article is there and takes
    /// a decision, only the corpus it runs in can say.
    pub fn requested_decision(&self, own: &str) -> Option<String> {
        let submission = self.get_submission()?;
        match submission.requests() {
            Some(target) => Some(target.to_string()),
            None if !self.produces_list() && self.get_decision().is_some() => Some(own.to_string()),
            None => None,
        }
    }

    /// The names of the outputs that belong to `item`, one of
    /// [`Self::get_produced`]: those it lists under `outputs`. An item that
    /// lists none yields nothing if it is a submission, and if it is the
    /// decision every output of the article that no other item lists and
    /// that it does not list as derived. For the object form that is every
    /// output.
    pub fn outputs_of<'a>(&'a self, item: Produced<'a>) -> Vec<&'a str> {
        if let Some(listed) = item.listed_outputs() {
            return listed.iter().map(String::as_str).collect();
        }
        if matches!(item, Produced::Item(i) if i.is_submission()) {
            return Vec::new();
        }
        let produced = self.get_produced();
        self.get_outputs()
            .iter()
            .map(|o| o.name.as_str())
            .filter(|name| {
                let in_list = |l: &[String]| l.iter().any(|n| n == name);
                !item.listed_derived().is_some_and(in_list)
                    && !produced.iter().any(|other| {
                        !other.same_as(item) && other.listed_outputs().is_some_and(in_list)
                    })
            })
            .collect()
    }

    /// The outputs the decision lists as data derived for it, `derived`
    /// (schema v0.8.4, RFC-045): computed at the decision's stages and
    /// recorded with it, but not among its outputs.
    pub fn derived_outputs(&self) -> Vec<&str> {
        self.get_decision()
            .and_then(Produced::listed_derived)
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect()
    }

    /// The names of the parameters that belong to `item`: in the list form
    /// those it lists under `parameters` (none when it lists none); for the
    /// object form every parameter of the article.
    pub fn parameters_of<'a>(&'a self, item: Produced<'a>) -> Vec<&'a str> {
        match item {
            Produced::Item(i) => i.parameters.iter().flatten().map(String::as_str).collect(),
            Produced::Object(_) => self
                .get_parameters()
                .iter()
                .map(|p| p.name.as_str())
                .collect(),
        }
    }
}

/// Aanhef of a law: the text that precedes article 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preamble {
    /// Preamble text in markdown, as published
    pub text: String,
    /// URL to the preamble in the official publication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The schema allows the aanhef its own machine-readable section
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_readable: Option<MachineReadable>,
}

/// Represents an article-based law document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleBasedLaw {
    /// JSON Schema URL
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    /// Law identifier (slug for referencing).
    ///
    /// The schema does not list `$id` as required, so a schema-valid document
    /// may omit it and the model must still load it (completeness). An empty
    /// id means "unnamed law": it can be executed directly but nothing can
    /// reference it.
    #[serde(rename = "$id", default)]
    pub id: String,
    /// Unique UUID
    #[serde(default)]
    pub uuid: Option<String>,
    /// Regulatory layer type
    pub regulatory_layer: RegulatoryLayer,
    /// Publication date
    pub publication_date: String,
    /// Date from which law is valid
    #[serde(default)]
    pub valid_from: Option<String>,
    /// Last date on which the law is in force (inclusive). Set only when the law is
    /// terminated (it vervalt or wordt ingetrokken — a horizonbepaling or repeal by
    /// another instrument); absent means valid indefinitely until superseded by a
    /// newer version. Not about whether a successor exists. See RFC-019.
    #[serde(default)]
    pub valid_to: Option<String>,
    /// Law name (can be a reference like "#wet_naam")
    #[serde(default)]
    pub name: Option<String>,
    /// Competent authority
    #[serde(default)]
    pub competent_authority: Option<CompetentAuthority>,
    /// BWB identifier for national laws
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// URL to official source
    #[serde(default)]
    pub url: Option<String>,
    /// Additional identifiers
    #[serde(default)]
    pub identifiers: Option<HashMap<String, String>>,
    /// Municipality code for gemeentelijke verordeningen
    #[serde(default)]
    pub gemeente_code: Option<String>,
    /// CBS province code for provinciale verordeningen
    #[serde(default)]
    pub provincie_code: Option<String>,
    /// Water board code for waterschapsverordeningen
    #[serde(default)]
    pub waterschap_code: Option<String>,
    /// CELEX number, required by the schema for `EU_VERORDENING`/`EU_RICHTLIJN`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub celex_nummer: Option<String>,
    /// European Legislation Identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eli: Option<String>,
    /// Tractatenblad identifier, required by the schema for `VERDRAG`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tractatenblad_id: Option<String>,
    /// UN Treaty Series number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unts_nummer: Option<String>,
    /// Staatscourant identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stcrt_id: Option<String>,
    /// Issuing organisation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation: Option<String>,
    /// Official title for local regulations
    #[serde(default)]
    pub officiele_titel: Option<String>,
    /// Year for versioned regulations (e.g., tariffs)
    #[serde(default)]
    pub jaar: Option<i32>,
    /// Legal basis references
    #[serde(default)]
    pub legal_basis: Option<Vec<LegalBasis>>,
    /// AWB-defined procedure lifecycles (RFC-008)
    #[serde(default)]
    pub procedure: Option<Vec<ProcedureDefinition>>,
    /// Aanhef preceding article 1. Carries the "Gelet op"-chain, and the schema
    /// allows it a `machine_readable` section of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble: Option<Preamble>,
    /// Articles in the law
    #[serde(default)]
    pub articles: Vec<Article>,
    /// SHA-256 hash of the source YAML, for provenance (RFC-013).
    ///
    /// This is the one runtime-populated field on the document model: it is left
    /// empty by plain deserialization and filled in by the engine's loader
    /// (`regelrecht_engine::article::LawLoad`) at parse time. It lives on the
    /// document (rather than an engine-side wrapper) so the hash travels with the
    /// law value through every consumer, and is `#[serde(skip)]` so it never
    /// round-trips into the YAML.
    #[serde(skip)]
    pub content_hash: Option<String>,
}

impl ArticleBasedLaw {
    /// Extract schema version (e.g., "v0.5.0") from the `$schema` URL.
    ///
    /// Looks for a `/vN.N.N` pattern (semver with v prefix) in the URL,
    /// skipping false matches like `/vendor/` or `/riva/`.
    pub fn schema_version(&self) -> Option<&str> {
        let url = self.schema.as_deref()?;
        let mut search_from = 0;
        loop {
            let pos = url[search_from..].find("/v")?;
            let abs_pos = search_from + pos;
            let version_start = abs_pos + 1;
            let rest = &url[version_start..];
            let end = rest.find('/').unwrap_or(rest.len());
            let candidate = &rest[..end];
            if candidate.starts_with('v') && Self::is_semver(&candidate[1..]) {
                return Some(candidate);
            }
            search_from = abs_pos + 2;
            if search_from >= url.len() {
                return None;
            }
        }
    }

    /// Check if a string looks like a semver version (N.N.N).
    fn is_semver(s: &str) -> bool {
        let mut parts = s.split('.');
        let valid = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
        matches!((parts.next(), parts.next(), parts.next(), parts.next()),
            (Some(a), Some(b), Some(c), None) if valid(a) && valid(b) && valid(c))
    }

    /// Find article that produces the given output.
    ///
    /// Uses allocation-free search via `Article::has_output()`.
    pub fn find_article_by_output(&self, output_name: &str) -> Option<&Article> {
        self.articles
            .iter()
            .find(|article| article.has_output(output_name))
    }

    /// Find article by article number
    pub fn find_article_by_number(&self, number: &str) -> Option<&Article> {
        self.articles
            .iter()
            .find(|article| article.number == number)
    }

    /// Get mapping of output names to articles
    pub fn get_all_outputs(&self) -> HashMap<String, &Article> {
        let mut outputs = HashMap::new();
        for article in &self.articles {
            for output_name in article.get_output_names() {
                outputs.insert(output_name.to_string(), article);
            }
        }
        outputs
    }

    /// Get all publicly callable articles
    pub fn get_public_articles(&self) -> Vec<&Article> {
        self.articles.iter().filter(|art| art.is_public()).collect()
    }
}
