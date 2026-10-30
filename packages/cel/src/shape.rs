//! The shape of an application's chronolexogram, derived from the law.
//!
//! The engine names the articles that take part in a submission
//! ([`LawExecutionService::submission`]): the article that establishes it and
//! the hooks on it. What they ask is what the application holds: the
//! parameters the submission item lists when `produces` is a list (RFC-045 §1),
//! every parameter of the establishing article in the object form, and every
//! parameter of each hook. But one: a `produces.moment` on one of them names
//! the moment that counts,
//! such as the receipt (Awb 4:13 lid 1). That parameter is the moment, not
//! part of the application (RFC-045); the cell fills it, not the applicant.
//! And what a hook on the submission outputs, such as whether an objection
//! came in on time (Awb 6:9), is what the submission gives, not what it
//! asks (RFC-045).

use chrono::NaiveDate;
use regelrecht_engine::{Article, Authority, LawExecutionService, ParameterType};
use regelrecht_law_model::{stored, ArticleRequirement};
use serde::Serialize;

use crate::error::{refused, setup, Result};

/// A parameter of an article that takes part in the submission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: ParameterType,
    /// Whether `null` (absence) is a value it can take (RFC-036): only when
    /// every article that asks it says so.
    pub nullable: bool,
    /// The articles that ask it, `<regulation>#<article>`, in the order they
    /// take part. Two articles that ask the same datum with the same type
    /// make one field, resting on both.
    pub declared_by: Vec<String>,
}

impl Field {
    /// The article that asks it first.
    pub fn first_declared_by(&self) -> &str {
        self.declared_by.first().map_or("", String::as_str)
    }
}

/// The moment that counts for the submission (`produces.moment`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Moment {
    /// The parameter that holds it, which the cell fills with the day it
    /// records the application. `None` when the law names no parameter: the
    /// moment is then the recording itself.
    pub parameter: Option<String>,
    /// The article that names the moment.
    pub declared_by: String,
}

/// What a chronolexogram of a submission holds, as the law in force on a day
/// says.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shape {
    /// The kind of submission, `produces.submission.kind` (`AANVRAAG`,
    /// `BEZWAAR`).
    pub kind: String,
    pub law_id: String,
    pub article: String,
    /// The outputs of the establishing article: executing them runs it.
    /// Empty when the article follows a procedure: its outputs are then the
    /// decision on the application (RFC-045 §4), and recording the application
    /// runs only the hooks, at [`Shape::stage`].
    pub outputs: Vec<String>,
    /// The stage of the article's procedure at which the hooks on the
    /// application fire, such as AANVRAAG; `None` outside a procedure.
    pub stage: Option<String>,
    /// What the application holds: the parameters of the submission (the
    /// item's `parameters` in the list form, every parameter of the
    /// establishing article in the object form), then those of the hooks in
    /// the order they fire. A name two articles declare is one field. The
    /// moment's parameter is not among them.
    pub fields: Vec<Field>,
    pub moment: Option<Moment>,
    /// The article whose decision the application asks for,
    /// `<regulation>#<article>` (`requests`, RFC-045 §1); `None` when the law
    /// names none.
    pub requests: Option<String>,
    /// The outputs of the hooks on the submission: what recording it gives.
    /// A parameter of that name is no field; a decision takes it from the
    /// chronolexogram's `outputs` (RFC-046).
    pub given: Vec<String>,
}

impl Shape {
    /// The establishing article, `<regulation>#<article>`.
    pub fn basis(&self) -> String {
        format!("{}#{}", self.law_id, self.article)
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Whether `name` is the parameter that holds the moment that counts.
    pub fn is_moment(&self, name: &str) -> bool {
        self.moment
            .as_ref()
            .and_then(|m| m.parameter.as_deref())
            .is_some_and(|p| p == name)
    }
}

/// Derive the shape of the submission that article `article` of `law_id`
/// establishes, in the law in force on `day`.
pub fn derive(
    service: &LawExecutionService,
    law_id: &str,
    article: &str,
    day: NaiveDate,
) -> Result<Shape> {
    let reference = format!("{law_id}#{article}");
    let submission = service
        .submission(law_id, article, &day.to_string())?
        .ok_or_else(|| {
            refused(format!(
                "{reference} establishes no submission (produces.submission)"
            ))
        })?;
    let mut shape = Shape {
        kind: submission.kind,
        law_id: law_id.to_string(),
        article: article.to_string(),
        outputs: Vec::new(),
        stage: submission.stage,
        fields: Vec::new(),
        moment: None,
        requests: service.requested_decision(law_id, article, &day.to_string())?,
        given: Vec::new(),
    };
    for part in &submission.articles {
        let declared_by = format!("{}#{}", part.law_id, part.article_number);
        let found = article_on(service, &part.law_id, &part.article_number, day)
            .ok_or_else(|| setup(format!("{declared_by} has no version in force on {day}")))?;
        let Some(execution) = found.get_execution_spec() else {
            continue;
        };
        if part.hook_point.is_some() {
            shape
                .given
                .extend(execution.output.iter().flatten().map(|o| o.name.clone()));
        }
        // The establishing article contributes what its submission
        // contains: in the list form of `produces` the item's own
        // `parameters`, in the object form every parameter (RFC-045 §1). A
        // hook contributes every parameter it declares.
        let item = found.get_submission().filter(|_| part.hook_point.is_none());
        if let Some(item) = item.filter(|_| shape.stage.is_none()) {
            shape.outputs = found
                .outputs_of(item)
                .into_iter()
                .map(str::to_string)
                .collect();
        }
        let asked: Option<Vec<&str>> = item.map(|i| found.parameters_of(i));
        for p in execution.parameters.iter().flatten() {
            if asked
                .as_ref()
                .is_some_and(|names| !names.contains(&p.name.as_str()))
            {
                continue;
            }
            let field = Field {
                name: p.name.clone(),
                type_: p.param_type,
                nullable: p.is_nullable(),
                declared_by: vec![declared_by.clone()],
            };
            add_field(&mut shape.fields, field, &reference)?;
        }
        if let Some(m) = found.get_produced().into_iter().find_map(|p| p.moment()) {
            if let Some(other) = &shape.moment {
                return Err(setup(format!(
                    "{reference}: two articles name the moment that counts ({} and {declared_by})",
                    other.declared_by
                )));
            }
            if let Some(name) = &m.parameter {
                match execution.parameters.iter().flatten().find(|p| &p.name == name) {
                    None => {
                        return Err(setup(format!(
                            "{declared_by}: the moment is parameter '{name}', which it does not declare"
                        )))
                    }
                    Some(p) if p.param_type != ParameterType::Date => {
                        return Err(setup(format!(
                            "{declared_by}: the moment is parameter '{name}', which is a {:?}, not a Date",
                            p.param_type
                        )))
                    }
                    Some(_) => {}
                }
            }
            shape.moment = Some(Moment {
                parameter: m.parameter.clone(),
                declared_by: declared_by.clone(),
            });
        }
    }
    // The moment is no part of what the application holds, and neither is
    // what a hook on it gives.
    if let Some(name) = shape.moment.as_ref().and_then(|m| m.parameter.clone()) {
        shape.fields.retain(|f| f.name != name);
    }
    let given = std::mem::take(&mut shape.given);
    shape.fields.retain(|f| !given.contains(&f.name));
    shape.given = given;
    if shape.outputs.is_empty() && shape.stage.is_none() {
        return Err(setup(format!(
            "{reference} establishes a submission but has no output to execute it by"
        )));
    }
    Ok(shape)
}

/// Add `field` to `fields`. Two articles that ask the same datum make one
/// field, resting on both; it can be absent only if both say so. A different
/// type is an error in the law: one chronolexogram cannot hold both.
pub(crate) fn add_field(fields: &mut Vec<Field>, field: Field, reference: &str) -> Result<()> {
    match fields.iter_mut().find(|f| f.name == field.name) {
        None => fields.push(field),
        Some(f) if f.type_ == field.type_ => {
            f.nullable &= field.nullable;
            for by in field.declared_by {
                if !f.declared_by.contains(&by) {
                    f.declared_by.push(by);
                }
            }
        }
        Some(f) => {
            return Err(setup(format!(
                "{reference}: '{}' is declared by {} and by {} with another type; \
                 one chronolexogram cannot hold both",
                field.name,
                f.first_declared_by(),
                field.first_declared_by()
            )))
        }
    }
    Ok(())
}

/// The article and the fact name of a fact reference
/// `<regulation>#<article>#<fact>` (schema v0.8.7, RFC-051).
pub fn split_fact(fact: &str) -> Result<(&str, &str)> {
    if !regelrecht_law_model::is_fact_reference(fact) {
        return Err(refused(format!(
            "'{fact}' is no <regulation>#<article>#<fact>"
        )));
    }
    fact.rsplit_once('#')
        .ok_or_else(|| refused(format!("'{fact}' is no <regulation>#<article>#<fact>")))
}

/// The fields of an executogram of the fact `fact`
/// (`<regulation>#<article>#<fact>`, schema v0.8.7, RFC-051), in the law in
/// force on `day`: the parameters the article's `requires` entry for that
/// fact lists, with their types, as the fields of an application are the
/// parameters its item lists. Refused when the article has no version in
/// force on `day` or does not require that fact.
///
/// With `recorder`, the cell of that authority is about to record an
/// instance: refused first when the fact names who establishes it
/// (`competent_authority`, schema v0.8.8, RFC-051) and does not name this
/// cell. A fact without one may be recorded by any cell.
pub fn fact_fields(
    service: &LawExecutionService,
    fact: &str,
    day: NaiveDate,
    recorder: Option<&Authority>,
) -> Result<Vec<Field>> {
    let (basis, _) = split_fact(fact)?;
    let (article, required) = required_fact(service, fact, day)?;
    if let Some(authority) = recorder {
        refuse_incompetent_fact(required, fact, authority)?;
    }
    required
        .parameters
        .iter()
        .flatten()
        .map(|field| {
            if [
                crate::reading::ROW_ID,
                crate::reading::ROW_BASIS,
                crate::reading::ROW_REFERS_TO,
            ]
            .contains(&field.as_str())
            {
                return Err(setup(format!(
                    "{fact}: the fact holds a field '{field}'; its row gives the executogram's \
                     own id, its article or what it refers to under that name"
                )));
            }
            let p = article
                .get_parameters()
                .iter()
                .find(|p| &p.name == field)
                .ok_or_else(|| {
                    setup(format!(
                        "{fact}: the fact holds '{field}', which is no parameter"
                    ))
                })?;
            Ok(Field {
                name: field.clone(),
                type_: p.param_type,
                nullable: p.is_nullable(),
                declared_by: vec![basis.to_string()],
            })
        })
        .collect()
}

/// The article of `fact` in force on `day` and its `requires` entry for
/// that fact. Refused when the article has no version in force on `day` or
/// does not require that fact.
fn required_fact<'s>(
    service: &'s LawExecutionService,
    fact: &str,
    day: NaiveDate,
) -> Result<(&'s Article, &'s ArticleRequirement)> {
    let (basis, name) = split_fact(fact)?;
    let (law_id, number) = basis
        .split_once('#')
        .ok_or_else(|| refused(format!("'{basis}' is no <regulation>#<article>")))?;
    let article = article_on(service, law_id, number, day)
        .ok_or_else(|| refused(format!("{basis} has no version in force on {day}")))?;
    let required = article.required_fact(name).ok_or_else(|| {
        refused(format!(
            "{basis} requires no fact '{name}' (requires, fact) on {day}"
        ))
    })?;
    Ok((article, required))
}

/// Refuse to record `fact` in the cell of `authority` when its `requires`
/// entry names who establishes it and does not name this cell.
fn refuse_incompetent_fact(
    required: &ArticleRequirement,
    fact: &str,
    authority: &Authority,
) -> Result<()> {
    let Some(named) = required
        .competent_authority
        .as_ref()
        .and_then(|competent| competent.named())
    else {
        return Ok(());
    };
    if authority.is_named_by(named) {
        return Ok(());
    }
    let names: Vec<String> = named.iter().map(|n| format!("'{}'", n.name)).collect();
    Err(refused(format!(
        "{fact} is established by {}, and the cell is '{}'{}",
        names.join(" or "),
        authority.name,
        authority
            .category
            .as_deref()
            .map(|c| format!(" ({c})"))
            .unwrap_or_default()
    )))
}

pub(crate) fn article_on<'s>(
    service: &'s LawExecutionService,
    law_id: &str,
    number: &str,
    day: NaiveDate,
) -> Option<&'s Article> {
    service
        .resolver()
        .get_law_for_date(law_id, Some(day))?
        .find_article_by_number(number)
}

/// Whether a supplied value fits the field. `null` fits a field that can
/// be absent (RFC-036): an application may leave it empty (Awb 4:5 asks to
/// complete it; it is still an application). An amount is in whole
/// eurocents. A date is written `YYYY-MM-DD`, and only so. A decision is a
/// decision value `{basis, fields, findings, outputs}` (schema
/// v0.8.4): an article reference and three objects, with `taken_on` where
/// the cell gave it ([`is_decision_value`]), and nothing else.
pub fn fits(field: &Field, value: &serde_json::Value) -> bool {
    use serde_json::Value as J;
    match (field.type_, value) {
        (_, J::Null) => field.nullable,
        (ParameterType::String, J::String(_)) => true,
        (ParameterType::Number, J::Number(_)) => true,
        // A decimal as the chronicle holds it ([`stored`]): only a
        // stored value, as one a decision holds; a supplied value that holds
        // the key is refused first.
        (ParameterType::Number, J::Object(_)) => matches!(
            stored::from_stored(value),
            Ok(regelrecht_engine::Value::Decimal(_))
        ),
        (ParameterType::Amount, J::Number(n)) => n.is_i64() || n.is_u64(),
        (ParameterType::Boolean, J::Bool(_)) => true,
        (ParameterType::Date, J::String(s)) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == *s),
        (ParameterType::Array, J::Array(_)) => true,
        (ParameterType::Object, J::Object(_)) => true,
        (ParameterType::Decision, J::Object(o)) => is_decision_value(o),
        _ => false,
    }
}

/// Whether `value` is a decision value `{basis, fields, findings,
/// outputs}`: the article that decided (`<regulation>#<article>`), and
/// three objects. A decision the cell gives from its chronicle also holds
/// `taken_on`: the article of the submission it was taken on, also
/// `<regulation>#<article>` (RFC-045 §6). A value an article gives
/// (`source.article`) holds no `taken_on`. Nothing else.
pub(crate) fn is_decision_value(value: &serde_json::Map<String, serde_json::Value>) -> bool {
    let reference = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(|g| g.split_once('#'))
            .is_some_and(|(law, article)| !law.is_empty() && !article.is_empty())
    };
    let taken_on = !value.contains_key("taken_on") || reference("taken_on");
    value.len() == 4 + usize::from(value.contains_key("taken_on"))
        && reference("basis")
        && taken_on
        && ["fields", "findings", "outputs"]
            .iter()
            .all(|key| value.get(*key).is_some_and(serde_json::Value::is_object))
}
