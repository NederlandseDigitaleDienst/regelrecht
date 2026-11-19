//! The standard reduction of a fact: an answer a cell gives from a fact it
//! records, without a reduction written for it.
//!
//! A reduction that only copies the fields of a fact into its state, and a
//! read model that gives its aggregates as they are, follow from the fact's
//! declaration alone. The catalogue (`cel.yaml`, `answers`) may then name the
//! fact instead of a reduction:
//!
//! ```yaml
//! answers:
//!   huren_van_bewoner: test_register#4#plaats_gehuurd
//!   # or, with the fields of the root a binding selects on:
//!   huren_van_bewoner:
//!     fact: test_register#4#plaats_gehuurd
//!     where: [bewoner]
//! ```
//!
//! The cell writes that out to the full form ([`StandardAnswer::versions`]),
//! a `REDUCTIE` in the law format with two articles, and the service loads it
//! like any other reduction. Article [`AGGREGATE`] plays one aggregate per
//! record of the fact (`projection.per`): its state holds every field of the
//! fact, with the type the article declares, and its one step copies every
//! field into the property of the same name. The article named after the
//! answer reads the aggregates (`source.projections`, filtered on the root's
//! fields `where` names, each a parameter the binding passes) and gives them
//! as rows: one row per record. There is no second way of running:
//! revocation, the calculation date and replaying behave as for a reduction
//! written out by hand, because it is one.
//!
//! What a record of another fact that refers to the root does with its row,
//! such as a correction, is a reading of the chronicle. It belongs written
//! out in a reduction; the standard reduction has no step for it, so such a
//! record sets no field of the row. An effect it has, such as revoking the
//! root, holds as in any reduction.

use chrono::NaiveDate;
use regelrecht_engine::LawExecutionService;
use regelrecht_law_model::{ArticleBasedLaw, is_fact_reference};
use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value as Yaml};

use crate::error::{Result, setup};
use crate::shape::{Field, fact_fields, required_fact};

/// The article of a standard reduction that plays the aggregates.
pub const AGGREGATE: &str = "aggregaat";

/// The input under which the answering article of a standard reduction
/// reads the aggregates.
const ROWS: &str = "standaard_aggregaten";

/// The schema a standard reduction is written out in: the one that has
/// every construct it uses (`per` naming a fact, a projection named by its
/// article, `nullable` and `columns` on the state).
const SCHEMA: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json";

/// The long form of an answer from a fact in the catalogue.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FactEntry {
    fact: String,
    #[serde(default, rename = "where")]
    where_: Vec<String>,
}

/// What the catalogue says for one answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A reduction the cell keeps, by its `$id`.
    Reduction(String),
    /// A fact, answered by its standard reduction.
    Standard(StandardAnswer),
}

/// What the catalogue's `entry` for `answer` names: a reduction of the cell
/// by its `$id`, or a fact (`<regulation>#<article>#<fact>`), alone or as a
/// map with the fields a binding selects on (`where`).
///
/// # Errors
///
/// When a name holds `#` but is no fact reference, when the map holds an
/// unknown key or misses `fact`, and when the entry is neither a name nor a
/// map.
pub(crate) fn source(answer: &str, entry: serde_yaml_ng::Value) -> Result<Source> {
    let standard = |entry: FactEntry| {
        Source::Standard(StandardAnswer {
            answer: answer.to_string(),
            fact: entry.fact,
            where_: entry.where_,
        })
    };
    match entry {
        Yaml::String(name) if is_fact_reference(&name) => Ok(standard(FactEntry {
            fact: name,
            where_: Vec::new(),
        })),
        Yaml::String(name) if name.contains('#') => Err(setup(format!(
            "the answer '{answer}' names '{name}', which is neither the $id of a reduction nor \
             a fact <regulation>#<article>#<fact>"
        ))),
        Yaml::String(name) => Ok(Source::Reduction(name)),
        entry @ Yaml::Mapping(_) => serde_yaml_ng::from_value(entry)
            .map(standard)
            .map_err(|e| setup(format!("the answer '{answer}' names a fact: {e}"))),
        _ => Err(setup(format!(
            "the answer '{answer}' names neither a reduction nor a fact"
        ))),
    }
}

/// One version of a standard reduction, written out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// The day it is valid from: that of the version of the root's
    /// regulation it is written out from.
    pub valid_from: NaiveDate,
    /// The reduction in the law format.
    pub yaml: String,
}

/// An answer the cell gives from a fact, through the standard reduction
/// ([`StandardAnswer::versions`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardAnswer {
    /// The name of the answer, which is also the output that gives it.
    pub answer: String,
    /// The fact whose records are the roots, `<regulation>#<article>#<fact>`.
    pub fact: String,
    /// The fields of the root a binding selects the aggregates on: each a
    /// parameter of the answer.
    pub where_: Vec<String>,
}

impl StandardAnswer {
    /// Refuse an entry that names no fact, a field to select on twice, or a
    /// name the standard reduction gives its own article or input.
    pub(crate) fn check(&self) -> Result<()> {
        let fail = |why: String| {
            Err(setup(format!(
                "the answer '{}' names the fact '{}': {why}",
                self.answer, self.fact
            )))
        };
        if !is_fact_reference(&self.fact) {
            return fail("it is no <regulation>#<article>#<fact>".to_string());
        }
        if let Some(twice) = first_repeated(self.where_.iter().map(String::as_str)) {
            return fail(format!("where names '{twice}' twice"));
        }
        // The answer names an output and an article of the written-out
        // reduction, and part of its $id.
        if self.answer.is_empty()
            || !self
                .answer
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return fail(format!(
                "the name '{}' holds a character other than a lowercase letter, a digit or _",
                self.answer
            ));
        }
        // The written-out reduction names an article and an input itself.
        if [AGGREGATE, ROWS].contains(&self.answer.as_str()) {
            return fail(format!(
                "the standard reduction names its own article or input '{}'",
                self.answer
            ));
        }
        if self.where_.iter().any(|name| name == ROWS) {
            return fail(format!(
                "where names '{ROWS}', the input the standard reduction reads its aggregates with"
            ));
        }
        Ok(())
    }

    /// The `$id` of the standard reduction of this answer in the cell of
    /// `recording_actor`: one per cell, so two cells of a sphere that both
    /// answer it from a fact do not share one.
    pub fn reduction_id(&self, recording_actor: &str) -> String {
        let actor: String = recording_actor
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        format!("standaard_{actor}_{}", self.answer)
    }

    /// The regulation whose fact the standard reduction reads.
    pub fn regulation(&self) -> &str {
        self.fact.split_once('#').map_or(&self.fact, |(law, _)| law)
    }

    /// The standard reduction `id` written out in full, as YAML: one
    /// version per version of the fact's regulation in `service` that
    /// requires the fact, valid from the same day, with the fields the fact
    /// holds in it. A version that no longer requires the fact adds no
    /// version: the one before stays in force, and plays the records
    /// already in the chronicle.
    ///
    /// # Errors
    ///
    /// When no version of the regulation requires the fact, when the fact
    /// holds a field that is no parameter or that a row gives itself, and
    /// when `where` names a field the fact does not hold.
    pub fn versions(&self, service: &LawExecutionService, id: &str) -> Result<Vec<Version>> {
        let regulation = self.regulation();
        let mut days: Vec<(NaiveDate, String)> = service
            .resolver()
            .all_law_versions()
            .filter(|law| law.id == regulation)
            .filter_map(|law| {
                let from = law.valid_from.as_deref()?;
                let day = NaiveDate::parse_from_str(from, "%Y-%m-%d").ok()?;
                Some((day, law.publication_date.clone()))
            })
            .collect();
        days.sort();
        days.dedup_by_key(|(day, _)| *day);
        let mut out = Vec::new();
        for (day, published) in days {
            // A version that does not require the fact has no records of it
            // to play.
            if required_fact(service, &self.fact, day).is_err() {
                continue;
            }
            let fields = fact_fields(service, &self.fact, day, None)?;
            let law = self.written_out(id, day, &published, &fields)?;
            out.push(Version {
                valid_from: day,
                yaml: serde_yaml_ng::to_string(&law)
                    .map_err(|e| setup(format!("the standard reduction '{id}': {e}")))?,
            });
        }
        if out.is_empty() {
            return Err(setup(format!(
                "the answer '{}' names the fact '{}', which no loaded version of \
                 '{regulation}' requires",
                self.answer, self.fact
            )));
        }
        Ok(out)
    }

    /// One version of the standard reduction `id`, valid from `day`, in
    /// the order a reduction written by hand gives its keys.
    fn written_out(
        &self,
        id: &str,
        day: NaiveDate,
        published: &str,
        fields: &[Field],
    ) -> Result<Yaml> {
        let state = fields.iter().map(typed).collect::<Result<Vec<_>>>()?;
        let answering = self.answering_article(id, fields)?;
        let aggregate = map([
            ("number", text(AGGREGATE)),
            (
                "text",
                text(&format!(
                    "Geen regelgeving. De standaardreductie van de cel: per vastgelegd feit {} \
                     de velden die het draagt.",
                    self.fact
                )),
            ),
            (
                "machine_readable",
                map([(
                    "projection",
                    map([
                        ("per", text(&self.fact)),
                        ("state", Yaml::Sequence(state)),
                        ("steps", Yaml::Sequence(vec![copy_step(&self.fact, fields)])),
                    ]),
                )]),
            ),
        ]);
        Ok(map([
            ("$schema", text(SCHEMA)),
            ("$id", text(id)),
            ("regulatory_layer", text("REDUCTIE")),
            ("publication_date", text(published)),
            ("valid_from", text(&day.format("%Y-%m-%d").to_string())),
            (
                "name",
                text(&format!("Standaardreductie van {}", self.answer)),
            ),
            ("articles", Yaml::Sequence(vec![aggregate, answering])),
        ]))
    }

    /// The article named after the answer: it reads the aggregates of
    /// article [`AGGREGATE`] of `id`, of the roots whose fields `where`
    /// names equal the parameters of the same name, and gives them as rows.
    fn answering_article(&self, id: &str, root: &[Field]) -> Result<Yaml> {
        let parameters = self
            .where_
            .iter()
            .map(|name| {
                let field = root.iter().find(|f| &f.name == name).ok_or_else(|| {
                    setup(format!(
                        "the answer '{}' selects on '{name}', which the fact '{}' does not hold",
                        self.answer, self.fact
                    ))
                })?;
                typed(field)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut source = map([("projections", text(&format!("{id}#{AGGREGATE}")))]);
        if !self.where_.is_empty() {
            let select = self
                .where_
                .iter()
                .map(|name| (text(name), text(&format!("${name}"))))
                .collect();
            insert(&mut source, "where", Yaml::Mapping(select));
        }
        let mut execution = Mapping::new();
        if !parameters.is_empty() {
            execution.insert(text("parameters"), Yaml::Sequence(parameters));
        }
        let rows = map([
            ("name", text(ROWS)),
            ("type", text("array")),
            ("source", source),
        ]);
        execution.insert(text("input"), Yaml::Sequence(vec![rows]));
        execution.insert(
            text("output"),
            Yaml::Sequence(vec![map([
                ("name", text(&self.answer)),
                ("type", text("array")),
            ])]),
        );
        execution.insert(
            text("actions"),
            Yaml::Sequence(vec![map([
                ("output", text(&self.answer)),
                ("value", text(&format!("${ROWS}"))),
            ])]),
        );
        Ok(map([
            ("number", text(&self.answer)),
            (
                "text",
                text(&format!(
                    "Geen regelgeving. De aggregaten van artikel {AGGREGATE}, als rijen."
                )),
            ),
            (
                "machine_readable",
                map([("execution", Yaml::Mapping(execution))]),
            ),
        ]))
    }
}

/// Load into `service` every version of the standard reduction of each of
/// `answers`, each with its `$id`, after the regulations whose facts they
/// read. A reduction the service already holds under that `$id` stays, when
/// it is this one, loaded before.
///
/// # Errors
///
/// When a standard reduction cannot be written out
/// ([`StandardAnswer::versions`]), when the service refuses it, and when
/// the service holds another rulework under its `$id`, such as the standard
/// reduction of another cell whose name gives the same `$id`.
pub fn load<'a>(
    service: &mut LawExecutionService,
    answers: impl IntoIterator<Item = (&'a StandardAnswer, String)>,
) -> Result<()> {
    for (fact, id) in answers {
        let versions = fact.versions(service, &id)?;
        if service.has_law(&id) {
            for version in &versions {
                let written: ArticleBasedLaw = serde_yaml_ng::from_str(&version.yaml)
                    .map_err(|e| setup(format!("the standard reduction '{id}': {e}")))?;
                let held = service
                    .resolver()
                    .get_law_for_date(&id, Some(version.valid_from));
                if held.is_none_or(|held| held.articles != written.articles) {
                    return Err(setup(format!(
                        "the answer '{}' comes from the standard reduction '{id}', and the \
                         service holds another rulework under that $id",
                        fact.answer
                    )));
                }
            }
            continue;
        }
        for version in versions {
            service.load_law(&version.yaml)?;
        }
    }
    Ok(())
}

/// A parameter or a property of the state with the name, the type, whether
/// it may hold null and the columns of `field`.
fn typed(field: &Field) -> Result<Yaml> {
    let yaml = |value: std::result::Result<Yaml, serde_yaml_ng::Error>| {
        value.map_err(|e| setup(format!("the field '{}': {e}", field.name)))
    };
    let mut out = map([
        ("name", text(&field.name)),
        ("type", yaml(serde_yaml_ng::to_value(field.type_))?),
    ]);
    if field.nullable {
        insert(&mut out, "nullable", Yaml::Bool(true));
    }
    if let Some(columns) = &field.columns {
        insert(&mut out, "columns", yaml(serde_yaml_ng::to_value(columns))?);
    }
    Ok(out)
}

/// The step of a record of `fact`: every field it holds, copied into the
/// property of the same name.
fn copy_step(fact: &str, fields: &[Field]) -> Yaml {
    let actions = fields
        .iter()
        .map(|f| {
            map([
                ("output", text(&f.name)),
                ("value", text(&format!("$record.{}", f.name))),
            ])
        })
        .collect();
    map([("record", text(fact)), ("actions", Yaml::Sequence(actions))])
}

/// A YAML string.
fn text(s: &str) -> Yaml {
    Yaml::String(s.to_string())
}

/// A YAML mapping with `pairs`, in their order.
fn map<const N: usize>(pairs: [(&str, Yaml); N]) -> Yaml {
    Yaml::Mapping(pairs.into_iter().map(|(k, v)| (text(k), v)).collect())
}

/// Add `key` with `value` to the mapping `to`; another value is left as it is.
fn insert(to: &mut Yaml, key: &str, value: Yaml) {
    if let Yaml::Mapping(m) = to {
        m.insert(text(key), value);
    }
}

/// The first item `items` holds twice.
fn first_repeated<'a>(mut items: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen = std::collections::BTreeSet::new();
    items.find(|item| !seen.insert(*item))
}
