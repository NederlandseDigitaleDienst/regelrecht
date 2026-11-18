//! The signatures of the operations (RFC-055).
//!
//! Every operation has a closed list of signatures: the operand types it
//! accepts and the type it returns. Operands that match no signature are an
//! error. This module holds that list once, for the engine, which holds the
//! values an operation receives against it, and for the type checker, which
//! holds the declared types against it.
//!
//! A [`Kind`] is what a value shows at run time. Whether a number is an
//! `amount` or carries a unit, and whether an object is a `decision`, follows
//! from declarations and is not a kind.
//!
//! `null`, unknown and untranslatable are not kinds. An operation deals with
//! those before it holds its operands against a signature (RFC-031,
//! RFC-036), so the functions here only ever see operands that have a kind.

use std::fmt;

use crate::error::EngineError;
use crate::types::{Operation, Value};

/// What a value shows at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Boolean,
    Number,
    String,
    Date,
    Array,
    Object,
}

impl Kind {
    /// The kind of a value, or `None` for `null`, an unknown and an
    /// untranslatable value, which have none.
    pub fn of(value: &Value) -> Option<Kind> {
        match value {
            Value::Null | Value::Unknown(_) | Value::Untranslatable { .. } => None,
            Value::Bool(_) => Some(Kind::Boolean),
            Value::Int(_) | Value::Decimal(_) => Some(Kind::Number),
            Value::String(_) => Some(Kind::String),
            Value::Date(_) => Some(Kind::Date),
            Value::Array(_) => Some(Kind::Array),
            Value::Object(_) => Some(Kind::Object),
        }
    }

    /// The name of the kind as a rulework declares it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Boolean => "boolean",
            Kind::Number => "number",
            Kind::String => "string",
            Kind::Date => "date",
            Kind::Array => "array",
            Kind::Object => "object",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What one signature asks of the operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operands {
    /// One operand or more, each of this kind: `ADD` of numbers.
    Each(Kind),
    /// Any number of operands, none included, each of this kind: the
    /// conditions of `AND` and `OR`.
    Any(Kind),
    /// Exactly one operand of this kind: `NOT`, the value of `ROUND`.
    One(Kind),
    /// Exactly two operands of these kinds, in order: an ordering.
    Two(Kind, Kind),
    /// Two operands of one kind, whatever it is: `EQUALS`.
    Alike,
    /// A subject of some kind and an array of that kind: `IN`.
    MemberOf,
    /// `boolean` conditions and outcomes of one kind: `IF`.
    Cases,
    /// Values of any kind: the items of `LIST`, the fields of `RECORD`.
    Values,
    /// An array to iterate over: `FOREACH`.
    Collection,
    /// A date and whole numbers: `DATE_ADD`.
    DateAndCounts,
    /// A date and a part or unit named in the rulework: `DATE_PART`,
    /// `START_OF`.
    DateAndPart,
    /// Two dates and a unit named in the rulework: `DATE_DIFF`.
    DatesAndUnit,
}

/// What one signature returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gives {
    /// A value of this kind.
    Kind(Kind),
    /// The kind of the outcomes (`IF`) or of the elements (`FOREACH` with
    /// `ONLY`): `T` in RFC-055's tables.
    Operand,
}

/// One row of the table: the operands an operation accepts and what it
/// returns for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    pub operands: Operands,
    pub gives: Gives,
}

const fn row(operands: Operands, kind: Kind) -> Signature {
    Signature {
        operands,
        gives: Gives::Kind(kind),
    }
}

const NUMBERS: &[Signature] = &[row(Operands::Each(Kind::Number), Kind::Number)];
const ADD: &[Signature] = &[
    row(Operands::Each(Kind::Number), Kind::Number),
    row(Operands::Each(Kind::String), Kind::String),
    row(Operands::Each(Kind::Array), Kind::Array),
];
const ROUNDING: &[Signature] = &[row(Operands::One(Kind::Number), Kind::Number)];
const EQUALITY: &[Signature] = &[row(Operands::Alike, Kind::Boolean)];
const ORDERING: &[Signature] = &[
    row(Operands::Two(Kind::Number, Kind::Number), Kind::Boolean),
    row(Operands::Two(Kind::Date, Kind::Date), Kind::Boolean),
];
const CONDITIONS: &[Signature] = &[row(Operands::Any(Kind::Boolean), Kind::Boolean)];
const NEGATION: &[Signature] = &[row(Operands::One(Kind::Boolean), Kind::Boolean)];
const CHOICE: &[Signature] = &[Signature {
    operands: Operands::Cases,
    gives: Gives::Operand,
}];
const MEMBERSHIP: &[Signature] = &[row(Operands::MemberOf, Kind::Boolean)];
const LIST: &[Signature] = &[row(Operands::Values, Kind::Array)];
const RECORD: &[Signature] = &[row(Operands::Values, Kind::Object)];
const ITERATION: &[Signature] = &[row(Operands::Collection, Kind::Array)];
const DATE: &[Signature] = &[row(Operands::Each(Kind::Number), Kind::Date)];
const DATE_ADD: &[Signature] = &[row(Operands::DateAndCounts, Kind::Date)];
const DATE_DIFF: &[Signature] = &[row(Operands::DatesAndUnit, Kind::Number)];
const AGE: &[Signature] = &[row(Operands::Two(Kind::Date, Kind::Date), Kind::Number)];
const DATE_PART: &[Signature] = &[row(Operands::DateAndPart, Kind::Number)];
const DAY_OF_WEEK: &[Signature] = &[row(Operands::One(Kind::Date), Kind::Number)];
const START_OF: &[Signature] = &[row(Operands::DateAndPart, Kind::Date)];

/// The signatures of `operation`, in the order RFC-055 lists them.
///
/// The operations schema versions up to v0.4.0 wrote (`NOT_EQUALS`, `IS_NULL`,
/// `NOT_NULL`, `NOT_IN`) have the signatures of the form they are read as.
pub fn signatures(operation: Operation) -> &'static [Signature] {
    match operation {
        Operation::Add => ADD,
        Operation::Subtract
        | Operation::Multiply
        | Operation::Divide
        | Operation::Min
        | Operation::Max => NUMBERS,
        Operation::Round | Operation::Ceil | Operation::Floor => ROUNDING,
        Operation::Equals | Operation::NotEquals | Operation::IsNull | Operation::NotNull => {
            EQUALITY
        }
        Operation::GreaterThan
        | Operation::LessThan
        | Operation::GreaterThanOrEqual
        | Operation::LessThanOrEqual => ORDERING,
        Operation::And | Operation::Or => CONDITIONS,
        Operation::Not => NEGATION,
        Operation::If => CHOICE,
        Operation::In | Operation::NotIn => MEMBERSHIP,
        Operation::List => LIST,
        Operation::Record => RECORD,
        Operation::ForEach => ITERATION,
        Operation::Date => DATE,
        Operation::DateAdd => DATE_ADD,
        Operation::DateDiff => DATE_DIFF,
        Operation::Age => AGE,
        Operation::DatePart => DATE_PART,
        Operation::DayOfWeek => DAY_OF_WEEK,
        Operation::StartOf => START_OF,
    }
}

/// Whether `operands` fit a signature whose operands are given by position.
/// A signature that names its operands another way (`IF`, `IN`, `FOREACH`)
/// never matches here; its operation checks them itself.
fn fits(expected: Operands, operands: &[Kind]) -> bool {
    match expected {
        Operands::Each(kind) => !operands.is_empty() && operands.iter().all(|k| *k == kind),
        Operands::Any(kind) => operands.iter().all(|k| *k == kind),
        Operands::One(kind) => operands == [kind],
        Operands::Two(first, second) => operands == [first, second],
        Operands::Alike => matches!(operands, [first, second] if first == second),
        Operands::MemberOf
        | Operands::Cases
        | Operands::Values
        | Operands::Collection
        | Operands::DateAndCounts
        | Operands::DateAndPart
        | Operands::DatesAndUnit => false,
    }
}

/// The signature of `operation` that `operands` match.
///
/// # Errors
///
/// [`EngineError::UnsupportedOperation`] when the operands match no
/// signature, naming the operation and the kinds it was given.
pub fn matching(operation: Operation, operands: &[Kind]) -> Result<Signature, EngineError> {
    signatures(operation)
        .iter()
        .copied()
        .find(|signature| fits(signature.operands, operands))
        .ok_or_else(|| unsupported(operation.name(), operands))
}

/// The error for operands that match no signature of `operation`:
/// `ADD is not defined for (date, string)`.
pub fn unsupported(operation: &str, operands: &[Kind]) -> EngineError {
    let kinds: Vec<&str> = operands.iter().map(|kind| kind.name()).collect();
    EngineError::UnsupportedOperation {
        operation: operation.to_string(),
        operands: kinds.join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_has_a_signature() {
        for operation in Operation::ALL_VARIANTS {
            assert!(
                !signatures(*operation).is_empty(),
                "{} has no signature",
                operation.name()
            );
        }
    }

    #[test]
    fn add_takes_numbers_strings_or_arrays_but_no_mix() {
        use Kind::{Array, Date, Number, String};
        assert_eq!(
            matching(Operation::Add, &[Number, Number])
                .ok()
                .map(|s| s.gives),
            Some(Gives::Kind(Number))
        );
        assert_eq!(
            matching(Operation::Add, &[String, String])
                .ok()
                .map(|s| s.gives),
            Some(Gives::Kind(String))
        );
        assert_eq!(
            matching(Operation::Add, &[Array]).ok().map(|s| s.gives),
            Some(Gives::Kind(Array))
        );
        for operands in [&[Date, String][..], &[Number, String], &[Date, Date], &[]] {
            assert!(matching(Operation::Add, operands).is_err(), "{operands:?}");
        }
    }

    #[test]
    fn the_error_names_the_operation_and_the_operand_kinds() {
        let err = matching(Operation::Add, &[Kind::Date, Kind::String]).unwrap_err();
        assert_eq!(err.to_string(), "ADD is not defined for (date, string)");
    }

    #[test]
    fn an_ordering_takes_two_numbers_or_two_dates() {
        use Kind::{Boolean, Date, Number, String};
        assert!(matching(Operation::LessThan, &[Number, Number]).is_ok());
        assert!(matching(Operation::LessThan, &[Date, Date]).is_ok());
        for operands in [
            &[Number, Date][..],
            &[String, String],
            &[Boolean, Boolean],
            &[Number],
        ] {
            assert!(
                matching(Operation::LessThan, operands).is_err(),
                "{operands:?}"
            );
        }
    }

    #[test]
    fn equals_takes_two_operands_of_one_kind() {
        use Kind::{Boolean, Number, Object, String};
        for kind in [Boolean, Number, String, Object] {
            assert!(matching(Operation::Equals, &[kind, kind]).is_ok(), "{kind}");
        }
        assert!(matching(Operation::Equals, &[Number, String]).is_err());
        assert!(matching(Operation::Equals, &[Boolean, Number]).is_err());
    }

    #[test]
    fn logic_takes_booleans_only() {
        use Kind::{Boolean, Number, String};
        assert!(matching(Operation::And, &[]).is_ok());
        assert!(matching(Operation::Or, &[Boolean, Boolean]).is_ok());
        assert!(matching(Operation::And, &[Boolean, Number]).is_err());
        assert!(matching(Operation::Not, &[Boolean]).is_ok());
        assert!(matching(Operation::Not, &[String]).is_err());
        assert!(matching(Operation::Not, &[]).is_err());
    }

    #[test]
    fn arithmetic_other_than_add_takes_numbers_only() {
        use Kind::{Number, String};
        for operation in [
            Operation::Subtract,
            Operation::Multiply,
            Operation::Divide,
            Operation::Min,
            Operation::Max,
        ] {
            assert!(matching(operation, &[Number, Number, Number]).is_ok());
            assert!(matching(operation, &[String, String]).is_err());
            assert!(matching(operation, &[]).is_err());
        }
        assert!(matching(Operation::Round, &[Number]).is_ok());
        assert!(matching(Operation::Round, &[String]).is_err());
    }

    #[test]
    fn a_value_without_a_kind_is_null_unknown_or_untranslatable() {
        assert_eq!(Kind::of(&Value::Null), None);
        assert_eq!(Kind::of(&Value::Unknown(Vec::new())), None);
        assert_eq!(Kind::of(&Value::Int(1)), Some(Kind::Number));
        assert_eq!(
            Kind::of(&Value::Date("2025-01-01".parse().expect("a date"))),
            Some(Kind::Date)
        );
    }
}
