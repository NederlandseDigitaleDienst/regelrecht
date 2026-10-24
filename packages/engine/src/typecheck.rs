//! Static type check of a law: nullability (RFC-036) and general typing
//! (RFC-037).
//!
//! A law declares the type of every parameter, input and output, and since
//! schema v0.5.8 also whether the field may be absent (`nullable`). The
//! declarations are claims the law makes about the world it reads and the
//! outcome it produces, and most of them can be held against the law's own
//! actions before it ever runs: a field that is never absent cannot usefully
//! be tested for absence, a value that may be absent cannot be added up
//! without first asking whether it is there, and an `IF` that says nothing
//! for the unmatched case cannot feed an output that promised an answer.
//! Finding these in the editor and in `just validate` is cheaper than finding
//! them as an `AbsentOperand` on one citizen's case.
//!
//! The checker only reports what it can know from the declarations. Objects
//! are untyped, so a property path `$x.y` has unknown type; a `FOREACH`
//! binding has unknown type and unknown nullability; a `MIN`/`MAX` over a
//! collection may be empty and so may be absent, which the checker cannot
//! see. "Unknown" means: make no claim. A finding on a correct law is a bug in
//! the rule.
//!
//! The rules, each pinned by a test:
//!
//! - **N1** `EQUALS`/`NOT_EQUALS`/`IS_NULL`/`NOT_NULL` against a literal
//!   `null`: the other side must be a field that may be absent.
//! - **N2** A literal `null` is written only as the value of a nullable
//!   output (directly, or through the branches of an `IF`), or as an element
//!   of a container literal (an item of `LIST`, the body of a `FOREACH`
//!   without `combine`): an array holding an absence is a value, and its
//!   elements are untyped anyway.
//! - **N3** An `IF` without `default` yields `null` when no case matches, so
//!   it is allowed only as the value of a nullable output.
//! - **N4** A value that may be absent enters arithmetic, ordering, logic, an
//!   `IF` condition or a date operation, or is assigned to an output that is
//!   never absent, only on a path where the law has established it is
//!   present. `EQUALS` and `IN` are not in that list: they are structural on
//!   an absence (RFC-036), so an absent subject is a defined `false` there,
//!   not an error. Nor is a `FOREACH` collection: a `null` collection
//!   iterates nothing (RFC-036). A path establishes presence through an
//!   absence test in a condition, through a boolean output that is such a
//!   test (`heeft_partner = NOT(EQUALS $partner null)`, referenced as
//!   `$heeft_partner`), and through a `FOREACH` filter for its body; a present
//!   field implies a present record.
//! - **N5** An input that takes another output must be nullable when that
//!   output is, and when the call that fetches it is skipped for a required
//!   parameter that may be null and that the target does not declare
//!   nullable. For a call to an article (RFC-050), whose required
//!   parameters are known only at run time, any reference under
//!   `parameters` that may be null, and any `parameters_from`, counts so.
//!   An article that implements a required open term without a
//!   default may not declare its output for the term nullable, unless the
//!   law leaves the term to an authority per case (`decided_per_case_by`).
//! - **T1** Arithmetic and ordering operands are numbers (or amounts); `ADD`
//!   also concatenates strings and arrays, ordering also compares dates.
//! - **T2** Logical operands and `IF` conditions are booleans.
//! - **T3** `EQUALS` compares values of one type; `number` and `amount` are
//!   one type here, and so are `date` and `string`, since dates arrive as ISO
//!   strings.
//! - **T4** A literal assigned to an output has the output's type.
//! - **T5** A call to an article (`source.article`, RFC-050) without
//!   `output` yields a decision value, so the input is of type `decision`.
//!   What it merges (`parameters_from`) is an object, and a `$`-reference
//!   that names the article is a string, or a decision value that is
//!   called anew (RFC-050). A fixed target the checker can
//!   consult takes a decision, and an `output` is one of that decision; a
//!   target of this law also exists. A target taken from a value is known
//!   only at run time: the checker then knows the input is a decision value
//!   and claims nothing about its fields (`$x.outputs.y`), as for rows.

use crate::article::{
    foreach_binding, Action, ActionOperation, ActionValue, Article, ArticleBasedLaw, CombineOp,
    Source,
};
use crate::types::{ParameterType, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The rule a finding violates. The N rules are RFC-036's nullability rules,
/// the T rules RFC-037's general typing rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    N1,
    N2,
    N3,
    N4,
    N5,
    T1,
    T2,
    T3,
    T4,
    T5,
}

impl Rule {
    /// The rule's short code, as written in the RFCs.
    pub fn code(self) -> &'static str {
        match self {
            Rule::N1 => "N1",
            Rule::N2 => "N2",
            Rule::N3 => "N3",
            Rule::N4 => "N4",
            Rule::N5 => "N5",
            Rule::T1 => "T1",
            Rule::T2 => "T2",
            Rule::T3 => "T3",
            Rule::T4 => "T4",
            Rule::T5 => "T5",
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// One violation, naming the law, the article, the place in the article and
/// the rule, with a message that says what to change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub law_id: String,
    pub article: String,
    /// `output 'x'`, `action 3`, `input 'x'` or `definition 'X'`.
    pub location: String,
    pub rule: Rule,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} article {} {}: [{}] {}",
            self.law_id, self.article, self.location, self.rule, self.message
        )
    }
}

/// Check one law. `lookup` finds the laws the checker may consult for N5 (the
/// other laws of a directory being validated, or the laws already loaded); a
/// referenced law it cannot find is skipped silently.
pub fn check_law<'l>(
    law: &'l ArticleBasedLaw,
    lookup: &dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for article in &law.articles {
        let mut checker = ArticleChecker::new(law, article, lookup, &mut findings);
        checker.check_article();
    }
    findings
}

/// Check one law without any other law available: N5 is then limited to the
/// law's own articles.
pub fn check_law_alone(law: &ArticleBasedLaw) -> Vec<Finding> {
    check_law(law, &|_| None)
}

/// The types a field can declare, with `number` and `amount` kept apart so a
/// message can name what the law wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    String,
    Number,
    Amount,
    Boolean,
    Date,
    Array,
    Object,
    /// A decision value `{basis, fields, findings, outputs}` (RFC-050): an object
    /// whose fields the checker does not type, like any object.
    Decision,
}

impl Ty {
    fn of(declared: ParameterType) -> Ty {
        match declared {
            ParameterType::String => Ty::String,
            ParameterType::Number => Ty::Number,
            ParameterType::Boolean => Ty::Boolean,
            ParameterType::Amount => Ty::Amount,
            ParameterType::Date => Ty::Date,
            ParameterType::Array => Ty::Array,
            ParameterType::Object => Ty::Object,
            ParameterType::Decision => Ty::Decision,
        }
    }

    /// The type of a literal, or `None` for `null` and for a variable
    /// reference (a string starting with `$`).
    fn of_literal(value: &Value) -> Option<Ty> {
        match value {
            Value::Null | Value::Unknown(_) | Value::Untranslatable { .. } => None,
            Value::Bool(_) => Some(Ty::Boolean),
            Value::Int(_) | Value::Decimal(_) => Some(Ty::Number),
            Value::String(s) if is_reference(s) => None,
            Value::String(_) => Some(Ty::String),
            Value::Array(_) => Some(Ty::Array),
            Value::Object(_) => Some(Ty::Object),
        }
    }

    fn is_numeric(self) -> bool {
        matches!(self, Ty::Number | Ty::Amount)
    }

    /// Whether two known types may meet in a comparison or an assignment:
    /// `number` and `amount` are both numbers, a `date` arrives as an ISO
    /// string from data and from `$referencedate.iso`, and a `decision` is an
    /// object at run time.
    fn compatible(self, other: Ty) -> bool {
        self == other
            || (self.is_numeric() && other.is_numeric())
            || matches!(
                (self, other),
                (Ty::Date, Ty::String)
                    | (Ty::String, Ty::Date)
                    | (Ty::Decision, Ty::Object)
                    | (Ty::Object, Ty::Decision)
            )
    }

    fn name(self) -> &'static str {
        match self {
            Ty::String => "string",
            Ty::Number => "number",
            Ty::Amount => "amount",
            Ty::Boolean => "boolean",
            Ty::Date => "date",
            Ty::Array => "array",
            Ty::Object => "object",
            Ty::Decision => "decision",
        }
    }
}

/// What the declarations say about one name in the article.
#[derive(Debug, Clone, Copy)]
struct Symbol {
    ty: Option<Ty>,
    nullable: bool,
}

/// Whether an expression can be `null` on the current path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nullness {
    /// Established present: a non-nullable field, a literal, an operation
    /// result that is never absent, or a nullable field inside its guard.
    Present,
    /// Established absent: inside the `then` of its own `EQUALS … null`.
    Absent,
    /// Declared nullable and not yet tested on this path.
    MayBeAbsent,
    /// The checker cannot know (a `FOREACH` binding, a property of a present
    /// record, a `MIN`/`MAX` that may run over nothing).
    Unknown,
}

/// What one `FOREACH` binds in its `filter` and `body`.
struct Binding {
    /// The element: its `as`, or `item` when it has none.
    name: String,
    /// Whether an object element's fields are also bound under their own
    /// names, which is the case only without `as` (RFC-049). Any bare name
    /// may then be a field of the data, so the checker claims nothing.
    fields: bool,
}

impl ArticleChecker<'_, '_> {
    /// Whether `name` is, or may be, bound by the innermost enclosing
    /// `FOREACH` or named by an outer one.
    fn is_bound(&self, name: &str) -> bool {
        self.bound.last().is_some_and(|b| b.fields) || self.bound.iter().any(|b| b.name == name)
    }
}

/// What the checker knows about one expression.
#[derive(Debug, Clone)]
struct Info {
    ty: Option<Ty>,
    nullness: Nullness,
    /// How to name the expression in a message: `'huur'` for a variable,
    /// `the IF` for an operation, `the literal` for a literal.
    describe: String,
}

impl Info {
    fn present(ty: Option<Ty>, describe: &str) -> Info {
        Info {
            ty,
            nullness: Nullness::Present,
            describe: describe.to_string(),
        }
    }
}

/// The facts a path has established about variables, keyed by the reference
/// without its `$` (`huur`, `partner.geboortedatum`).
#[derive(Debug, Clone, Default)]
struct Facts {
    present: BTreeSet<String>,
    absent: BTreeSet<String>,
}

impl Facts {
    fn with(&self, other: &Facts) -> Facts {
        let mut merged = self.clone();
        merged.extend(other);
        merged
    }

    fn extend(&mut self, other: &Facts) {
        self.present.extend(other.present.iter().cloned());
        self.absent.extend(other.absent.iter().cloned());
    }

    fn is_empty(&self) -> bool {
        self.present.is_empty() && self.absent.is_empty()
    }

    /// A field that is present sits on a record that is present: `x.y`
    /// present marks `x` present too (RFC-036: a field of no record is no
    /// record). The converse does not hold, so `mark_absent` marks only the
    /// path itself.
    fn mark_present(&mut self, path: &str) {
        for (index, character) in path.char_indices() {
            if character == '.' {
                self.present.insert(path[..index].to_string());
            }
        }
        self.present.insert(path.to_string());
    }

    fn mark_absent(&mut self, path: &str) {
        self.absent.insert(path.to_string());
    }
}

/// What a boolean output tells about other variables, learned from the
/// action that computes it: `heeft_x = NOT(EQUALS $x null)` establishes `x`
/// present where `$heeft_x` holds and absent where it fails. This is the
/// corpus idiom for an absence test (`heeft_partner`), and a condition that
/// references such an output inherits its facts.
#[derive(Debug, Clone, Default)]
struct Derived {
    when_true: Facts,
    when_false: Facts,
}

/// Where an expression sits, for the rules that depend on it.
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// The expression is (a branch of) the value of a nullable output, so a
    /// literal `null` and an `IF` without `default` are allowed (N2, N3).
    absence_allowed: bool,
    /// The expression is (a branch of) the value of an output that is never
    /// absent: a reference that may be absent on this path is an N4 there,
    /// since the assignment would break the output's promise at run time.
    assigned: bool,
}

const ABSENCE_ALLOWED: Slot = Slot {
    absence_allowed: true,
    assigned: false,
};
const OPERAND: Slot = Slot {
    absence_allowed: false,
    assigned: false,
};
const STRICT_OUTPUT: Slot = Slot {
    absence_allowed: false,
    assigned: true,
};

fn is_reference(s: &str) -> bool {
    s.starts_with('$')
}

/// The path of a reference without its `$`, and its root name.
fn reference_path(s: &str) -> (&str, &str) {
    let path = &s[1..];
    let root = path.split('.').next().unwrap_or(path);
    (path, root)
}

/// The literal `null`, as an operand.
fn is_null_literal(value: &ActionValue) -> bool {
    matches!(value, ActionValue::Literal(Value::Null))
}

/// The literal `true`, as a `when:`.
fn is_true_literal(value: &ActionValue) -> bool {
    matches!(value, ActionValue::Literal(Value::Bool(true)))
}

/// The reference path an operand names, if it is a variable reference.
fn reference_of(value: &ActionValue) -> Option<&str> {
    match value {
        ActionValue::Literal(Value::String(s)) if is_reference(s) => Some(&s[1..]),
        _ => None,
    }
}

/// The checker for one article. `'l` is the lifetime of the laws (the one
/// being checked and the ones `lookup` may return), `'f` that of the call.
struct ArticleChecker<'l, 'f> {
    law: &'l ArticleBasedLaw,
    article: &'l Article,
    lookup: &'f dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
    symbols: BTreeMap<String, Symbol>,
    /// What each enclosing `FOREACH` binds; those names shadow the
    /// declarations and the checker claims nothing about them.
    bound: Vec<Binding>,
    /// Outputs computed by an earlier action of this article that are an
    /// absence test in disguise, with what they establish.
    derived: BTreeMap<String, Derived>,
    /// Outputs an earlier action of this article assigned; a second
    /// assignment makes the derived facts ambiguous and drops them.
    assigned: BTreeSet<String>,
    /// The output the current action assigns, if it declares one.
    output: Option<String>,
    location: String,
    findings: &'f mut Vec<Finding>,
}

impl<'l, 'f> ArticleChecker<'l, 'f> {
    fn new(
        law: &'l ArticleBasedLaw,
        article: &'l Article,
        lookup: &'f dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
        findings: &'f mut Vec<Finding>,
    ) -> Self {
        Self {
            law,
            article,
            lookup,
            symbols: environment(article),
            bound: Vec::new(),
            derived: BTreeMap::new(),
            assigned: BTreeSet::new(),
            output: None,
            location: String::new(),
            findings,
        }
    }

    /// Record a finding, unless the same finding is already there: one
    /// operand used twice in one operation is one mistake.
    fn report(&mut self, rule: Rule, message: String) {
        let finding = Finding {
            law_id: self.law.id.clone(),
            article: self.article.number.clone(),
            location: self.location.clone(),
            rule,
            message,
        };
        if !self.findings.contains(&finding) {
            self.findings.push(finding);
        }
    }

    /// The law a reference to `law_id` means: this law for its own id, else
    /// what `lookup` knows.
    fn law_named(&self, law_id: &str) -> Option<&'l ArticleBasedLaw> {
        if law_id == self.law.id {
            Some(self.law)
        } else {
            (self.lookup)(law_id)
        }
    }

    fn check_article(&mut self) {
        self.check_definitions();
        self.check_input_sources();
        self.check_implements();
        let Some(actions) = self
            .article
            .get_execution_spec()
            .and_then(|e| e.actions.as_ref())
        else {
            return;
        };
        // The order the engine runs the actions in, so a fact an output
        // establishes (an absence test) reaches every action that reads it,
        // wherever the file declares it. Outputs that read each other are a
        // cycle the engine reports when it runs; here they keep file order.
        let order = crate::demand::execution_order(actions, None)
            .unwrap_or_else(|_| (0..actions.len()).collect());
        for index in order {
            let action = &actions[index];
            self.location = match &action.output {
                Some(output) => format!("output '{output}'"),
                None => format!("action {}", index + 1),
            };
            self.output = action.output.clone();
            self.check_action(action);
        }
    }

    /// N5 for delegation (RFC-036): an article that implements a required
    /// open term without a default promises the delegating law a value. The
    /// engine takes the implementation's output as the term's value, and the
    /// delegating law reads the term as never absent, so the output may not
    /// be declared nullable. A term with a default is filled by the default
    /// when the implementation is silent, a term the law leaves to an
    /// authority per case (`decided_per_case_by`) by that authority's
    /// judgement (RFC-047), and an optional term without one is nullable
    /// itself; none of them needs this.
    fn check_implements(&mut self) {
        let Some(implements) = self.article.get_implements() else {
            return;
        };
        for declaration in implements {
            let Some(output) = self.article.find_output(&declaration.open_term) else {
                continue;
            };
            if !output.is_nullable() {
                continue;
            }
            let Some(term) = self
                .law_named(&declaration.law)
                .and_then(|law| law.find_article_by_number(&declaration.article))
                .and_then(|article| article.get_open_terms())
                .and_then(|terms| terms.iter().find(|t| t.id == declaration.open_term))
            else {
                continue;
            };
            if term.required && !term_has_default(term) && term.decided_per_case_by.is_none() {
                self.location = format!("output '{}'", output.name);
                self.report(
                    Rule::N5,
                    format!(
                        "'{}' implements the required term '{}' of {} article {}, which has no \
                         default, and may be null; produce a value for every case or give the \
                         term a default",
                        output.name, term.id, declaration.law, declaration.article
                    ),
                );
            }
        }
    }

    /// A definition is a constant the law writes, and a constant is never
    /// absent (N2).
    fn check_definitions(&mut self) {
        let Some(definitions) = self.article.get_definitions() else {
            return;
        };
        let mut names: Vec<&String> = definitions.keys().collect();
        names.sort();
        for name in names {
            if matches!(definitions[name].value(), Value::Null) {
                self.location = format!("definition '{name}'");
                self.report(
                    Rule::N2,
                    format!(
                        "definition '{name}' is null; a definition is a constant the law \
                         writes, and a constant is never absent"
                    ),
                );
            }
        }
    }

    /// N5: an input that takes another article's output. T5 for an input
    /// that calls an article.
    fn check_input_sources(&mut self) {
        for input in self.article.get_inputs() {
            let Some(source) = &input.source else {
                continue;
            };
            if let Some(target) = &source.article {
                self.location = format!("input '{}'", input.name);
                self.check_article_call(input, source, target);
                continue;
            }
            if input.is_nullable() {
                continue;
            }
            let (target_law, output_name) = match (&source.regulation, &source.output) {
                (Some(regulation), output) => {
                    let Some(law) = self.law_named(regulation) else {
                        continue;
                    };
                    (law, output.as_deref().unwrap_or(&input.name))
                }
                (None, Some(output)) => (self.law, output.as_str()),
                (None, None) => continue,
            };
            self.location = format!("input '{}'", input.name);
            let Some(target_article) = target_law.find_article_by_output(output_name) else {
                continue;
            };
            if target_article
                .find_output(output_name)
                .is_some_and(|output| output.is_nullable())
            {
                self.report(
                    Rule::N5,
                    format!(
                        "'{}' takes '{output_name}' from {}, which may be null; declare it \
                         nullable or guard in the source",
                        input.name, target_law.id
                    ),
                );
            }
            self.check_skip_on_null_parameter(source, target_article, &input.name, &target_law.id);
        }
    }

    /// T5 and N5: an input that calls the decision of an
    /// article (RFC-050).
    fn check_article_call(&mut self, input: &crate::article::Input, source: &Source, target: &str) {
        let name = &input.name;
        if source.output.is_none() && input.input_type != ParameterType::Decision {
            self.report(
                Rule::T5,
                format!(
                    "'{name}' takes the decision of {target}, a decision value; declare it type \
                     decision, or name one output of the decision with `output`"
                ),
            );
        }
        let declared_root = |checker: &Self, reference: &str| -> Option<Ty> {
            let (path, root) = reference_path(reference);
            (path == root)
                .then(|| checker.symbols.get(root).and_then(|s| s.ty))
                .flatten()
        };
        for reference in source.parameters_from.iter().flatten() {
            if let Some(ty) = declared_root(self, reference) {
                if ty != Ty::Object {
                    let hint = if ty == Ty::Decision {
                        format!(" (the parameters of a decision are {reference}.fields)")
                    } else {
                        String::new()
                    };
                    self.report(
                        Rule::T5,
                        format!(
                            "'{name}' merges {reference} into the parameters of {target}, but \
                             it is declared {}; parameters_from takes objects{hint}",
                            ty.name()
                        ),
                    );
                }
            }
        }
        if !input.is_nullable() {
            self.check_skip_on_null_article_parameters(source, name, target);
        }
        if is_reference(target) {
            // A null target is no call, and the input is null (RFC-036); a
            // path is null when its record is.
            let (_, root) = reference_path(target);
            if !input.is_nullable() && self.symbols.get(root).is_some_and(|s| s.nullable) {
                self.report(
                    Rule::N5,
                    format!(
                        "'{name}' is null when '{root}' is null, because no article is then \
                         called; declare '{name}' nullable or make '{root}' non-nullable"
                    ),
                );
            }
            match declared_root(self, target) {
                // A decision value is called anew with its own fields and
                // findings (RFC-050); a required field it holds as null
                // skips the call, as `parameters_from` may.
                Some(Ty::Decision) if !input.is_nullable() => self.report(
                    Rule::N5,
                    format!(
                        "'{name}' calls the decision {target} anew, whose fields may leave a \
                         required parameter null, and the article is then not called; declare \
                         '{name}' nullable"
                    ),
                ),
                Some(Ty::String | Ty::Decision) | None => {}
                Some(ty) => self.report(
                    Rule::T5,
                    format!(
                        "'{name}' calls the article named by {target}, which is declared {}; \
                         an article is named by a string `<regulation>#<article>`, or called \
                         anew from a decision value",
                        ty.name()
                    ),
                ),
            }
            return;
        }
        let Some((regulation, number)) = target.split_once('#') else {
            return;
        };
        let Some(target_law) = self.law_named(regulation) else {
            return;
        };
        let Some(target_article) = target_law.find_article_by_number(number) else {
            // Another law may number this article in another version than
            // the one at hand; only this law's own numbering is certain.
            if regulation == self.law.id {
                self.report(
                    Rule::T5,
                    format!("'{name}' calls {target}, and this law has no article {number}"),
                );
            }
            return;
        };
        // The decision is checked in the version at hand, as N5 checks the
        // output of a `regulation` reference.
        let Some(decision) = target_article.get_decision() else {
            self.report(
                Rule::T5,
                format!("'{name}' calls the decision of {target}, which takes no decision"),
            );
            return;
        };
        if let Some(output) = &source.output {
            if !target_article
                .outputs_of(decision)
                .contains(&output.as_str())
            {
                self.report(
                    Rule::T5,
                    format!("'{name}' takes '{output}' from {target}, which is no output of its decision"),
                );
                return;
            }
            if !input.is_nullable()
                && target_article
                    .find_output(output)
                    .is_some_and(|o| o.is_nullable())
            {
                self.report(
                    Rule::N5,
                    format!(
                        "'{name}' takes '{output}' from {target}, which may be null; declare it \
                         nullable or guard in the source"
                    ),
                );
            }
        }
    }

    /// N5 for the skip rule (RFC-036) on a call to an article: a required
    /// parameter that is null skips the call, and the input is null. Which
    /// parameters the article requires is known only at run time (the target
    /// may be named by a value, and is the version in force on the date), so
    /// any nullable reference under `parameters` may skip it, and so may
    /// `parameters_from`, whose objects may lack or null any field.
    fn check_skip_on_null_article_parameters(&mut self, source: &Source, name: &str, target: &str) {
        for (parameter, argument) in source.parameters.iter().flatten() {
            if !is_reference(argument) {
                continue;
            }
            let (path, root) = reference_path(argument);
            if !self.symbols.get(root).is_some_and(|s| s.nullable) {
                continue;
            }
            let argument = if path == root {
                String::new()
            } else {
                format!(" (and so is '{path}')")
            };
            self.report(
                Rule::N5,
                format!(
                    "'{name}' is null when '{root}' is null{argument}, because {target} is then \
                     not called if its parameter '{parameter}' is required; declare '{name}' \
                     nullable or make '{root}' non-nullable"
                ),
            );
        }
        if source
            .parameters_from
            .as_ref()
            .is_some_and(|f| !f.is_empty())
        {
            self.report(
                Rule::N5,
                format!(
                    "'{name}' takes its parameters for {target} from parameters_from, which may \
                     leave a required parameter null, and {target} is then not called; declare \
                     '{name}' nullable"
                ),
            );
        }
    }

    /// The skip rule (RFC-036): a cross-law call whose required parameter is
    /// `null` is not made, and the input is `null`, unless the target
    /// declares that parameter nullable (it then said it can decide on
    /// nobody, and it is run). So a nullable variable passed for a required,
    /// non-nullable parameter of the target makes the input nullable,
    /// whatever the target's output says. A property path `$rec.bsn` is null
    /// when its record is (RFC-036), so the record's nullability counts for
    /// it; whether the field itself may be null on a present record is not
    /// known here.
    fn check_skip_on_null_parameter(
        &mut self,
        source: &Source,
        target_article: &Article,
        input_name: &str,
        target_id: &str,
    ) {
        let Some(parameters) = &source.parameters else {
            return;
        };
        if source.regulation.is_none() {
            return;
        }
        let mut nullable_keys: Vec<(&String, &str, &str)> = Vec::new();
        for (name, argument) in parameters {
            if !is_reference(argument) {
                continue;
            }
            let (path, root) = reference_path(argument);
            let Some(symbol) = self.symbols.get(root) else {
                continue;
            };
            let parameter = target_article.find_parameter(name);
            let required = parameter.is_none_or(|p| p.required != Some(false));
            let nullable = parameter.is_some_and(|p| p.is_nullable());
            if symbol.nullable && required && !nullable {
                nullable_keys.push((name, path, root));
            }
        }
        for (name, path, root) in nullable_keys {
            let argument = if path == root {
                String::new()
            } else {
                format!(" (and so is '{path}')")
            };
            self.report(
                Rule::N5,
                format!(
                    "'{input_name}' is null when '{root}' is null{argument}, because {target_id} \
                     is then not called (its parameter '{name}' is required and not nullable); \
                     declare '{input_name}' nullable or make '{root}' non-nullable"
                ),
            );
        }
    }

    fn check_action(&mut self, action: &Action) {
        let output = action
            .output
            .as_deref()
            .and_then(|name| self.article.find_output(name));
        let declared_ty = output.map(|o| Ty::of(o.output_type));
        // An undeclared output makes no promise about absence, so it is
        // checked as an operand: no literal null, no IF without default, but
        // no assignment rule either.
        let slot = match output {
            Some(o) if o.is_nullable() => ABSENCE_ALLOWED,
            Some(_) => STRICT_OUTPUT,
            None => OPERAND,
        };
        let facts = Facts::default();

        let expression = match &action.operation {
            Some(operation) => match crate::engine::action_to_operation(action, operation) {
                Ok(op) => ActionValue::Operation(Box::new(op)),
                // A malformed inline action is the evaluator's error to
                // report; the checker claims nothing about it.
                Err(_) => return,
            },
            // `value: null` at the action level deserializes to no value at
            // all, and the evaluator then yields `null`: it is the literal null
            // written in the only way YAML allows here.
            None => action
                .value
                .clone()
                .unwrap_or(ActionValue::Literal(Value::Null)),
        };

        let info = self.check_expr(&expression, &facts, slot);
        if let (Some(declared), Some(actual)) = (declared_ty, info.ty) {
            if !declared.compatible(actual) && is_literal_shaped(&expression) {
                self.report(
                    Rule::T4,
                    format!(
                        "output is declared {} but is assigned a {} literal",
                        declared.name(),
                        actual.name()
                    ),
                );
            }
        }
        // A literal in a branch of an IF that is the output's value is an
        // assignment to the output as well (T4).
        if let (Some(declared), ActionValue::Operation(op)) = (declared_ty, &expression) {
            self.check_branch_literals(op, declared);
        }
        // What this output tells a later action that decides on it.
        if let Some(name) = &action.output {
            let mut derived = Derived::default();
            self.facts_when_true(&expression, &mut derived.when_true);
            self.facts_when_false(&expression, &mut derived.when_false);
            let first_assignment = self.assigned.insert(name.clone());
            if first_assignment && !(derived.when_true.is_empty() && derived.when_false.is_empty())
            {
                self.derived.insert(name.clone(), derived);
            } else {
                self.derived.remove(name);
            }
        }
    }

    /// T4 through the branches of an `IF` that is the output's value.
    fn check_branch_literals(&mut self, op: &ActionOperation, declared: Ty) {
        let ActionOperation::If { cases, default } = op else {
            return;
        };
        let branches = cases.iter().map(|case| &case.then).chain(default.iter());
        for branch in branches {
            match branch {
                ActionValue::Literal(value) => {
                    if let Some(actual) = Ty::of_literal(value) {
                        if !declared.compatible(actual) {
                            self.report(
                                Rule::T4,
                                format!(
                                    "output is declared {} but a branch assigns a {} literal",
                                    declared.name(),
                                    actual.name()
                                ),
                            );
                        }
                    }
                }
                ActionValue::Operation(nested) => self.check_branch_literals(nested, declared),
            }
        }
    }

    /// Check an expression on a path with `facts`, in `slot`, and say what is
    /// known about its result.
    fn check_expr(&mut self, expr: &ActionValue, facts: &Facts, slot: Slot) -> Info {
        match expr {
            ActionValue::Literal(Value::Null) => {
                if !slot.absence_allowed {
                    self.report(
                        Rule::N2,
                        "a literal null may only be the value of a nullable output (directly \
                         or as a branch of its IF); here it would be calculated with, decided \
                         on or assigned to an output that is never absent"
                            .to_string(),
                    );
                }
                Info {
                    ty: None,
                    nullness: Nullness::Absent,
                    describe: "the literal null".to_string(),
                }
            }
            ActionValue::Literal(Value::String(s)) if is_reference(s) => {
                let info = self.reference(s, facts);
                if slot.assigned {
                    self.check_assignment(&info);
                }
                info
            }
            ActionValue::Literal(value) => Info::present(Ty::of_literal(value), "the literal"),
            ActionValue::Operation(op) => self.check_operation(op, facts, slot),
        }
    }

    /// N4 on assignment: a reference that may be absent on this path, written
    /// as (a branch of) the value of an output that is never absent, would
    /// make that output `null` at run time (`NullOutput`, RFC-036). The
    /// checker refuses it where it is written.
    fn check_assignment(&mut self, info: &Info) {
        let output = self.output.clone().unwrap_or_default();
        match info.nullness {
            Nullness::MayBeAbsent => self.report(
                Rule::N4,
                format!(
                    "{} may be absent and is assigned to '{output}', which is never absent; \
                     test for absence first (EQUALS … null) or declare '{output}' nullable",
                    info.describe
                ),
            ),
            Nullness::Absent => self.report(
                Rule::N4,
                format!(
                    "{} is absent on this path (the enclosing condition tested it with \
                     EQUALS … null) and is assigned to '{output}', which is never absent",
                    info.describe
                ),
            ),
            Nullness::Present | Nullness::Unknown => {}
        }
    }

    /// What the declarations and the path facts say about a reference.
    fn reference(&self, reference: &str, facts: &Facts) -> Info {
        let (path, root) = reference_path(reference);
        let describe = format!("'{path}'");
        if root == "referencedate" {
            let ty = if path == root {
                Some(Ty::Date)
            } else {
                match &path[root.len() + 1..] {
                    "year" | "month" | "day" => Some(Ty::Number),
                    "iso" => Some(Ty::String),
                    _ => None,
                }
            };
            return Info::present(ty, &describe);
        }
        if self.is_bound(root) {
            return Info {
                ty: None,
                nullness: Nullness::Unknown,
                describe,
            };
        }
        let Some(symbol) = self.symbols.get(root).copied() else {
            return Info {
                ty: None,
                nullness: Nullness::Unknown,
                describe,
            };
        };
        let nullness = if facts.absent.contains(path) {
            Nullness::Absent
        } else if facts.present.contains(path) {
            Nullness::Present
        } else if path != root {
            // A property of a record: objects are untyped, so nothing is
            // known about the field itself. What is known is the record: a
            // field of an absent record is absent (RFC-036).
            if facts.absent.contains(root) {
                Nullness::Absent
            } else if facts.present.contains(root) || !symbol.nullable {
                Nullness::Unknown
            } else {
                Nullness::MayBeAbsent
            }
        } else if symbol.nullable {
            Nullness::MayBeAbsent
        } else {
            Nullness::Present
        };
        let ty = if path == root { symbol.ty } else { None };
        Info {
            ty,
            nullness,
            describe,
        }
    }

    /// N4: an operand that must be present on this path.
    fn require_present(&mut self, info: &Info, operation: &str) {
        match info.nullness {
            Nullness::MayBeAbsent => self.report(
                Rule::N4,
                format!(
                    "{} may be absent and is used in {operation} without an absence test; \
                     test for absence first (EQUALS … null) or declare the field non-nullable",
                    info.describe
                ),
            ),
            Nullness::Absent => self.report(
                Rule::N4,
                format!(
                    "{} is absent on this path (the enclosing condition tested it with \
                     EQUALS … null) and is used in {operation}",
                    info.describe
                ),
            ),
            Nullness::Present | Nullness::Unknown => {}
        }
    }

    /// T1: an operand of arithmetic or ordering that must be a number (or one
    /// of the `also` types). Returns whether the operand passed, so a caller
    /// combining operands does not report the same operand twice.
    fn require_numeric(&mut self, info: &Info, operation: &str, also: &[Ty]) -> bool {
        match info.ty {
            Some(ty) if !ty.is_numeric() && !also.contains(&ty) => {
                self.report(
                    Rule::T1,
                    format!(
                        "{} is a {} and is used in {operation}, which needs a number",
                        info.describe,
                        ty.name()
                    ),
                );
                false
            }
            _ => true,
        }
    }

    /// T2: an operand of logic or an `IF` condition that must be a boolean.
    fn require_boolean(&mut self, info: &Info, operation: &str) {
        if let Some(ty) = info.ty {
            if ty != Ty::Boolean {
                self.report(
                    Rule::T2,
                    format!(
                        "{} is a {} and is used as a condition in {operation}, which needs a \
                         boolean",
                        info.describe,
                        ty.name()
                    ),
                );
            }
        }
    }

    /// An operand that is calculated with, ordered, decided on, or otherwise
    /// needed as a present value.
    fn operand(&mut self, expr: &ActionValue, facts: &Facts, operation: &str) -> Info {
        let info = self.check_expr(expr, facts, OPERAND);
        // A literal null here is already reported under N2.
        if !is_null_literal(expr) {
            self.require_present(&info, operation);
        }
        info
    }

    fn check_operation(&mut self, op: &ActionOperation, facts: &Facts, slot: Slot) -> Info {
        let name = op.operation_name();
        match op {
            ActionOperation::Add { values } => self.arithmetic(values, facts, name, true),
            ActionOperation::Subtract { values }
            | ActionOperation::Multiply { values }
            | ActionOperation::Divide { values }
            | ActionOperation::Max { values }
            | ActionOperation::Min { values } => self.arithmetic(values, facts, name, false),
            ActionOperation::Round { value, .. }
            | ActionOperation::Ceil { value, .. }
            | ActionOperation::Floor { value, .. } => {
                let info = self.operand(value, facts, name);
                self.require_numeric(&info, name, &[]);
                Info::present(info.ty.filter(|t| t.is_numeric()), &format!("the {name}"))
            }
            ActionOperation::GreaterThan { subject, value }
            | ActionOperation::LessThan { subject, value }
            | ActionOperation::GreaterThanOrEqual { subject, value }
            | ActionOperation::LessThanOrEqual { subject, value } => {
                let left = self.operand(subject, facts, name);
                let right = self.operand(value, facts, name);
                // Ordering compares numbers, or dates (which may arrive as
                // ISO strings, RFC-021).
                let both_ordered = self.require_numeric(&left, name, &[Ty::Date, Ty::String])
                    & self.require_numeric(&right, name, &[Ty::Date, Ty::String]);
                if let (true, Some(l), Some(r)) = (both_ordered, left.ty, right.ty) {
                    if !l.compatible(r) {
                        self.report(
                            Rule::T1,
                            format!(
                                "{name} orders {} ({}) against {} ({}), which are not the same \
                                 kind of value",
                                left.describe,
                                l.name(),
                                right.describe,
                                r.name()
                            ),
                        );
                    }
                }
                Info::present(Some(Ty::Boolean), &format!("the {name}"))
            }
            ActionOperation::Equals { subject, value }
            | ActionOperation::NotEquals { subject, value } => {
                self.equality(subject, value, facts, name);
                Info::present(Some(Ty::Boolean), &format!("the {name}"))
            }
            ActionOperation::IsNull { subject } | ActionOperation::NotNull { subject } => {
                let info = self.check_expr(subject, facts, OPERAND);
                self.absence_test(&info, subject, facts, name);
                Info::present(Some(Ty::Boolean), &format!("the {name}"))
            }
            ActionOperation::And { conditions } | ActionOperation::Or { conditions } => {
                let is_and = matches!(op, ActionOperation::And { .. });
                let mut path = facts.clone();
                for condition in conditions {
                    let info = self.operand(condition, &path, name);
                    self.require_boolean(&info, name);
                    // AND stops at the first false, so later conditions run
                    // only when the earlier ones held; OR stops at the first
                    // true, so later conditions run only when the earlier
                    // ones failed (RFC-036). That is what makes the guard
                    // idiom work, and the checker follows the same order.
                    let mut learned = Facts::default();
                    if is_and {
                        self.facts_when_true(condition, &mut learned);
                    } else {
                        self.facts_when_false(condition, &mut learned);
                    }
                    path = path.with(&learned);
                }
                Info::present(Some(Ty::Boolean), &format!("the {name}"))
            }
            ActionOperation::Not { value } => {
                let info = self.operand(value, facts, name);
                self.require_boolean(&info, name);
                Info::present(Some(Ty::Boolean), &format!("the {name}"))
            }
            ActionOperation::If { cases, default } => {
                self.conditional(cases, default.as_ref(), facts, slot)
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
            } => {
                // Membership is structural: `IN(null, [650])` is a definite
                // false (RFC-036), the same way `EQUALS` decides on an absence.
                // The subject therefore need not be established present, and
                // a `null` among the `values` is an element of a list, a
                // membership test for absence (N2).
                self.check_expr(subject, facts, OPERAND);
                if let Some(value) = value {
                    self.check_expr(value, facts, OPERAND);
                }
                for item in values.iter().flatten() {
                    self.check_expr(item, facts, ABSENCE_ALLOWED);
                }
                Info::present(Some(Ty::Boolean), &format!("the {name}"))
            }
            ActionOperation::List { items } => {
                // A container literal: an element may be an absence (RFC-036
                // compares containers element by element, and a null inside
                // one is a value), and elements are untyped.
                for item in items {
                    self.check_expr(item, facts, ABSENCE_ALLOWED);
                }
                Info::present(Some(Ty::Array), "the LIST")
            }
            ActionOperation::Foreach {
                collection,
                as_name,
                body,
                filter,
                combine,
            } => {
                // A null collection iterates nothing (RFC-036), so the
                // collection need not be established present.
                self.check_expr(collection, facts, OPERAND);
                self.bound.push(Binding {
                    name: foreach_binding(as_name.as_deref()).to_string(),
                    fields: as_name.is_none(),
                });
                // The body runs only for the elements the filter lets
                // through, so what the filter established holds in the body.
                let mut body_facts = facts.clone();
                if let Some(filter) = filter {
                    let info = self.operand(filter, facts, "FOREACH filter");
                    self.require_boolean(&info, "FOREACH filter");
                    self.facts_when_true(filter, &mut body_facts);
                }
                // Without `combine` the FOREACH builds an array, and its
                // elements may be absences like the items of a LIST; with a
                // combine the body is calculated with or decided on.
                let body_info = match combine {
                    None | Some(CombineOp::Only) => {
                        self.check_expr(body, &body_facts, ABSENCE_ALLOWED)
                    }
                    Some(_) => self.operand(body, &body_facts, "FOREACH body"),
                };
                self.bound.pop();
                match combine {
                    None => Info::present(Some(Ty::Array), "the FOREACH"),
                    Some(CombineOp::Add) => {
                        self.require_numeric(&body_info, "FOREACH combine ADD", &[]);
                        Info::present(body_info.ty.filter(|t| t.is_numeric()), "the FOREACH")
                    }
                    Some(CombineOp::And) | Some(CombineOp::Or) => {
                        self.require_boolean(&body_info, "FOREACH combine");
                        Info::present(Some(Ty::Boolean), "the FOREACH")
                    }
                    // The lowest or highest of nothing is absent, and whether
                    // the collection is empty is not something the checker
                    // can know.
                    Some(CombineOp::Min) | Some(CombineOp::Max) => {
                        self.require_numeric(&body_info, "FOREACH combine MIN/MAX", &[]);
                        Info {
                            ty: body_info.ty.filter(|t| t.is_numeric()),
                            nullness: Nullness::Unknown,
                            describe: "the FOREACH".to_string(),
                        }
                    }
                    // The one element the filter left, of the body's type;
                    // absent when it left none.
                    Some(CombineOp::Only) => Info {
                        ty: body_info.ty,
                        nullness: Nullness::Unknown,
                        describe: "the FOREACH".to_string(),
                    },
                }
            }
            ActionOperation::Age {
                date_of_birth,
                reference_date,
            } => {
                self.operand(date_of_birth, facts, name);
                self.operand(reference_date, facts, name);
                Info::present(Some(Ty::Number), "the AGE")
            }
            ActionOperation::DateAdd {
                date,
                years,
                months,
                weeks,
                days,
            } => {
                self.operand(date, facts, name);
                for part in [years, months, weeks, days].into_iter().flatten() {
                    let info = self.operand(part, facts, name);
                    self.require_numeric(&info, name, &[]);
                }
                Info::present(Some(Ty::Date), "the DATE_ADD")
            }
            ActionOperation::Date { year, month, day } => {
                for part in [year, month, day] {
                    let info = self.operand(part, facts, name);
                    self.require_numeric(&info, name, &[]);
                }
                Info::present(Some(Ty::Date), "the DATE")
            }
            ActionOperation::DayOfWeek { date } => {
                self.operand(date, facts, name);
                Info::present(Some(Ty::Number), "the DAY_OF_WEEK")
            }
            ActionOperation::DateDiff { from, to, unit } => {
                self.operand(from, facts, name);
                self.operand(to, facts, name);
                self.check_expr(unit, facts, OPERAND);
                Info::present(Some(Ty::Number), "the DATE_DIFF")
            }
            // RFC-032: DATE_PART reads one calendar component as a number,
            // START_OF truncates a date and stays a date.
            ActionOperation::DatePart { date, .. } => {
                self.operand(date, facts, name);
                Info::present(Some(Ty::Number), "the DATE_PART")
            }
            ActionOperation::StartOf { date, .. } => {
                self.operand(date, facts, name);
                Info::present(Some(Ty::Date), "the START_OF")
            }
        }
    }

    /// `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE`, `MIN`, `MAX` (T1, N4). `ADD`
    /// also concatenates strings and arrays (RFC-007).
    fn arithmetic(
        &mut self,
        values: &[ActionValue],
        facts: &Facts,
        name: &str,
        is_add: bool,
    ) -> Info {
        let also: &[Ty] = if is_add {
            &[Ty::String, Ty::Array]
        } else {
            &[]
        };
        let mut result: Option<Ty> = None;
        for value in values {
            let info = self.operand(value, facts, name);
            if !self.require_numeric(&info, name, also) {
                continue;
            }
            match (result, info.ty) {
                (None, Some(ty)) => result = Some(ty),
                // An amount stays an amount when a plain number joins it.
                (Some(Ty::Number), Some(Ty::Amount)) => result = Some(Ty::Amount),
                (Some(current), Some(ty)) if !current.compatible(ty) => {
                    self.report(
                        Rule::T1,
                        format!(
                            "{name} combines a {} with {} ({}), which are not the same kind of \
                             value",
                            current.name(),
                            info.describe,
                            ty.name()
                        ),
                    );
                }
                _ => {}
            }
        }
        Info::present(result, &format!("the {name}"))
    }

    /// `EQUALS` and `NOT_EQUALS` (N1, T3). Either side may be a literal
    /// `null`: that is the absence test, and the other side must then be a
    /// field that may be absent. Otherwise both sides are compared as values,
    /// and an absent value may take part: `EQUALS` is structural.
    fn equality(&mut self, subject: &ActionValue, value: &ActionValue, facts: &Facts, name: &str) {
        match (is_null_literal(subject), is_null_literal(value)) {
            (true, true) => {
                self.report(
                    Rule::N1,
                    format!("{name} compares null with null, which decides nothing"),
                );
            }
            (true, false) | (false, true) => {
                let other = if is_null_literal(subject) {
                    value
                } else {
                    subject
                };
                let info = self.check_expr(other, facts, OPERAND);
                self.absence_test(&info, other, facts, name);
            }
            (false, false) => {
                let left = self.check_expr(subject, facts, OPERAND);
                let right = self.check_expr(value, facts, OPERAND);
                if let (Some(l), Some(r)) = (left.ty, right.ty) {
                    if !l.compatible(r) {
                        self.report(
                            Rule::T3,
                            format!(
                                "{name} compares {} ({}) with {} ({}), which can never be equal",
                                left.describe,
                                l.name(),
                                right.describe,
                                r.name()
                            ),
                        );
                    }
                }
            }
        }
    }

    /// N1: the subject of an absence test must be something that can be
    /// absent. A field the law declares never absent, a literal, or an
    /// operation that always yields a value cannot be. A nullable field that
    /// an enclosing condition already established present cannot be either,
    /// but there the test is redundant rather than wrong, and the message
    /// says so.
    fn absence_test(&mut self, info: &Info, subject: &ActionValue, facts: &Facts, name: &str) {
        if info.nullness != Nullness::Present {
            return;
        }
        let guarded = reference_of(subject).is_some_and(|path| facts.present.contains(path));
        let message = if guarded {
            format!(
                "{} is already established present on this path (an enclosing condition tested \
                 it with EQUALS … null), so comparing it with null in {name} cannot be true; \
                 the test is redundant",
                info.describe
            )
        } else {
            format!(
                "{} is not nullable, so comparing it with null in {name} cannot be true; \
                 declare `nullable: true` or remove the test",
                info.describe
            )
        };
        self.report(Rule::N1, message);
    }

    /// `IF` (N3, N4 flow, T2). Each `when` is checked with what the earlier
    /// cases established by failing, each `then` with what its own `when`
    /// established by holding, the `default` with everything that failed.
    fn conditional(
        &mut self,
        cases: &[crate::article::Case],
        default: Option<&ActionValue>,
        facts: &Facts,
        slot: Slot,
    ) -> Info {
        let mut path = facts.clone();
        let mut branches: Vec<Info> = Vec::new();
        for case in cases {
            let info = self.operand(&case.when, &path, "IF condition");
            self.require_boolean(&info, "IF");
            let mut holds = Facts::default();
            self.facts_when_true(&case.when, &mut holds);
            branches.push(self.check_expr(&case.then, &path.with(&holds), slot));
            let mut fails = Facts::default();
            self.facts_when_false(&case.when, &mut fails);
            path = path.with(&fails);
        }
        let exhaustive = cases.last().is_some_and(|case| is_true_literal(&case.when));
        let mut nullness = Nullness::Present;
        match default {
            Some(default) => branches.push(self.check_expr(default, &path, slot)),
            None if exhaustive => {}
            None => {
                if !slot.absence_allowed {
                    self.report(
                        Rule::N3,
                        "IF without default (or with `default: null`) may yield null; add a \
                         default or declare the output nullable"
                            .to_string(),
                    );
                }
                nullness = Nullness::MayBeAbsent;
            }
        }
        let mut ty: Option<Ty> = None;
        let mut agree = true;
        let mut describe = "the IF".to_string();
        for branch in &branches {
            match (ty, branch.ty) {
                (None, Some(t)) => ty = Some(t),
                (Some(current), Some(t)) if current != t => agree = false,
                _ => {}
            }
            match branch.nullness {
                Nullness::Present => {}
                Nullness::Unknown => {
                    if nullness == Nullness::Present {
                        nullness = Nullness::Unknown;
                    }
                }
                Nullness::Absent | Nullness::MayBeAbsent => {
                    nullness = Nullness::MayBeAbsent;
                    describe = format!("the IF, whose branch {} may be absent,", branch.describe);
                }
            }
        }
        Info {
            ty: if agree { ty } else { None },
            nullness,
            describe,
        }
    }

    /// The facts a boolean output computed earlier in this article carries,
    /// when a condition references it as `$name`. A `FOREACH` binding of the
    /// same name shadows the output.
    fn derived_facts(&self, condition: &ActionValue) -> Option<&Derived> {
        let path = reference_of(condition)?;
        if path.contains('.') || self.is_bound(path) {
            return None;
        }
        self.derived.get(path)
    }

    /// The variables a condition establishes as present or absent when it
    /// holds.
    fn facts_when_true(&self, condition: &ActionValue, out: &mut Facts) {
        let ActionValue::Operation(op) = condition else {
            if let Some(derived) = self.derived_facts(condition) {
                out.extend(&derived.when_true);
            }
            return;
        };
        match op.as_ref() {
            ActionOperation::Equals { subject, value } => {
                if let Some(path) = null_tested(subject, value) {
                    out.mark_absent(path);
                }
            }
            ActionOperation::NotEquals { subject, value } => {
                if let Some(path) = null_tested(subject, value) {
                    out.mark_present(path);
                }
            }
            ActionOperation::IsNull { subject } => {
                if let Some(path) = reference_of(subject) {
                    out.mark_absent(path);
                }
            }
            ActionOperation::NotNull { subject } => {
                if let Some(path) = reference_of(subject) {
                    out.mark_present(path);
                }
            }
            ActionOperation::Not { value } => self.facts_when_false(value, out),
            ActionOperation::And { conditions } => {
                for condition in conditions {
                    self.facts_when_true(condition, out);
                }
            }
            _ => {}
        }
    }

    /// The variables a condition establishes as present or absent when it
    /// fails.
    fn facts_when_false(&self, condition: &ActionValue, out: &mut Facts) {
        let ActionValue::Operation(op) = condition else {
            if let Some(derived) = self.derived_facts(condition) {
                out.extend(&derived.when_false);
            }
            return;
        };
        match op.as_ref() {
            ActionOperation::Equals { subject, value } => {
                if let Some(path) = null_tested(subject, value) {
                    out.mark_present(path);
                }
            }
            ActionOperation::NotEquals { subject, value } => {
                if let Some(path) = null_tested(subject, value) {
                    out.mark_absent(path);
                }
            }
            ActionOperation::IsNull { subject } => {
                if let Some(path) = reference_of(subject) {
                    out.mark_present(path);
                }
            }
            ActionOperation::NotNull { subject } => {
                if let Some(path) = reference_of(subject) {
                    out.mark_absent(path);
                }
            }
            ActionOperation::Not { value } => self.facts_when_true(value, out),
            ActionOperation::Or { conditions } => {
                for condition in conditions {
                    self.facts_when_false(condition, out);
                }
            }
            _ => {}
        }
    }
}

/// Whether an expression is a literal in the sense of T4: a bare literal
/// (not a reference), assigned directly to an output.
fn is_literal_shaped(expr: &ActionValue) -> bool {
    matches!(expr, ActionValue::Literal(value) if Ty::of_literal(value).is_some())
}

/// Whether an open term carries a default the engine can run. `default: {}`
/// or `default: {actions: []}` resolves to `null` ("using empty default"),
/// and counts as no default here.
fn term_has_default(term: &crate::article::OpenTerm) -> bool {
    term.default
        .as_ref()
        .is_some_and(|d| d.actions.as_ref().is_some_and(|a| !a.is_empty()))
}

/// The reference an `EQUALS`/`NOT_EQUALS` tests against a literal `null`.
fn null_tested<'v>(subject: &'v ActionValue, value: &'v ActionValue) -> Option<&'v str> {
    if is_null_literal(value) {
        reference_of(subject)
    } else if is_null_literal(subject) {
        reference_of(value)
    } else {
        None
    }
}

/// The declarations of an article as a symbol table: parameters, inputs,
/// outputs, definitions (constants, never absent) and open terms. An optional
/// open term without a `default` resolves to `null` when nothing implements
/// it (RFC-003), so it is nullable; a term with a default is filled by that
/// default, also when an implementation is silent for the case, so it is not.
/// A name declared twice (an input that is also an output) is nullable when
/// either declaration says so.
fn environment(article: &Article) -> BTreeMap<String, Symbol> {
    let mut symbols: BTreeMap<String, Symbol> = BTreeMap::new();
    let mut declare = |name: &str, ty: Option<Ty>, nullable: bool| {
        symbols
            .entry(name.to_string())
            .and_modify(|existing| {
                existing.nullable |= nullable;
                if existing.ty.is_none() {
                    existing.ty = ty;
                }
            })
            .or_insert(Symbol { ty, nullable });
    };
    for parameter in article.get_parameters() {
        declare(
            &parameter.name,
            Some(Ty::of(parameter.param_type)),
            parameter.is_nullable(),
        );
    }
    for input in article.get_inputs() {
        declare(
            &input.name,
            Some(Ty::of(input.input_type)),
            input.is_nullable(),
        );
    }
    for output in article.get_outputs() {
        declare(
            &output.name,
            Some(Ty::of(output.output_type)),
            output.is_nullable(),
        );
    }
    if let Some(definitions) = article.get_definitions() {
        for (name, definition) in definitions {
            declare(name, Ty::of_literal(definition.value()), false);
        }
    }
    if let Some(terms) = article.get_open_terms() {
        for term in terms {
            declare(
                &term.id,
                Some(Ty::of(term.term_type)),
                !term.required && !term_has_default(term),
            );
        }
    }
    symbols
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::article::LawLoad;

    /// A one-article law around the given declarations and actions.
    fn law(id: &str, execution: &str) -> ArticleBasedLaw {
        let indented: String = execution
            .lines()
            .map(|line| format!("        {line}\n"))
            .collect();
        let yaml = format!(
            "$id: {id}\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: t\n    machine_readable:\n      execution:\n{indented}"
        );
        ArticleBasedLaw::from_yaml_str(&yaml).unwrap_or_else(|e| panic!("{e}\n{yaml}"))
    }

    fn rules(findings: &[Finding]) -> Vec<Rule> {
        findings.iter().map(|f| f.rule).collect()
    }

    fn check(execution: &str) -> Vec<Finding> {
        check_law_alone(&law("t", execution))
    }

    fn assert_clean(execution: &str) {
        let findings = check(execution);
        assert!(findings.is_empty(), "unexpected findings: {findings:#?}");
    }

    fn assert_only(execution: &str, rule: Rule, needle: &str) -> Finding {
        let findings = check(execution);
        assert_eq!(rules(&findings), vec![rule], "{findings:#?}");
        let finding = findings.into_iter().next().unwrap();
        assert!(
            finding.message.contains(needle),
            "message {:?} does not contain {needle:?}",
            finding.message
        );
        finding
    }

    // -- N1 -----------------------------------------------------------------

    const N1_FAIL: &str = r#"
input:
  - name: huur
    type: amount
    source: {}
output:
  - name: geen_huur
    type: boolean
actions:
  - output: geen_huur
    value:
      operation: EQUALS
      subject: $huur
      value: null
"#;

    #[test]
    fn n1_absence_test_on_a_non_nullable_field_is_an_error() {
        let finding = assert_only(N1_FAIL, Rule::N1, "'huur' is not nullable");
        assert_eq!(finding.location, "output 'geen_huur'");
        assert_eq!(finding.article, "1");
        assert_eq!(finding.law_id, "t");
        // The nested EQUALS behind NOT, and NOT_EQUALS, IS_NULL and NOT_NULL, are
        // the same test.
        for variant in [
            "operation: NOT_EQUALS\n      subject: $huur\n      value: null",
            "operation: IS_NULL\n      subject: $huur",
            "operation: NOT_NULL\n      subject: $huur",
            "operation: NOT\n      value:\n        operation: EQUALS\n        subject: $huur\n        value: null",
        ] {
            let yaml = N1_FAIL.replace(
                "operation: EQUALS\n      subject: $huur\n      value: null",
                variant,
            );
            assert_eq!(rules(&check(&yaml)), vec![Rule::N1], "{variant}");
        }
    }

    #[test]
    fn n1_absence_test_on_a_nullable_field_or_a_property_path_is_fine() {
        assert_clean(&N1_FAIL.replace("type: amount\n", "type: amount\n    nullable: true\n"));
        // A property of a record: objects are untyped, no claim.
        assert_clean(&N1_FAIL.replace("subject: $huur", "subject: $huur.bedrag"));
        // The literal null may sit on the subject side as well.
        assert_clean(
            &N1_FAIL
                .replace(
                    "subject: $huur\n      value: null",
                    "subject: null\n      value: $huur",
                )
                .replace("type: amount\n", "type: amount\n    nullable: true\n"),
        );
    }

    #[test]
    fn n1_a_literal_or_a_definition_is_never_absent() {
        assert_only(
            &N1_FAIL.replace("subject: $huur", "subject: 650"),
            Rule::N1,
            "the literal is not nullable",
        );
        let with_definition = r#"
$id: t
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      definitions:
        GRENS: 650
      execution:
        output:
          - name: geen_grens
            type: boolean
        actions:
          - output: geen_grens
            value:
              operation: EQUALS
              subject: $GRENS
              value: null
"#;
        // The environment entry for the definition is what makes this a claim.
        let findings = check_law_alone(&ArticleBasedLaw::from_yaml_str(with_definition).unwrap());
        assert_eq!(rules(&findings), vec![Rule::N1], "{findings:#?}");
        assert!(findings[0].message.contains("'GRENS' is not nullable"));
        assert_only(
            &N1_FAIL.replace("subject: $huur", "subject: $referencedate"),
            Rule::N1,
            "'referencedate' is not nullable",
        );
    }

    // -- N2 -----------------------------------------------------------------

    #[test]
    fn n2_null_literal_only_as_the_value_of_a_nullable_output() {
        let direct = r#"
output:
  - name: klasse
    type: string
    nullable: true
actions:
  - output: klasse
    value: null
"#;
        assert_clean(direct);
        assert_only(
            &direct.replace("    nullable: true\n", ""),
            Rule::N2,
            "literal null may only be the value of a nullable output",
        );
        let branch = r#"
input:
  - name: huur
    type: amount
    source: {}
output:
  - name: klasse
    type: string
    nullable: true
actions:
  - output: klasse
    value:
      operation: IF
      cases:
        - when:
            operation: GREATER_THAN
            subject: $huur
            value: 500
          then: hoog
      default:
        operation: IF
        cases:
          - when:
              operation: GREATER_THAN
              subject: $huur
              value: 100
            then: laag
        default: null
"#;
        assert_clean(branch);
        // `default: null` deserializes to no default at all, so the nested IF
        // is one without default: N3 reports it, with the same remedy.
        assert_only(
            &branch.replace("    nullable: true\n", ""),
            Rule::N3,
            "IF without default (or with `default: null`)",
        );
        // `then: null` is a literal null in a branch.
        assert_only(
            &branch
                .replace("    nullable: true\n", "")
                .replace("then: hoog", "then: null")
                .replace("default: null", "default: laag"),
            Rule::N2,
            "literal null",
        );
    }

    #[test]
    fn n2_null_literal_as_an_operand_or_a_definition_is_an_error() {
        let operand = r#"
input:
  - name: huur
    type: amount
    source: {}
output:
  - name: totaal
    type: amount
actions:
  - output: totaal
    operation: ADD
    values:
      - $huur
      - null
"#;
        assert_only(operand, Rule::N2, "literal null");
        let definition = r#"
$id: t
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      definitions:
        GRENS: null
      execution:
        output:
          - name: x
            type: number
        actions:
          - output: x
            value: 1
"#;
        let findings = check_law_alone(&ArticleBasedLaw::from_yaml_str(definition).unwrap());
        assert_eq!(rules(&findings), vec![Rule::N2], "{findings:#?}");
        assert!(findings[0].message.contains("definition 'GRENS' is null"));
        assert_eq!(findings[0].location, "definition 'GRENS'");
    }

    #[test]
    fn n2_null_inside_a_container_literal_is_a_value() {
        // An array holding an absence is a value (RFC-036 compares containers
        // element by element): a LIST item, a member of IN's list or the body
        // of a FOREACH without combine may be null, whatever the output
        // declares. With a combine the body is calculated with, and the null
        // is an operand.
        let base = r#"
input:
  - name: kinderen
    type: array
    source: {}
output:
  - name: data
    type: array
actions:
  - output: data
"#;
        assert_clean(&format!(
            "{base}    value:\n      operation: LIST\n      items:\n        - null\n        - 1\n"
        ));
        // `IN $x [null, 650]` is a membership test for absence.
        assert_clean(
            &format!(
                "{base}    operation: IN\n    subject: $huur\n    values:\n      - 650\n      - null\n"
            )
            .replace("name: kinderen\n    type: array", "name: huur\n    type: amount\n    nullable: true")
            .replace("type: array\nactions", "type: boolean\nactions"),
        );
        assert_clean(&format!(
            "{base}    value:\n      operation: FOREACH\n      collection: $kinderen\n      as: k\n      body: null\n"
        ));
        assert_only(
            &format!(
                "{base}    value:\n      operation: FOREACH\n      collection: $kinderen\n      as: k\n      body: null\n      combine: ADD\n"
            )
            .replace("type: array\nactions", "type: number\nactions"),
            Rule::N2,
            "literal null",
        );
    }

    // -- N3 -----------------------------------------------------------------

    const N3_LAW: &str = r#"
input:
  - name: huur
    type: amount
    source: {}
output:
  - name: klasse
    type: string
actions:
  - output: klasse
    value:
      operation: IF
      cases:
        - when:
            operation: GREATER_THAN
            subject: $huur
            value: 500
          then: hoog
"#;

    #[test]
    fn n3_if_without_default_needs_a_nullable_output() {
        assert_only(N3_LAW, Rule::N3, "may yield null; add a default");
        assert_clean(&N3_LAW.replace("type: string\n", "type: string\n    nullable: true\n"));
        assert_clean(&N3_LAW.replace("then: hoog\n", "then: hoog\n      default: laag\n"));
        // A final `when: true` is a default written differently.
        assert_clean(&N3_LAW.replace(
            "then: hoog\n",
            "then: hoog\n        - when: true\n          then: laag\n",
        ));
    }

    #[test]
    fn n3_if_without_default_nested_in_an_operation_is_an_error_even_for_a_nullable_output() {
        let nested = N3_LAW
            .replace("type: string\n", "type: string\n    nullable: true\n")
            .replace(
                "    value:\n      operation: IF\n",
                "    operation: ADD\n    values:\n     - 1\n     - operation: IF\n",
            )
            .replace("      cases:\n        - when:\n            operation: GREATER_THAN\n            subject: $huur\n            value: 500\n          then: hoog\n",
                     "       cases:\n         - when:\n             operation: GREATER_THAN\n             subject: $huur\n             value: 500\n           then: 5\n");
        let findings = check(&nested);
        assert!(
            rules(&findings).contains(&Rule::N3),
            "expected N3 in {findings:#?}"
        );
    }

    // -- N4 -----------------------------------------------------------------

    /// The nullable rent, used in `ADD` under the given guard expression
    /// (`{guard}` is substituted into the IF's `when`).
    fn n4_law(actions: &str) -> String {
        format!(
            r#"
input:
  - name: huur
    type: amount
    nullable: true
    source: {{}}
  - name: partner
    type: object
    nullable: true
    source: {{}}
output:
  - name: uitkomst
    type: amount
actions:
{actions}"#
        )
    }

    #[test]
    fn n4_nullable_operand_without_a_guard_is_an_error() {
        let unguarded = n4_law(
            "  - output: uitkomst\n    operation: ADD\n    values:\n      - $huur\n      - 100\n",
        );
        let finding = assert_only(
            &unguarded,
            Rule::N4,
            "'huur' may be absent and is used in ADD",
        );
        assert_eq!(finding.location, "output 'uitkomst'");
        // A property of a nullable record may be absent too (a field of no
        // record is no record).
        let property = n4_law(
            "  - output: uitkomst\n    operation: ADD\n    values:\n      - $partner.inkomen\n      - 100\n",
        );
        assert_only(&property, Rule::N4, "'partner.inkomen' may be absent");
        // Every operand position that needs a present value.
        for (op, body) in [
            ("GREATER_THAN", "    operation: GREATER_THAN\n    subject: $huur\n    value: 100\n"),
            ("NOT", "    operation: NOT\n    value: $huur\n"),
            ("AND", "    operation: AND\n    conditions:\n      - true\n      - $huur\n"),
            ("ROUND", "    operation: ROUND\n    value: $huur\n    precision: 0\n"),
            ("IF condition", "    value:\n      operation: IF\n      cases:\n        - when: $huur\n          then: 1\n      default: 2\n"),
            ("AGE", "    value:\n      operation: AGE\n      date_of_birth: $huur\n      reference_date: $referencedate\n"),
        ] {
            let yaml = n4_law(&format!("  - output: uitkomst\n{body}"));
            let findings = check(&yaml);
            assert!(
                findings.iter().any(|f| f.rule == Rule::N4 && f.message.contains(op)),
                "{op}: {findings:#?}"
            );
        }
        // A FOREACH over a nullable collection is not in the list: a null
        // collection iterates nothing (RFC-036).
        assert_clean(&n4_law(
            "  - output: uitkomst\n    value:\n      operation: FOREACH\n      collection: $partner\n      as: x\n      body: 1\n      combine: ADD\n",
        ));
    }

    #[test]
    fn n4_assigning_a_nullable_value_to_a_non_nullable_output_is_an_error() {
        // The bare pass-through `value: $huur` would make `uitkomst` null at
        // run time (NullOutput); the checker refuses it where it is written.
        let finding = assert_only(
            &n4_law("  - output: uitkomst\n    value: $huur\n"),
            Rule::N4,
            "'huur' may be absent and is assigned to 'uitkomst', which is never absent",
        );
        assert_eq!(finding.location, "output 'uitkomst'");
        // The same through a branch of the output's IF, guarded or not.
        let branch = |when: &str| {
            n4_law(&format!(
                "  - output: uitkomst\n    value:\n      operation: IF\n      cases:\n        - when:\n{when}          then: $huur\n      default: 0\n"
            ))
        };
        assert_only(
            &branch("            operation: GREATER_THAN\n            subject: 1\n            value: 0\n"),
            Rule::N4,
            "is assigned to 'uitkomst'",
        );
        assert_clean(&branch(
            "            operation: NOT\n            value:\n              operation: EQUALS\n              subject: $huur\n              value: null\n",
        ));
        assert_only(
            &branch("            operation: EQUALS\n            subject: $huur\n            value: null\n"),
            Rule::N4,
            "'huur' is absent on this path",
        );
        // To a nullable output the pass-through is fine, and a property of a
        // record makes no claim.
        assert_clean(&n4_law("  - output: uitkomst\n    value: $huur\n").replace(
            "type: amount\nactions",
            "type: amount\n    nullable: true\nactions",
        ));
        assert_clean(&n4_law(
            "  - output: uitkomst\n    value:\n      operation: IF\n      cases:\n        - when:\n            operation: NOT\n            value:\n              operation: EQUALS\n              subject: $partner\n              value: null\n          then: $partner.inkomen\n      default: 0\n",
        ));
    }

    #[test]
    fn n4_flow_e_the_fact_reaches_a_decision_declared_before_the_absence_test() {
        // The engine runs an action after the outputs it reads, so the fact
        // of `heeft_huur` reaches `uitkomst` also where the file declares the
        // decision first.
        let law = n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when: $heeft_huur
          then:
            operation: ADD
            values:
              - $huur
              - 100
      default: 0
  - output: heeft_huur
    operation: NOT
    value:
      operation: EQUALS
      subject: $huur
      value: null
"#,
        )
        .replace(
            "output:\n  - name: uitkomst",
            "output:\n  - name: heeft_huur\n    type: boolean\n  - name: uitkomst",
        );
        assert_clean(&law);
    }

    #[test]
    fn n4_flow_e_a_boolean_output_that_is_an_absence_test_carries_its_facts() {
        // The corpus idiom: `heeft_huur = NOT(EQUALS $huur null)`, then a
        // decision on `$heeft_huur`. The output carries the fact into every
        // condition that references it, also negated and inside AND.
        let idiom = |decision: &str| {
            n4_law(&format!(
                r#"  - output: heeft_huur
    operation: NOT
    value:
      operation: EQUALS
      subject: $huur
      value: null
  - output: uitkomst
    value:
{decision}"#
            ))
            .replace(
                "output:\n  - name: uitkomst",
                "output:\n  - name: heeft_huur\n    type: boolean\n  - name: uitkomst",
            )
        };
        assert_clean(&idiom(
            "      operation: IF\n      cases:\n        - when: $heeft_huur\n          then:\n            operation: ADD\n            values:\n              - $huur\n              - 100\n      default: 0\n",
        ));
        assert_clean(&idiom(
            "      operation: IF\n      cases:\n        - when:\n            operation: NOT\n            value: $heeft_huur\n          then: 0\n      default:\n        operation: ADD\n        values:\n          - $huur\n          - 100\n",
        ));
        assert_clean(&idiom(
            "      operation: IF\n      cases:\n        - when:\n            operation: AND\n            conditions:\n              - $heeft_huur\n              - operation: GREATER_THAN\n                subject: $huur\n                value: 500\n          then: $huur\n      default: 0\n",
        ));
        // Where the output says "absent", the variable is absent.
        assert_only(
            &idiom(
                "      operation: IF\n      cases:\n        - when: $heeft_huur\n          then: 0\n      default:\n        operation: ADD\n        values:\n          - $huur\n          - 100\n",
            ),
            Rule::N4,
            "'huur' is absent on this path",
        );
        // A boolean output that is not an absence test carries nothing, and
        // an output assigned twice is ambiguous and carries nothing either.
        assert_only(
            &idiom(
                "      operation: IF\n      cases:\n        - when: $heeft_huur\n          then:\n            operation: ADD\n            values:\n              - $huur\n              - 100\n      default: 0\n",
            )
            .replace(
                "    operation: NOT\n    value:\n      operation: EQUALS\n      subject: $huur\n      value: null\n",
                "    operation: EQUALS\n    subject: $huur\n    value: 650\n",
            ),
            Rule::N4,
            "'huur' may be absent and is used in ADD",
        );
        assert_only(
            &idiom(
                "      operation: IF\n      cases:\n        - when: $heeft_huur\n          then:\n            operation: ADD\n            values:\n              - $huur\n              - 100\n      default: 0\n",
            )
            .replace(
                "  - output: uitkomst\n",
                "  - output: heeft_huur\n    value: true\n  - output: uitkomst\n",
            ),
            Rule::N4,
            "'huur' may be absent and is used in ADD",
        );
    }

    #[test]
    fn n4_flow_f_a_foreach_filter_guards_its_body() {
        // The body runs only for the elements the filter let through, so a
        // guard on an outer variable in the filter holds in the body.
        let law = n4_law(
            r#"  - output: uitkomst
    value:
      operation: FOREACH
      collection: $kinderen
      as: kind
      filter:
        operation: NOT
        value:
          operation: EQUALS
          subject: $huur
          value: null
      body:
        operation: ADD
        values:
          - $huur
          - $kind.bedrag
      combine: ADD
"#,
        )
        .replace(
            "  - name: partner\n",
            "  - name: kinderen\n    type: array\n    source: {}\n  - name: partner\n",
        );
        assert_clean(&law);
        // Without the filter the body is unguarded.
        let unguarded = law.replace(
            "      filter:\n        operation: NOT\n        value:\n          operation: EQUALS\n          subject: $huur\n          value: null\n",
            "",
        );
        assert!(unguarded != law);
        assert_only(
            &unguarded,
            Rule::N4,
            "'huur' may be absent and is used in ADD",
        );
    }

    #[test]
    fn a_foreach_without_as_makes_no_claim_about_a_bare_name_in_its_body() {
        // Without `as`, an object element binds its fields under their own
        // names, so `$huur` in the body may be the element's field rather
        // than the declared input, and the checker claims nothing. With
        // `as`, only that name is bound (RFC-049), and `$huur` is the input.
        let with_as = n4_law(
            r#"  - output: uitkomst
    value:
      operation: FOREACH
      collection: $kinderen
      as: kind
      body:
        operation: ADD
        values:
          - $huur
          - $kind.bedrag
      combine: ADD
"#,
        )
        .replace(
            "  - name: partner\n",
            "  - name: kinderen\n    type: array\n    source: {}\n  - name: partner\n",
        );
        assert_only(
            &with_as,
            Rule::N4,
            "'huur' may be absent and is used in ADD",
        );
        let without_as = with_as
            .replace("      as: kind\n", "")
            .replace("$kind.bedrag", "$item.bedrag");
        assert!(without_as != with_as);
        assert_clean(&without_as);
        // The claim comes back in a nested FOREACH with `as`: its body runs in
        // a scope of its own, where the outer element's fields are not bound.
        let nested = n4_law(
            r#"  - output: uitkomst
    value:
      operation: FOREACH
      collection: $kinderen
      body:
        operation: FOREACH
        collection: $item.leden
        as: lid
        body:
          operation: ADD
          values:
            - $huur
            - $lid.bedrag
        combine: ADD
      combine: ADD
"#,
        )
        .replace(
            "  - name: partner\n",
            "  - name: kinderen\n    type: array\n    source: {}\n  - name: partner\n",
        );
        assert_only(&nested, Rule::N4, "'huur' may be absent and is used in ADD");
    }

    #[test]
    fn n4_flow_g_a_present_field_implies_a_present_record() {
        // A field of no record is no record (RFC-036), so a guard on
        // `$partner.a` establishes `partner`, and thereby `partner.b`.
        assert_clean(&n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: NOT
            value:
              operation: EQUALS
              subject: $partner.inkomen
              value: null
          then:
            operation: ADD
            values:
              - $partner.vermogen
              - 100
      default: 0
"#,
        ));
        // The converse does not hold: an absent field says nothing about the
        // record, and the record's other field may still be absent.
        assert_only(
            &n4_law(
                r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: EQUALS
            subject: $partner.inkomen
            value: null
          then:
            operation: ADD
            values:
              - $partner.vermogen
              - 100
      default: 0
"#,
            ),
            Rule::N4,
            "'partner.vermogen' may be absent",
        );
        // And a field of a record established absent is absent.
        assert_only(
            &n4_law(
                r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: EQUALS
            subject: $partner
            value: null
          then:
            operation: ADD
            values:
              - $partner.vermogen
              - 100
      default: 0
"#,
            ),
            Rule::N4,
            "'partner.vermogen' is absent on this path",
        );
    }

    #[test]
    fn n1_a_redundant_test_inside_its_own_guard_says_so() {
        let finding = assert_only(
            &n4_law(
                r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: NOT
            value:
              operation: EQUALS
              subject: $huur
              value: null
          then:
            operation: IF
            cases:
              - when:
                  operation: EQUALS
                  subject: $huur
                  value: null
                then: 1
            default: 2
      default: 0
"#,
            ),
            Rule::N1,
            "'huur' is already established present on this path",
        );
        assert!(finding.message.contains("the test is redundant"));
    }

    #[test]
    fn identical_findings_are_reported_once() {
        let findings = check(
            r#"
input:
  - name: naam
    type: string
    source: {}
output:
  - name: x
    type: number
actions:
  - output: x
    operation: MULTIPLY
    values:
      - $naam
      - $naam
"#,
        );
        assert_eq!(rules(&findings), vec![Rule::T1], "{findings:#?}");
    }

    #[test]
    fn n4_flow_a_then_of_not_equals_null_sees_the_variable_present() {
        assert_clean(&n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: NOT
            value:
              operation: EQUALS
              subject: $huur
              value: null
          then:
            operation: ADD
            values:
              - $huur
              - 100
      default: 0
"#,
        ));
        assert_clean(&n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: NOT_NULL
            subject: $huur
          then:
            operation: ADD
            values:
              - $huur
              - 100
      default: 0
"#,
        ));
    }

    #[test]
    fn n4_flow_b_cases_after_equals_null_and_the_default_see_the_variable_present() {
        assert_clean(&n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: EQUALS
            subject: $huur
            value: null
          then: 0
        - when:
            operation: GREATER_THAN
            subject: $huur
            value: 500
          then: $huur
      default:
        operation: ADD
        values:
          - $huur
          - 100
"#,
        ));
        assert_clean(&n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: IS_NULL
            subject: $huur
          then: 0
      default:
        operation: ADD
        values:
          - $huur
          - 100
"#,
        ));
    }

    #[test]
    fn n4_flow_c_the_then_of_equals_null_sees_the_variable_absent() {
        assert_only(
            &n4_law(
                r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: EQUALS
            subject: $huur
            value: null
          then:
            operation: ADD
            values:
              - $huur
              - 100
      default: 0
"#,
            ),
            Rule::N4,
            "'huur' is absent on this path",
        );
    }

    #[test]
    fn n4_flow_d_and_after_a_guard_and_or_after_an_absence_test_see_the_variable_present() {
        let law = n4_law(
            r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: AND
            conditions:
              - operation: NOT
                value:
                  operation: EQUALS
                  subject: $huur
                  value: null
              - operation: GREATER_THAN
                subject: $huur
                value: 500
          then: 1
      default: 0
"#,
        )
        .replace("type: amount\nactions", "type: number\nactions");
        assert_clean(&law);
        let or = law.replace(
            "operation: AND\n            conditions:\n              - operation: NOT\n                value:\n                  operation: EQUALS\n                  subject: $huur\n                  value: null\n",
            "operation: OR\n            conditions:\n              - operation: EQUALS\n                subject: $huur\n                value: null\n",
        );
        assert!(or.contains("operation: OR"), "{or}");
        assert_clean(&or);
        // The order matters: a guard after the comparison protects nothing.
        let reversed = law.replace(
            "              - operation: NOT\n                value:\n                  operation: EQUALS\n                  subject: $huur\n                  value: null\n              - operation: GREATER_THAN\n                subject: $huur\n                value: 500\n",
            "              - operation: GREATER_THAN\n                subject: $huur\n                value: 500\n              - operation: NOT\n                value:\n                  operation: EQUALS\n                  subject: $huur\n                  value: null\n",
        );
        assert!(reversed != law);
        assert_only(
            &reversed,
            Rule::N4,
            "'huur' may be absent and is used in GREATER_THAN",
        );
    }

    #[test]
    fn n4_passing_a_nullable_value_on_is_fine() {
        // Into EQUALS or IN on either side (both are structural on an
        // absence), as a cross-law parameter, or straight through to a
        // nullable output.
        assert_clean(
            &n4_law(
                r#"  - output: uitkomst
    value:
      operation: IF
      cases:
        - when:
            operation: EQUALS
            subject: $huur
            value: 650
          then: 1
        - when:
            operation: IN
            subject: 650
            values:
              - $huur
          then: 2
        - when:
            operation: IN
            subject: $huur
            values:
              - 650
              - 700
          then: 3
      default: $huur
"#,
            )
            .replace(
                "type: amount\nactions",
                "type: amount\n    nullable: true\nactions",
            ),
        );
    }

    // -- N5 -----------------------------------------------------------------

    #[test]
    fn n5_a_nullable_output_of_another_law_needs_a_nullable_input() {
        let bron = law(
            "bron",
            r#"
output:
  - name: rendementsgrondslag
    type: amount
    nullable: true
actions:
  - output: rendementsgrondslag
    value: null
"#,
        );
        let afnemer = r#"
input:
  - name: vermogen
    type: amount
    source:
      regulation: bron
      output: rendementsgrondslag
output:
  - name: x
    type: amount
actions:
  - output: x
    value: 1
"#;
        let lookup = |id: &str| (id == "bron").then_some(&bron);
        let strict = law("afnemer", afnemer);
        let findings = check_law(&strict, &lookup);
        assert_eq!(rules(&findings), vec![Rule::N5], "{findings:#?}");
        assert!(
            findings[0]
                .message
                .contains("'vermogen' takes 'rendementsgrondslag' from bron, which may be null"),
            "{}",
            findings[0].message
        );
        assert_eq!(findings[0].location, "input 'vermogen'");
        // Declared nullable: fine. Target not available: silent.
        let nullable = law(
            "afnemer",
            &afnemer.replace(
                "type: amount\n    source",
                "type: amount\n    nullable: true\n    source",
            ),
        );
        assert!(check_law(&nullable, &lookup).is_empty());
        assert!(check_law_alone(&strict).is_empty());
    }

    #[test]
    fn n5_a_skipped_call_on_a_nullable_key_needs_a_nullable_input() {
        let bron = law(
            "bron",
            r#"
parameters:
  - name: bsn
    type: string
    required: true
output:
  - name: geboortejaar
    type: number
actions:
  - output: geboortejaar
    value: 1980
"#,
        );
        let afnemer = r#"
input:
  - name: partner_bsn
    type: string
    nullable: true
    source: {}
  - name: partner_geboortejaar
    type: number
    source:
      regulation: bron
      output: geboortejaar
      parameters:
        bsn: $partner_bsn
output:
  - name: x
    type: number
actions:
  - output: x
    value: 1
"#;
        let lookup = |id: &str| (id == "bron").then_some(&bron);
        let afnemer = law("afnemer", afnemer);
        let findings = check_law(&afnemer, &lookup);
        assert_eq!(rules(&findings), vec![Rule::N5], "{findings:#?}");
        assert!(
            findings[0]
                .message
                .contains("'partner_geboortejaar' is null when 'partner_bsn' is null"),
            "{}",
            findings[0].message
        );
        // An optional parameter is passed through, not skipped on.
        let optional_bron = law(
            "bron",
            r#"
parameters:
  - name: bsn
    type: string
    required: false
    nullable: true
output:
  - name: geboortejaar
    type: number
actions:
  - output: geboortejaar
    value: 1980
"#,
        );
        let lookup = |id: &str| (id == "bron").then_some(&optional_bron);
        assert!(check_law(&afnemer, &lookup).is_empty());
        // A required parameter the target declares nullable is not skipped
        // on: the target said it can decide on nobody, and is run.
        let nullable_bron = law(
            "bron",
            r#"
parameters:
  - name: bsn
    type: string
    required: true
    nullable: true
output:
  - name: geboortejaar
    type: number
actions:
  - output: geboortejaar
    value: 1980
"#,
        );
        let lookup = |id: &str| (id == "bron").then_some(&nullable_bron);
        assert!(check_law(&afnemer, &lookup).is_empty());
        // A parameter the target does not declare at all is required.
        let undeclared_bron = law(
            "bron",
            r#"
output:
  - name: geboortejaar
    type: number
actions:
  - output: geboortejaar
    value: 1980
"#,
        );
        let lookup = |id: &str| (id == "bron").then_some(&undeclared_bron);
        assert_eq!(rules(&check_law(&afnemer, &lookup)), vec![Rule::N5]);
    }

    #[test]
    fn n5_a_property_path_argument_is_null_when_its_record_is() {
        let bron = law(
            "bron",
            r#"
parameters:
  - name: bsn
    type: string
    required: true
output:
  - name: geboortejaar
    type: number
actions:
  - output: geboortejaar
    value: 1980
"#,
        );
        let afnemer = law(
            "afnemer",
            r#"
input:
  - name: partner
    type: object
    nullable: true
    source: {}
  - name: partner_geboortejaar
    type: number
    source:
      regulation: bron
      output: geboortejaar
      parameters:
        bsn: $partner.bsn
output:
  - name: x
    type: number
actions:
  - output: x
    value: 1
"#,
        );
        let lookup = |id: &str| (id == "bron").then_some(&bron);
        let findings = check_law(&afnemer, &lookup);
        assert_eq!(rules(&findings), vec![Rule::N5], "{findings:#?}");
        assert!(
            findings[0].message.contains(
                "'partner_geboortejaar' is null when 'partner' is null (and so is 'partner.bsn')"
            ),
            "{}",
            findings[0].message
        );
        // A field of a record that is never absent: no claim about the field.
        let strict = law(
            "afnemer",
            r#"
input:
  - name: partner
    type: object
    source: {}
  - name: partner_geboortejaar
    type: number
    source:
      regulation: bron
      output: geboortejaar
      parameters:
        bsn: $partner.bsn
output:
  - name: x
    type: number
actions:
  - output: x
    value: 1
"#,
        );
        assert!(check_law(&strict, &lookup).is_empty());
    }

    #[test]
    fn n5_an_implementation_of_a_required_term_without_default_may_not_be_nullable() {
        let delegating = |term: &str| {
            ArticleBasedLaw::from_yaml_str(&format!(
                r#"
$id: wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: t
    machine_readable:
      open_terms:
        - id: premie
          type: number
{term}
      execution:
        output:
          - name: premie
            type: number
        actions:
          - output: premie
            value: $premie
"#
            ))
            .unwrap()
        };
        let implementing = ArticleBasedLaw::from_yaml_str(
            r#"
$id: regeling
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      implements:
        - law: wet
          article: '4'
          open_term: premie
      execution:
        parameters:
          - name: soort
            type: string
            required: true
        output:
          - name: premie
            type: number
            nullable: true
        actions:
          - output: premie
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $soort
                    value: basis
                  then: 1928
"#,
        )
        .unwrap();
        // Required, no default: the implementation's null would be the term's
        // value, and the delegating law reads the term as never absent.
        let required = delegating("          required: true");
        let lookup = |id: &str| (id == "wet").then_some(&required);
        let findings = check_law(&implementing, &lookup);
        assert_eq!(rules(&findings), vec![Rule::N5], "{findings:#?}");
        assert_eq!(findings[0].location, "output 'premie'");
        assert!(
            findings[0]
                .message
                .contains("'premie' implements the required term 'premie' of wet article 4"),
            "{}",
            findings[0].message
        );
        // With a default the silent implementation falls back to it; an
        // optional term without default is nullable itself; the delegating
        // law not in view: silent.
        let with_default = delegating(
            "          required: true\n          default:\n            actions:\n              - output: premie\n                value: 100",
        );
        let lookup = |id: &str| (id == "wet").then_some(&with_default);
        assert!(check_law(&implementing, &lookup).is_empty());
        let optional = delegating("          required: false");
        let lookup = |id: &str| (id == "wet").then_some(&optional);
        assert!(check_law(&implementing, &lookup).is_empty());
        assert!(check_law_alone(&implementing).is_empty());
    }

    // -- mutation exposure ------------------------------------------------------

    #[test]
    fn referencedate_fields_are_typed() {
        let base = r#"
input:
  - name: aantal
    type: number
    source: {}
output:
  - name: x
    type: boolean
actions:
  - output: x
"#;
        // `.year`, `.month`, `.day` are numbers: they add up and compare with
        // a number.
        assert_clean(
            &format!("{base}    operation: ADD\n    values:\n      - $referencedate.year\n      - $referencedate.month\n      - $referencedate.day\n      - $aantal\n")
                .replace("type: boolean\nactions", "type: number\nactions"),
        );
        assert_only(
            &format!(
                "{base}    operation: EQUALS\n    subject: $referencedate.year\n    value: aap\n"
            ),
            Rule::T3,
            "compares 'referencedate.year' (number) with the literal (string)",
        );
        // `.iso` is a string, and orders like a date.
        assert_only(
            &format!("{base}    operation: GREATER_THAN\n    subject: $referencedate.iso\n    value: 2025\n"),
            Rule::T1,
            "orders 'referencedate.iso' (string) against the literal (number)",
        );
        assert_clean(&format!(
            "{base}    operation: GREATER_THAN\n    subject: $referencedate.iso\n    value: '2025-01-01'\n"
        ));
        // An unknown field of the reference date: no claim.
        assert_clean(&format!(
            "{base}    operation: EQUALS\n    subject: $referencedate.week\n    value: aap\n"
        ));
    }

    #[test]
    fn an_if_whose_branches_disagree_on_type_makes_no_claim() {
        let base = r#"
input:
  - name: aantal
    type: number
    source: {}
output:
  - name: x
    type: number
actions:
  - output: x
    operation: ADD
    values:
      - 1
      - operation: IF
        cases:
          - when:
              operation: GREATER_THAN
              subject: $aantal
              value: 5
            then: aap
        default: DEFAULT
"#;
        // Agreeing branches type the IF: two strings into ADD with a number
        // is T1.
        assert_only(
            &base.replace("default: DEFAULT", "default: noot"),
            Rule::T1,
            "combines a number with the IF (string)",
        );
        // Disagreeing branches: the IF has no type, so no claim.
        assert_clean(&base.replace("default: DEFAULT", "default: 2"));
    }

    #[test]
    fn only_a_final_when_true_makes_an_if_exhaustive() {
        assert_only(
            &N3_LAW.replace(
                "      cases:\n        - when:\n            operation: GREATER_THAN\n            subject: $huur\n            value: 500\n          then: hoog\n",
                "      cases:\n        - when: true\n          then: laag\n        - when:\n            operation: GREATER_THAN\n            subject: $huur\n            value: 500\n          then: hoog\n",
            ),
            Rule::N3,
            "may yield null",
        );
    }

    #[test]
    fn a_number_joining_an_amount_is_an_amount() {
        let base = r#"
input:
  - name: aantal
    type: number
    source: {}
  - name: bedrag
    type: amount
    source: {}
output:
  - name: x
    type: boolean
actions:
  - output: x
    operation: EQUALS
    subject:
      operation: ADD
      values:
        - $aantal
        - $bedrag
    value: aap
"#;
        assert_only(
            base,
            Rule::T3,
            "compares the ADD (amount) with the literal (string)",
        );
        // Two numbers stay a number.
        assert_only(
            &base.replace("        - $bedrag\n", "        - $aantal\n"),
            Rule::T3,
            "compares the ADD (number)",
        );
    }

    #[test]
    fn an_open_term_with_an_empty_default_has_no_default() {
        // `default: {}` resolves to null in the engine ("using empty default"),
        // so the optional term is nullable and may be tested for absence.
        for default in [
            "          default: {}",
            "          default:\n            actions: []",
        ] {
            let yaml = format!(
                r#"
$id: bw
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '42'
    text: t
    machine_readable:
      open_terms:
        - id: afstand
          type: number
          required: false
{default}
      execution:
        output:
          - name: geen_afstand
            type: boolean
        actions:
          - output: geen_afstand
            value:
              operation: EQUALS
              subject: $afstand
              value: null
"#
            );
            let law = ArticleBasedLaw::from_yaml_str(&yaml).unwrap();
            assert!(check_law_alone(&law).is_empty(), "{default}");
        }
    }

    #[test]
    fn a_date_and_a_string_are_compatible_in_either_order() {
        let base = r#"
input:
  - name: datum
    type: date
    source: {}
output:
  - name: x
    type: boolean
actions:
  - output: x
"#;
        assert_clean(&format!(
            "{base}    operation: EQUALS\n    subject: '2025-01-01'\n    value: $datum\n"
        ));
        assert_clean(&format!(
            "{base}    operation: EQUALS\n    subject: $datum\n    value: '2025-01-01'\n"
        ));
        assert_clean(&format!(
            "{base}    operation: GREATER_THAN\n    subject: '2025-01-01'\n    value: $datum\n"
        ));
    }

    #[test]
    fn n5_same_law_reference_is_checked_without_a_lookup() {
        let yaml = r#"
$id: zelf
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: klasse
            type: string
            nullable: true
        actions:
          - output: klasse
            value: null
  - number: '2'
    text: t
    machine_readable:
      execution:
        input:
          - name: klasse
            type: string
            source:
              output: klasse
        output:
          - name: x
            type: number
        actions:
          - output: x
            value: 1
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let findings = check_law_alone(&law);
        assert_eq!(rules(&findings), vec![Rule::N5], "{findings:#?}");
        assert_eq!(findings[0].article, "2");
    }

    #[test]
    fn a_bare_absence_test_guards_as_well_as_the_negated_one() {
        // The corpus writes the guard as `NOT(EQUALS … null)`, but the four
        // bare forms decide the same thing, and each of them establishes its
        // fact in both directions. `NOT_EQUALS`/`NOT_NULL` hold where the
        // field is present, `EQUALS`/`IS_NULL` where it is absent, so the
        // branch that runs on the holding side is the guarded one for the
        // first pair and the unguarded one for the second.
        let guard = |when: &str, then: &str, default: &str| {
            n4_law(&format!(
                "  - output: uitkomst\n    value:\n      operation: IF\n      cases:\n        - when:\n{when}          then: {then}\n      default: {default}\n"
            ))
        };
        let sum = "\n            operation: ADD\n            values:\n              - $huur\n              - 100";
        let sum_in_default =
            "\n        operation: ADD\n        values:\n          - $huur\n          - 100";
        for present_when_true in [
            "            operation: NOT_EQUALS\n            subject: $huur\n            value: null\n",
            "            operation: NOT_NULL\n            subject: $huur\n",
        ] {
            assert_clean(&guard(present_when_true, sum, "0"));
            // And absent on the other side, which the message names.
            assert_only(
                &guard(present_when_true, "0", sum_in_default),
                Rule::N4,
                "'huur' is absent on this path",
            );
        }
        for absent_when_true in [
            "            operation: EQUALS\n            subject: $huur\n            value: null\n",
            "            operation: IS_NULL\n            subject: $huur\n",
        ] {
            assert_only(
                &guard(absent_when_true, sum, "0"),
                Rule::N4,
                "'huur' is absent on this path",
            );
            assert_clean(&guard(absent_when_true, "0", sum_in_default));
        }
    }

    #[test]
    fn an_absence_test_is_read_from_the_operation_and_not_from_the_text_of_a_literal() {
        // The subject of an absence test only establishes a fact when it is a
        // reference. A plain string that happens to read like a variable name
        // with a character in front of it is a literal: `NOT_NULL('xhuur')`
        // is an N1 about a literal, and it says nothing about 'huur'.
        let findings = check(&n4_law(
            "  - output: uitkomst\n    value:\n      operation: IF\n      cases:\n        - when:\n            operation: NOT_NULL\n            subject: xhuur\n          then:\n            operation: ADD\n            values:\n              - $huur\n              - 100\n      default: 0\n",
        ));
        assert_eq!(rules(&findings), vec![Rule::N1, Rule::N4], "{findings:#?}");
        assert!(findings[0].message.contains("the literal is not nullable"));
        assert!(findings[1].message.contains("'huur' may be absent"));
    }

    #[test]
    fn ordering_a_number_against_a_boolean_reports_the_boolean_and_stops_there() {
        // An operand that is not orderable at all is one mistake: naming the
        // two sides as "not the same kind of value" on top of it would be a
        // second message about the same operand.
        let base = r#"
input:
  - name: aantal
    type: number
    source: {}
  - name: vlag
    type: boolean
    source: {}
output:
  - name: x
    type: boolean
actions:
  - output: x
    operation: GREATER_THAN
    subject: $aantal
    value: $vlag
"#;
        assert_only(
            base,
            Rule::T1,
            "'vlag' is a boolean and is used in GREATER_THAN",
        );
    }

    #[test]
    fn an_if_without_default_whose_branch_is_unknown_still_may_be_absent() {
        // The missing default is what makes the IF absent, and a branch the
        // checker knows nothing about cannot take that away: "unknown" only
        // weakens a claim of presence, never a claim of absence.
        let base = r#"
input:
  - name: bedragen
    type: array
    source: {}
output:
  - name: totaal
    type: number
actions:
  - output: totaal
    operation: ADD
    values:
      - operation: IF
        cases:
          - when:
              operation: EQUALS
              subject: 1
              value: 1
            then:
              operation: FOREACH
              collection: $bedragen
              as: b
              body: $b
              combine: MIN
      - 1
"#;
        let findings = check(base);
        assert_eq!(rules(&findings), vec![Rule::N3, Rule::N4], "{findings:#?}");
        assert!(findings[1].message.contains("the IF"));
        assert!(findings[1]
            .message
            .contains("may be absent and is used in ADD"));
    }

    #[test]
    fn a_foreach_binding_shadows_an_output_of_the_same_name_and_its_facts() {
        // `heeft_huur` is a boolean output that is an absence test, so a
        // condition on `$heeft_huur` normally guards `$huur`. Inside a FOREACH
        // that binds the same name, `$heeft_huur` is the element, and the
        // element says nothing about 'huur'.
        let foreach = |as_name: &str| {
            n4_law(&format!(
                r#"  - output: heeft_huur
    operation: NOT_NULL
    subject: $huur
  - output: uitkomst
    value:
      operation: FOREACH
      collection: $lijst
      as: {as_name}
      filter: $heeft_huur
      body:
        operation: ADD
        values:
          - $huur
          - 100
      combine: ADD
"#
            ))
            .replace(
                "output:\n  - name: uitkomst",
                "output:\n  - name: heeft_huur\n    type: boolean\n  - name: uitkomst",
            )
            .replace(
                "  - name: partner",
                "  - name: lijst\n    type: array\n    source: {}\n  - name: partner",
            )
        };
        // Bound under another name, the output is itself and guards the body.
        assert_clean(&foreach("k"));
        // Bound under its own name, it is the element: no guard, so the body
        // calculates with a value that may be absent.
        assert_only(&foreach("heeft_huur"), Rule::N4, "'huur' may be absent");
    }

    #[test]
    fn an_output_assigned_twice_carries_no_facts_even_when_both_are_absence_tests() {
        // Which of the two assignments a later condition decides on is not
        // something the checker can see, so the second one drops the facts
        // rather than replacing them, also when it tests the same field.
        let twice = n4_law(
            r#"  - output: heeft_huur
    operation: NOT_NULL
    subject: $huur
  - output: heeft_huur
    operation: NOT_NULL
    subject: $huur
  - output: uitkomst
    value:
      operation: IF
      cases:
        - when: $heeft_huur
          then:
            operation: ADD
            values:
              - $huur
              - 100
      default: 0
"#,
        )
        .replace(
            "output:\n  - name: uitkomst",
            "output:\n  - name: heeft_huur\n    type: boolean\n  - name: uitkomst",
        );
        assert_only(&twice, Rule::N4, "'huur' may be absent and is used in ADD");
    }

    #[test]
    fn an_output_that_establishes_a_fact_in_one_direction_only_still_carries_it() {
        // `AND` learns from its conditions only where it holds, so this output
        // knows 'huur' is present when it is true and nothing when it is
        // false. Half a Derived is a Derived: the true side guards the branch
        // that runs on it.
        let idiom = n4_law(
            r#"  - output: hoge_huur
    operation: AND
    conditions:
      - operation: NOT_NULL
        subject: $huur
      - true
  - output: uitkomst
    value:
      operation: IF
      cases:
        - when: $hoge_huur
          then:
            operation: ADD
            values:
              - $huur
              - 100
      default: 0
"#,
        )
        .replace(
            "output:\n  - name: uitkomst",
            "output:\n  - name: hoge_huur\n    type: boolean\n  - name: uitkomst",
        );
        assert_clean(&idiom);
    }

    #[test]
    fn a_name_declared_twice_is_nullable_when_either_declaration_says_so() {
        // An input that the article also declares as an output: the two
        // declarations meet in one symbol, and the absence the one admits is
        // not talked away by the other. Testing it for absence is therefore
        // no N1.
        let both = r#"
input:
  - name: huur
    type: amount
    source: {}
output:
  - name: huur
    type: amount
    nullable: true
  - name: geen_huur
    type: boolean
actions:
  - output: geen_huur
    value:
      operation: EQUALS
      subject: $huur
      value: null
"#;
        assert_clean(both);
        // The other order declares the same symbol.
        assert_clean(
            &both
                .replace(
                    "  - name: huur\n    type: amount\n    source: {}",
                    "  - name: huur\n    type: amount\n    nullable: true\n    source: {}",
                )
                .replace(
                    "  - name: huur\n    type: amount\n    nullable: true\n  - name: geen_huur",
                    "  - name: huur\n    type: amount\n  - name: geen_huur",
                ),
        );
    }

    // -- T1..T4 ---------------------------------------------------------------

    #[test]
    fn t1_arithmetic_and_ordering_need_numbers() {
        let base = r#"
input:
  - name: naam
    type: string
    source: {}
  - name: bedrag
    type: amount
    source: {}
  - name: datum
    type: date
    source: {}
output:
  - name: x
    type: number
actions:
  - output: x
"#;
        assert_only(
            &format!("{base}    operation: MULTIPLY\n    values:\n      - $naam\n      - 2\n"),
            Rule::T1,
            "'naam' is a string and is used in MULTIPLY",
        );
        assert_only(
            &format!("{base}    operation: GREATER_THAN\n    subject: $naam\n    value: 2\n"),
            Rule::T1,
            "orders 'naam' (string) against the literal (number)",
        );
        // ADD concatenates strings; number and amount mix; dates order.
        assert_clean(
            &format!("{base}    operation: ADD\n    values:\n      - $naam\n      - a\n")
                .replace("type: number\nactions", "type: string\nactions"),
        );
        assert_clean(&format!(
            "{base}    operation: ADD\n    values:\n      - $bedrag\n      - 2\n"
        ));
        assert_clean(
            &format!(
            "{base}    operation: GREATER_THAN\n    subject: $datum\n    value: $referencedate\n"
        )
            .replace("type: number\nactions", "type: boolean\nactions"),
        );
        // A string added to a number is neither concatenation nor a sum.
        assert_only(
            &format!("{base}    operation: ADD\n    values:\n      - $bedrag\n      - $naam\n"),
            Rule::T1,
            "combines a amount with 'naam' (string)",
        );
    }

    #[test]
    fn t2_logic_and_if_conditions_need_booleans() {
        let base = r#"
input:
  - name: bedrag
    type: amount
    source: {}
  - name: ok
    type: boolean
    source: {}
output:
  - name: x
    type: boolean
actions:
  - output: x
"#;
        assert_only(
            &format!("{base}    operation: AND\n    conditions:\n      - $ok\n      - $bedrag\n"),
            Rule::T2,
            "'bedrag' is a amount and is used as a condition in AND",
        );
        assert_only(
            &format!("{base}    operation: NOT\n    value: $bedrag\n"),
            Rule::T2,
            "NOT",
        );
        assert_only(
            &format!("{base}    value:\n      operation: IF\n      cases:\n        - when: $bedrag\n          then: true\n      default: false\n"),
            Rule::T2,
            "condition in IF",
        );
        assert_clean(&format!(
            "{base}    operation: AND\n    conditions:\n      - $ok\n      - true\n"
        ));
    }

    #[test]
    fn t3_equals_between_different_types_is_an_error() {
        let base = r#"
input:
  - name: naam
    type: string
    source: {}
  - name: bedrag
    type: amount
    source: {}
  - name: aantal
    type: number
    source: {}
  - name: datum
    type: date
    source: {}
output:
  - name: x
    type: boolean
actions:
  - output: x
"#;
        assert_only(
            &format!("{base}    operation: EQUALS\n    subject: $bedrag\n    value: $naam\n"),
            Rule::T3,
            "compares 'bedrag' (amount) with 'naam' (string)",
        );
        assert_only(
            &format!("{base}    operation: EQUALS\n    subject: $bedrag\n    value: aap\n"),
            Rule::T3,
            "which can never be equal",
        );
        assert_clean(&format!(
            "{base}    operation: EQUALS\n    subject: $bedrag\n    value: $aantal\n"
        ));
        assert_clean(&format!(
            "{base}    operation: EQUALS\n    subject: $datum\n    value: '2025-01-01'\n"
        ));
        assert_clean(&format!(
            "{base}    operation: EQUALS\n    subject: $naam\n    value: $naam.deel\n"
        ));
    }

    #[test]
    fn t4_a_literal_assigned_to_an_output_has_its_type() {
        let base = r#"
output:
  - name: x
    type: number
actions:
  - output: x
"#;
        let finding = assert_only(
            &format!("{base}    value: aap\n"),
            Rule::T4,
            "output is declared number but is assigned a string literal",
        );
        assert_eq!(finding.location, "output 'x'");
        assert_clean(&format!("{base}    value: 12\n"));
        assert_clean(&format!("{base}    value: 12\n").replace("type: number", "type: amount"));
        // Through the branches of the IF that is the output's value.
        assert_only(
            &format!("{base}    value:\n      operation: IF\n      cases:\n        - when: true\n          then: aap\n"),
            Rule::T4,
            "a branch assigns a string literal",
        );
        // A date output takes an ISO string.
        assert_clean(
            &format!("{base}    value: '2025-01-01'\n").replace("type: number", "type: date"),
        );
    }

    // -- environment and edge cases ---------------------------------------------

    #[test]
    fn a_foreach_binding_and_an_undeclared_name_make_no_claim() {
        assert_clean(
            r#"
input:
  - name: kinderen
    type: array
    source: {}
output:
  - name: totaal
    type: number
actions:
  - output: totaal
    value:
      operation: FOREACH
      collection: $kinderen
      as: kind
      filter:
        operation: NOT
        value:
          operation: EQUALS
          subject: $kind.inkomen
          value: null
      body:
        operation: ADD
        values:
          - $kind.inkomen
          - $onbekend
      combine: ADD
"#,
        );
    }

    #[test]
    fn an_optional_open_term_without_default_is_nullable_and_the_others_are_not() {
        let yaml = r#"
$id: bw
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '42'
    text: t
    machine_readable:
      open_terms:
        - id: gemeentelijke_afstand
          type: number
          required: false
        - id: verplichte_afstand
          type: number
          required: true
        - id: afstand_met_default
          type: number
          required: false
          default:
            actions:
              - output: afstand_met_default
                value: 200
      execution:
        output:
          - name: afstand
            type: number
        actions:
          - output: afstand
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $gemeentelijke_afstand
                    value: null
                  then: 200
              default: $gemeentelijke_afstand
          - output: afstand
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $verplichte_afstand
                    value: null
                  then: 200
              default: $verplichte_afstand
          # A term with a default is filled by it, also when the implementing
          # regulation is silent for the case: never absent.
          - output: afstand
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $afstand_met_default
                    value: null
                  then: 200
              default: $afstand_met_default
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let findings = check_law_alone(&law);
        assert_eq!(rules(&findings), vec![Rule::N1, Rule::N1], "{findings:#?}");
        assert!(findings[0].message.contains("'verplichte_afstand'"));
        assert!(findings[1].message.contains("'afstand_met_default'"));
    }

    #[test]
    fn a_nullable_output_referenced_later_is_a_nullable_variable() {
        // The output of one action is the input of the next; its declared
        // nullability travels with it.
        assert_only(
            r#"
input:
  - name: huur
    type: amount
    source: {}
output:
  - name: klasse
    type: number
    nullable: true
  - name: dubbel
    type: number
actions:
  - output: klasse
    value:
      operation: IF
      cases:
        - when:
            operation: GREATER_THAN
            subject: $huur
            value: 500
          then: 2
  - output: dubbel
    operation: MULTIPLY
    values:
      - $klasse
      - 2
"#,
            Rule::N4,
            "'klasse' may be absent and is used in MULTIPLY",
        );
    }

    #[test]
    fn a_min_or_max_over_a_collection_makes_no_claim_about_absence() {
        // The lowest of nothing is null, and whether the collection is empty
        // is not known here: neither N4 on the ADD nor N1 on the test.
        assert_clean(
            r#"
input:
  - name: bedragen
    type: array
    source: {}
output:
  - name: laagste_plus
    type: number
  - name: geen_laagste
    type: boolean
actions:
  - output: laagste_plus
    operation: ADD
    values:
      - operation: FOREACH
        collection: $bedragen
        as: b
        body: $b
        combine: MIN
      - 1
  - output: geen_laagste
    value:
      operation: EQUALS
      subject:
        operation: FOREACH
        collection: $bedragen
        as: b
        body: $b
        combine: MIN
      value: null
"#,
        );
    }

    #[test]
    fn foreach_only_takes_a_body_of_any_type_where_min_wants_a_number() {
        // ONLY picks the one element the filter leaves, such as the name in
        // a record; MIN and MAX compare, so they want numbers.
        let law = |combine: &str| {
            format!(
                r#"
input:
  - name: namen
    type: array
    source: {{}}
output:
  - name: de_naam
    type: string
    nullable: true
actions:
  - output: de_naam
    value:
      operation: FOREACH
      collection: $namen
      as: n
      body: "Utrecht"
      combine: {combine}
"#
            )
        };
        assert_clean(&law("ONLY"));
        assert!(!check(&law("MIN")).is_empty());
    }

    #[test]
    fn findings_display_names_law_article_location_and_rule() {
        let finding = check(N1_FAIL).into_iter().next().unwrap();
        let text = finding.to_string();
        assert!(
            text.starts_with("t article 1 output 'geen_huur': [N1] "),
            "{text}"
        );
    }
}
