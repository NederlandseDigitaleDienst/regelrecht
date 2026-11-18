//! A [`Value`] as JSON that is kept and read back, without losing a number,
//! and the rule by which two such values are the same.
//!
//! A `Value` keeps an integer as an `Int` and every other number as an exact
//! `Decimal`. Plain JSON numbers cannot carry that: the ordinary
//! serialization of a `Value` writes a decimal as a float, so `1/3` loses its
//! 28 digits, and reads a whole float back as an integer, so a decimal `5.0`
//! returns as `5`. That serialization stays as it is: it is what the API,
//! the WASM build and the frontend read. What is kept to be read back, such
//! as a chronicle, goes through this module instead: [`to_stored`] writes a
//! decimal as `{"__decimal": "<digits>"}`, with the digits and the scale the
//! `Decimal` holds, and [`from_stored`] reads that back as the same decimal.
//! Every other value is the JSON the ordinary conversion writes.
//!
//! [`equal_as_written`] is the rule the engine's `EQUALS` applies to two
//! lists or two records; [`same`] applies it to two stored values.

use std::collections::BTreeMap;
use std::fmt;

use rust_decimal::Decimal;
use serde_json::Value as J;

use crate::Value;

/// The key of the object a decimal is stored as.
pub const DECIMAL_KEY: &str = "__decimal";

/// Why a value cannot be stored, read back or compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredError {
    /// A record holds [`DECIMAL_KEY`] itself: read back, it would be a
    /// decimal it never was.
    ReservedKey(String),
    /// An object holds [`DECIMAL_KEY`] but is not the object [`to_stored`]
    /// writes for a decimal.
    NotADecimal(String),
    /// Two values hold an integer and a decimal of the same value at one
    /// place ([`equal_as_written`]).
    Unlike(WrittenUnlike),
}

impl fmt::Display for StoredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoredError::ReservedKey(value) => write!(
                f,
                "{value} holds the key '{DECIMAL_KEY}', which is kept for a stored decimal"
            ),
            StoredError::NotADecimal(json) => write!(f, "{json} is no stored decimal"),
            StoredError::Unlike(unlike) => unlike.fmt(f),
        }
    }
}

impl std::error::Error for StoredError {}

impl From<WrittenUnlike> for StoredError {
    fn from(unlike: WrittenUnlike) -> Self {
        StoredError::Unlike(unlike)
    }
}

/// An integer against a decimal of the same value (`12` against `12.0`),
/// found by [`equal_as_written`]: not an answer either way, because whether
/// the two are alike is a question about where the values came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenUnlike {
    /// The number on the left, as written.
    pub left: String,
    /// The number on the right, as written.
    pub right: String,
    /// Where inside the two values, such as `.data[0].dagen`; empty for two
    /// bare numbers.
    pub path: String,
}

impl fmt::Display for WrittenUnlike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "numbers written alike expected, got {} against {} at ${}",
            self.left, self.right, self.path
        )
    }
}

/// `value` as JSON to keep and read back with [`from_stored`]. A record that
/// holds [`DECIMAL_KEY`] itself is an error.
pub fn to_stored(value: &Value) -> Result<J, StoredError> {
    Ok(match value {
        Value::Decimal(d) => {
            let mut marker = serde_json::Map::new();
            marker.insert(DECIMAL_KEY.to_string(), J::String(d.to_string()));
            J::Object(marker)
        }
        Value::Object(map) if map.contains_key(DECIMAL_KEY) => {
            return Err(StoredError::ReservedKey(value.to_string()));
        }
        Value::Array(items) => J::Array(items.iter().map(to_stored).collect::<Result<_, _>>()?),
        Value::Object(map) => J::Object(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), to_stored(v)?)))
                .collect::<Result<_, StoredError>>()?,
        ),
        other => J::from(other),
    })
}

/// The value `json` holds: the inverse of [`to_stored`]. An object that
/// holds [`DECIMAL_KEY`] but is not the object [`to_stored`] writes is an
/// error.
pub fn from_stored(json: &J) -> Result<Value, StoredError> {
    restore(Value::from(json))
}

/// Whether `json` holds [`DECIMAL_KEY`] anywhere. A value supplied from
/// outside that holds it is refused before it is stored: read back, it would
/// turn into a decimal it never was.
pub fn holds_reserved_key(json: &J) -> bool {
    match json {
        J::Object(map) => map.contains_key(DECIMAL_KEY) || map.values().any(holds_reserved_key),
        J::Array(items) => items.iter().any(holds_reserved_key),
        _ => false,
    }
}

/// Whether `a` and `b` are equal by the rule of the engine's `EQUALS` on two
/// lists or two records, at any depth, a bare number included: two decimals
/// are equal by their value, whatever their scale (`5.0` is `5.00`), and an
/// integer against a decimal of the same value is an error. The whole
/// structure is walked, so the error does not depend on what differs first.
pub fn equal_as_written(a: &Value, b: &Value) -> Result<bool, WrittenUnlike> {
    equal_at(a, b, "")
}

fn equal_at(a: &Value, b: &Value, path: &str) -> Result<bool, WrittenUnlike> {
    match (a, b) {
        (Value::Object(left), Value::Object(right)) => {
            let mut equal = left.len() == right.len();
            for (key, l) in left {
                match right.get(key) {
                    Some(r) => equal &= equal_at(l, r, &format!("{path}.{key}"))?,
                    None => equal = false,
                }
            }
            Ok(equal)
        }
        (Value::Array(left), Value::Array(right)) => {
            let mut equal = left.len() == right.len();
            for (index, (l, r)) in left.iter().zip(right).enumerate() {
                equal &= equal_at(l, r, &format!("{path}[{index}]"))?;
            }
            Ok(equal)
        }
        (Value::Int(i), Value::Decimal(d)) | (Value::Decimal(d), Value::Int(i))
            if Decimal::from(*i) == *d =>
        {
            Err(WrittenUnlike {
                left: a.to_string(),
                right: b.to_string(),
                path: path.to_string(),
            })
        }
        _ => Ok(a == b),
    }
}

/// Whether the stored values `a` and `b` are the same, by
/// [`equal_as_written`].
pub fn same(a: &J, b: &J) -> Result<bool, StoredError> {
    Ok(equal_as_written(&from_stored(a)?, &from_stored(b)?)?)
}

/// Whether `a` and `b` are the same, as [`same`], where a value may be
/// absent: two absent values are the same, one absent value is not.
pub fn same_held(a: Option<&J>, b: Option<&J>) -> Result<bool, StoredError> {
    match (a, b) {
        (Some(a), Some(b)) => same(a, b),
        (a, b) => Ok(a.is_none() && b.is_none()),
    }
}

/// Turn the decimal objects in `value` back into decimals.
fn restore(value: Value) -> Result<Value, StoredError> {
    Ok(match value {
        Value::Object(map) if map.contains_key(DECIMAL_KEY) => Value::Decimal(decimal(&map)?),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| Ok((k, restore(v)?)))
                .collect::<Result<_, StoredError>>()?,
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(restore).collect::<Result<_, _>>()?)
        }
        other => other,
    })
}

/// The decimal `map` stands for: the object [`to_stored`] writes for one.
fn decimal(map: &BTreeMap<String, Value>) -> Result<Decimal, StoredError> {
    let digits = map
        .get(DECIMAL_KEY)
        .and_then(Value::as_str)
        .filter(|_| map.len() == 1);
    // Only the digits `to_stored` writes: what parses but reads otherwise,
    // such as `1_0` or `+1`, is no decimal it wrote.
    digits
        .and_then(|d| d.parse::<Decimal>().ok().filter(|p| p.to_string() == d))
        .ok_or_else(|| StoredError::NotADecimal(J::from(&Value::Object(map.clone())).to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dec(s: &str) -> Value {
        Value::Decimal(s.parse().unwrap())
    }

    fn round_trip(value: &Value) -> Value {
        let text = serde_json::to_string(&to_stored(value).unwrap()).unwrap();
        from_stored(&serde_json::from_str(&text).unwrap()).unwrap()
    }

    #[test]
    fn a_decimal_keeps_its_digits_and_scale() {
        for s in ["5.0", "5", "0.3333333333333333333333333333", "-12.50"] {
            let value = dec(s);
            let back = round_trip(&value);
            assert!(matches!(back, Value::Decimal(_)), "{s}");
            assert_eq!(back.to_string(), s);
        }
    }

    #[test]
    fn the_ordinary_serialization_does_not_keep_a_decimal() {
        // Why this module exists, and what it leaves alone: the JSON the
        // API and the frontend read still holds a float.
        let five = dec("5.0");
        assert_eq!(serde_json::to_string(&five).unwrap(), "5.0");
        assert!(matches!(Value::from(&J::from(&five)), Value::Int(5)));
    }

    #[test]
    fn an_integer_stays_an_integer() {
        assert!(matches!(round_trip(&Value::Int(5)), Value::Int(5)));
    }

    #[test]
    fn decimals_inside_lists_and_records_come_back() {
        let value = Value::Array(vec![Value::Object(
            [
                ("aandeel".to_string(), dec("0.5")),
                ("n".into(), Value::Int(2)),
            ]
            .into(),
        )]);
        assert_eq!(
            to_stored(&value).unwrap(),
            json!([{"aandeel": {"__decimal": "0.5"}, "n": 2}])
        );
        let Value::Array(items) = round_trip(&value) else {
            panic!("not a list")
        };
        let Value::Object(map) = &items[0] else {
            panic!("not a record")
        };
        assert_eq!(map["aandeel"].to_string(), "0.5");
        assert!(matches!(map["aandeel"], Value::Decimal(_)));
        assert!(matches!(map["n"], Value::Int(2)));
    }

    #[test]
    fn a_damaged_decimal_is_an_error() {
        for json in [
            json!({"__decimal": "geen getal"}),
            json!({"__decimal": "1.5", "ander": 1}),
            json!([{"__decimal": 1.5}]),
            json!({"__decimal": "1_0"}),
            json!({"__decimal": "+1"}),
        ] {
            let err = from_stored(&json).unwrap_err();
            assert!(matches!(err, StoredError::NotADecimal(_)), "{json}: {err}");
            assert!(err.to_string().contains("no stored decimal"), "{err}");
        }
    }

    #[test]
    fn a_decimal_is_the_same_whatever_its_scale() {
        let a = to_stored(&dec("5.0")).unwrap();
        assert!(same(&a, &to_stored(&dec("5.00")).unwrap()).unwrap());
        assert!(!same(&a, &to_stored(&dec("5.01")).unwrap()).unwrap());
        assert!(!same(&a, &json!(6)).unwrap());
        assert!(same_held(Some(&a), Some(&to_stored(&dec("5.00")).unwrap())).unwrap());
        assert!(same_held(None, None).unwrap());
        assert!(!same_held(Some(&a), None).unwrap());
        assert!(!same_held(None, Some(&a)).unwrap());
    }

    #[test]
    fn an_integer_against_the_same_decimal_is_an_error() {
        // Not a difference and not a match, bare or inside a record or a
        // list: whether 5 and 5.0 are alike is not answered silently.
        let five = to_stored(&dec("5.0")).unwrap();
        for (a, b, path) in [
            (five.clone(), json!(5), "$"),
            (json!({"n": five.clone()}), json!({"n": 5}), "$.n"),
            (json!([5]), json!([five.clone()]), "$[0]"),
        ] {
            let err = same(&a, &b).unwrap_err();
            assert!(matches!(err, StoredError::Unlike(_)), "{err}");
            assert!(err.to_string().ends_with(&format!(" at {path}")), "{err}");
            assert!(same_held(Some(&b), Some(&a)).is_err());
        }
    }

    #[test]
    fn equal_as_written_walks_the_whole_structure() {
        let record = |v: Value| Value::Object(BTreeMap::from([("n".to_string(), v)]));
        assert!(equal_as_written(&dec("5.0"), &dec("5.00")).unwrap());
        assert!(!equal_as_written(&dec("5.0"), &Value::Int(6)).unwrap());
        assert!(equal_as_written(&record(Value::Int(5)), &record(Value::Int(5))).unwrap());
        assert!(!equal_as_written(&record(Value::Int(5)), &Value::Array(vec![])).unwrap());
        // A difference earlier in the walk does not hide the mismatch later.
        let pair = |a: Value, b: Value| Value::Array(vec![a, b]);
        let err = equal_as_written(
            &pair(Value::Int(1), Value::Int(5)),
            &pair(Value::Int(2), dec("5")),
        )
        .unwrap_err();
        assert_eq!(
            err,
            WrittenUnlike {
                left: "5".into(),
                right: "5".into(),
                path: "[1]".into()
            }
        );
        let a = Value::Object(BTreeMap::from([
            ("a".to_string(), Value::Int(1)),
            ("b".to_string(), Value::Int(5)),
        ]));
        let b = Value::Object(BTreeMap::from([
            ("a".to_string(), Value::Int(2)),
            ("b".to_string(), dec("5.0")),
        ]));
        assert_eq!(equal_as_written(&a, &b).unwrap_err().path, ".b");
        // A record with a key the other lacks is unequal, not an error.
        let c = Value::Object(BTreeMap::from([("z".to_string(), Value::Int(5))]));
        assert!(!equal_as_written(&record(Value::Int(5)), &c).unwrap());
        // One differing member makes the whole unequal, whatever follows it;
        // a list of another length is unequal.
        let two = |a: i64, b: i64| {
            Value::Object(BTreeMap::from([
                ("a".to_string(), Value::Int(a)),
                ("b".to_string(), Value::Int(b)),
            ]))
        };
        assert!(!equal_as_written(&two(1, 2), &two(3, 2)).unwrap());
        let list = |items: &[i64]| Value::Array(items.iter().map(|i| Value::Int(*i)).collect());
        assert!(!equal_as_written(&list(&[1, 2]), &list(&[3, 2])).unwrap());
        assert!(!equal_as_written(&list(&[1]), &list(&[1, 2])).unwrap());
        assert!(equal_as_written(&list(&[1, 2]), &list(&[1, 2])).unwrap());
    }

    #[test]
    fn other_values_are_the_json_the_ordinary_conversion_writes() {
        let unknown = Value::from(&json!({"__unknown": true, "missing": [
            {"law": "wet", "name": "x", "kind": "no_data"}
        ]}));
        let untranslatable = Value::Untranslatable {
            article: "1".into(),
            construct: "iets".into(),
        };
        for value in [
            Value::Null,
            Value::Bool(true),
            "tekst".into(),
            unknown,
            untranslatable,
        ] {
            assert_eq!(to_stored(&value).unwrap(), J::from(&value));
            assert_eq!(round_trip(&value), value);
        }
    }

    #[test]
    fn a_record_that_holds_the_decimal_key_is_not_stored() {
        let record = Value::from(&json!({"x": {"__decimal": "1.5"}}));
        let err = to_stored(&record).unwrap_err();
        assert!(matches!(err, StoredError::ReservedKey(_)), "{err}");
        assert!(to_stored(&Value::Array(vec![record])).is_err());
    }

    #[test]
    fn the_decimal_key_is_found_anywhere() {
        assert!(holds_reserved_key(&json!({"__decimal": "1"})));
        assert!(holds_reserved_key(&json!([{"a": {"__decimal": 1}}])));
        assert!(!holds_reserved_key(&json!({"a": [1, "__decimal"]})));
    }
}
