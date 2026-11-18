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
//!   # or, with facts that refer to it and the roots a binding selects:
//!   huren_van_bewoner:
//!     fact: test_register#4#plaats_gehuurd
//!     referred_by: [test_register#4#huur_opgezegd]
//!     where: [bewoner]
//! ```
//!
//! The cell writes that out to the full form ([`StandardAnswer::versions`]),
//! a `REDUCTIE` in the law format with two articles, and the service loads it
//! like any other reduction. Article [`AGGREGATE`] plays one aggregate per
//! record of the fact (`projection.per`): its state holds every field of the
//! fact, and of each fact that refers to it, with the type the article
//! declares; the step of each fact copies every field it holds into the
//! property of the same name. A referring fact, such as a correction, so
//! overwrites what it holds under the same name and adds what only it holds.
//! The article named after the answer reads the aggregates
//! (`source.projections`, filtered on the root's fields `where` names, each a
//! parameter the binding passes) and gives them as rows. There is no second
//! way of running: revocation, the calculation date and replaying behave as
//! for a reduction written out by hand, because it is one.
//!
//! Which facts refer to the root, the declarations do not say: a fact's
//! `refers_to` is a property of each record, not of the fact. So the
//! catalogue lists them (`referred_by`).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::LawExecutionService;
use regelrecht_law_model::is_fact_reference;
use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value as Yaml};

use crate::error::{Result, setup};
use crate::shape::{Field, fact_fields, split_fact};

/// The article of a standard reduction that plays the aggregates.
pub const AGGREGATE: &str = "aggregaat";

/// The input under which the answering article of a standard reduction
/// reads the aggregates.
const ROWS: &str = "standaard_aggregaten";

/// The schema a standard reduction is written out in: the one that has
/// every construct it uses (`per` naming a fact, a projection named by its
/// article, `nullable` and `columns` on the state).
const SCHEMA: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json";

/// How the catalogue names where an answer comes from: a reduction of the
/// cell by its `$id`, or a fact (`<regulation>#<article>#<fact>`), alone or
/// with the facts that refer to it and the fields a binding selects on.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Entry {
    Named(String),
    Fact(FactEntry),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FactEntry {
    fact: String,
    #[serde(default)]
    referred_by: Vec<String>,
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

impl Entry {
    /// What the entry for `answer` names. A name that is a fact reference
    /// names a fact; any other names a reduction.
    pub(crate) fn source(self, answer: &str) -> Source {
        match self {
            Entry::Named(name) if is_fact_reference(&name) => Source::Standard(StandardAnswer {
                answer: answer.to_string(),
                fact: name,
                referred_by: Vec::new(),
                where_: Vec::new(),
            }),
            Entry::Named(name) => Source::Reduction(name),
            Entry::Fact(entry) => Source::Standard(StandardAnswer {
                answer: answer.to_string(),
                fact: entry.fact,
                referred_by: entry.referred_by,
                where_: entry.where_,
            }),
        }
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
    /// The facts whose records refer to a root and set the fields they hold
    /// under the same name.
    pub referred_by: Vec<String>,
    /// The fields of the root a binding selects the aggregates on: each a
    /// parameter of the answer.
    pub where_: Vec<String>,
}

impl StandardAnswer {
    /// Refuse an entry that names no fact, a referring fact twice or the
    /// root among them, or a field to select on twice.
    pub(crate) fn check(&self) -> Result<()> {
        let fail = |why: String| {
            Err(setup(format!(
                "the answer '{}' names the fact '{}': {why}",
                self.answer, self.fact
            )))
        };
        for fact in std::iter::once(&self.fact).chain(&self.referred_by) {
            if !is_fact_reference(fact) {
                return fail(format!("'{fact}' is no <regulation>#<article>#<fact>"));
            }
        }
        if let Some(twice) = first_repeated(
            std::iter::once(&self.fact)
                .chain(&self.referred_by)
                .map(String::as_str),
        ) {
            return fail(format!("it names the fact '{twice}' twice"));
        }
        if let Some(twice) = first_repeated(self.where_.iter().map(String::as_str)) {
            return fail(format!("where names '{twice}' twice"));
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

    /// The regulations whose facts the standard reduction reads, root first.
    pub fn regulations(&self) -> impl Iterator<Item = &str> {
        std::iter::once(&self.fact)
            .chain(&self.referred_by)
            .filter_map(|fact| fact.split_once('#').map(|(law, _)| law))
    }

    /// The standard reduction `id` written out in full, as YAML: one
    /// version per version of the root's regulation in `service` that
    /// requires the fact, valid from the same day, with the fields each
    /// fact holds in the versions in force on that day.
    ///
    /// # Errors
    ///
    /// When no version of the root's regulation requires the fact, when a
    /// referring fact is not required on the day of a version, when two
    /// facts hold a field of the same name with another type or other
    /// columns, and when `where` names a field the root does not hold.
    pub fn versions(&self, service: &LawExecutionService, id: &str) -> Result<Vec<Version>> {
        let (basis, _) = split_fact(&self.fact)?;
        let root_law = basis.split_once('#').map_or(basis, |(law, _)| law);
        let mut days: Vec<(NaiveDate, String)> = service
            .resolver()
            .all_law_versions()
            .filter(|law| law.id == root_law)
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
            // A version of the regulation that does not yet require the
            // fact has no records of it to play.
            let Ok(root) = fact_fields(service, &self.fact, day, None) else {
                continue;
            };
            let referring = self
                .referred_by
                .iter()
                .map(|fact| Ok((fact.as_str(), fact_fields(service, fact, day, None)?)))
                .collect::<Result<Vec<_>>>()?;
            let law = self.written_out(id, day, &published, &root, &referring)?;
            out.push(Version {
                valid_from: day,
                yaml: serde_yaml_ng::to_string(&law)
                    .map_err(|e| setup(format!("the standard reduction '{id}': {e}")))?,
            });
        }
        if out.is_empty() {
            return Err(setup(format!(
                "the answer '{}' names the fact '{}', which no loaded version of '{root_law}' \
                 requires",
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
        root: &[Field],
        referring: &[(&str, Vec<Field>)],
    ) -> Result<Yaml> {
        let state = self.state(root, referring)?;
        let steps: Vec<Yaml> = std::iter::once((self.fact.as_str(), root))
            .chain(
                referring
                    .iter()
                    .map(|(fact, fields)| (*fact, fields.as_slice())),
            )
            .map(|(fact, fields)| copy_step(fact, fields))
            .collect();
        let answering = self.answering_article(id, root)?;
        let aggregate = map([
            ("number", text(AGGREGATE)),
            (
                "text",
                text(&format!(
                    "Geen regelgeving. De standaardreductie van de cel: per vastgelegd feit {} \
                     de velden die het draagt, met wat de feiten die ernaar verwijzen onder \
                     dezelfde naam vastleggen.",
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
                        ("steps", Yaml::Sequence(steps)),
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

    /// The state: every field of the root, then every field of a referring
    /// fact the root does not hold, in the order the facts declare them. A
    /// name two facts hold has one type and the same columns in both.
    fn state(&self, root: &[Field], referring: &[(&str, Vec<Field>)]) -> Result<Vec<Yaml>> {
        let mut seen: BTreeMap<&str, (&Field, &str)> = BTreeMap::new();
        let mut order: Vec<&Field> = Vec::new();
        let facts = std::iter::once((self.fact.as_str(), root)).chain(
            referring
                .iter()
                .map(|(fact, fields)| (*fact, fields.as_slice())),
        );
        for (fact, fields) in facts {
            for field in fields {
                match seen.get(field.name.as_str()) {
                    Some((held, by))
                        if held.type_ != field.type_ || held.columns != field.columns =>
                    {
                        return Err(setup(format!(
                            "the answer '{}': the facts {by} and {fact} both hold '{}', with \
                             another type or other columns; the standard reduction copies it \
                             into one property",
                            self.answer, field.name
                        )));
                    }
                    Some(_) => {}
                    None => {
                        seen.insert(&field.name, (field, fact));
                        order.push(field);
                    }
                }
            }
        }
        order.into_iter().map(typed).collect()
    }
}

/// Load into `service` every version of the standard reduction of each of
/// `answers`, each with its `$id`, after the regulations whose facts they
/// read. A reduction the service already holds stays as it is: its `$id`
/// names its cell, so it was loaded before.
///
/// # Errors
///
/// When a standard reduction cannot be written out
/// ([`StandardAnswer::versions`]), or the service refuses it.
pub fn load<'a>(
    service: &mut LawExecutionService,
    answers: impl IntoIterator<Item = (&'a StandardAnswer, String)>,
) -> Result<()> {
    for (fact, id) in answers {
        if service.has_law(&id) {
            continue;
        }
        for version in fact.versions(service, &id)? {
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
fn first_repeated<'a>(items: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen = std::collections::BTreeSet::new();
    items.into_iter().find(|item| !seen.insert(*item))
}
