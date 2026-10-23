//! Core law-model value types shared across the workspace.
//!
//! `Value`, `Operation` and `ParameterType` describe the *format* of a law
//! document (literals, operation tags, parameter types). They are the canonical
//! representation re-exported by the engine at `regelrecht_engine::types`.
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;

/// Represents any value in the engine (similar to Python's Any)
///
/// Non-integer numbers are represented as an exact [`rust_decimal::Decimal`]
/// rather than `f64`, so intermediate calculations carry full precision and
/// rounding only ever happens at an explicit `ROUND`/`CEIL`/`FLOOR`
/// operation (RFC-024). `Decimal` has no NaN: non-finite `f64` reaching the
/// engine boundary (NaN/∞) maps to [`Value::Null`] (the missing/invalid value).
///
/// The `Untranslatable` variant (RFC-012 Layer 3) represents a value that
/// originates from an article with untranslatable constructs. It propagates
/// through operations: any operation involving an Untranslatable input produces
/// an Untranslatable output.
///
/// Two kinds of "nothing" are kept apart (RFC-036):
///
/// - [`Value::Null`] is **absence**: the register is authoritative and says
///   there is none (no partner, no rent, no permit). It is a value a law can
///   test for (`EQUALS … null`), but not calculate with or decide on.
/// - [`Value::Unknown`] is **a fact nobody has (yet)**: no data source held
///   the input, or an optional parameter was not passed. It cannot be written
///   in a law; only resolution produces it, and it propagates through every
///   operation carrying the names of the missing facts, so a decision process
///   can ask for exactly those (Awb art. 4:5).
#[derive(Debug, Clone, Default)]
pub enum Value {
    /// Null/None value: an absence the data is authoritative about (RFC-036).
    #[default]
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Exact decimal value (non-integer numbers)
    Decimal(Decimal),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<Value>),
    /// Object/Map of values
    Object(BTreeMap<String, Value>),
    /// Untranslatable taint marker (RFC-012 Layer 3).
    /// Carries origin info: (article_number, construct description).
    Untranslatable {
        /// Article number where the untranslatable originated
        article: String,
        /// The construct that could not be translated
        construct: String,
    },
    /// Unknown: the facts listed are missing (RFC-036). Never empty.
    ///
    /// Built with [`Value::unknown`] and widened with [`Value::merge_unknown`];
    /// the list is ordered by first appearance and free of duplicates.
    Unknown(Vec<MissingFact>),
}

/// One fact the engine needed and nobody supplied (RFC-036).
///
/// This is what an Unknown outcome is made of: not "null", but the name of
/// the input or parameter that has no value, and the law that declares it.
/// A decision process turns this list into the request for completion of an
/// incomplete application (Awb art. 4:5).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MissingFact {
    /// `$id` of the law whose input or parameter is missing.
    pub law: String,
    /// The input or parameter name.
    pub name: String,
    /// Why it is missing.
    pub kind: MissingKind,
}

/// Why a fact is missing (RFC-036).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingKind {
    /// A `source: {}` input no data source has a value for.
    NoData,
    /// A `required: false` parameter the caller did not pass.
    NotPassed,
}

/// Sentinel key used to identify serialized Untranslatable values.
const UNTRANSLATABLE_KEY: &str = "__untranslatable";

/// Sentinel key used to identify serialized Unknown values (RFC-036).
const UNKNOWN_KEY: &str = "__unknown";

/// Key under which an Unknown carries its missing facts.
const MISSING_KEY: &str = "missing";

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Value::Null => serializer.serialize_none(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::Decimal(d) => {
                // Infallible within Decimal's value range (max ~7.9e28 << f64::MAX);
                // surface the impossible case as an error rather than a wrong 0.0.
                let f = d.to_f64().ok_or_else(|| {
                    serde::ser::Error::custom(format!("Decimal {d} is not representable as f64"))
                })?;
                serializer.serialize_f64(f)
            }
            Value::String(s) => serializer.serialize_str(s),
            Value::Array(arr) => arr.serialize(serializer),
            Value::Object(map) => map.serialize(serializer),
            Value::Untranslatable { article, construct } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry(UNTRANSLATABLE_KEY, &true)?;
                map.serialize_entry("article", article)?;
                map.serialize_entry("construct", construct)?;
                map.end()
            }
            Value::Unknown(missing) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry(UNKNOWN_KEY, &true)?;
                map.serialize_entry(MISSING_KEY, missing)?;
                map.end()
            }
        }
    }
}

/// Read the `missing` list of a serialized Unknown back into facts.
///
/// Every entry has to be a complete `{law, name, kind}` object; a list that
/// is empty or malformed is not an Unknown at all, and the caller reports it
/// as such rather than inventing a provenance.
fn missing_facts_from_value(value: &Value) -> Option<Vec<MissingFact>> {
    let Value::Array(items) = value else {
        return None;
    };
    let mut facts = Vec::with_capacity(items.len());
    for item in items {
        let Value::Object(fields) = item else {
            return None;
        };
        let law = fields.get("law")?.as_str()?.to_string();
        let name = fields.get("name")?.as_str()?.to_string();
        let kind = match fields.get("kind")?.as_str()? {
            "no_data" => MissingKind::NoData,
            "not_passed" => MissingKind::NotPassed,
            _ => return None,
        };
        facts.push(MissingFact { law, name, kind });
    }
    if facts.is_empty() {
        return None;
    }
    Some(dedup_facts(facts))
}

/// Keep the first occurrence of every fact, in order of first appearance.
fn dedup_facts(facts: Vec<MissingFact>) -> Vec<MissingFact> {
    let mut seen: Vec<MissingFact> = Vec::with_capacity(facts.len());
    for fact in facts {
        if !seen.contains(&fact) {
            seen.push(fact);
        }
    }
    seen
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Value, E> {
        Ok(Value::Int(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Value, E> {
        i64::try_from(v)
            .map(Value::Int)
            .map_err(|_| E::custom(format!("u64 value {v} overflows i64")))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Value, E> {
        Ok(f64_to_value(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_none<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Value, A::Error> {
        let mut arr = Vec::new();
        while let Some(elem) = seq.next_element()? {
            arr.push(elem);
        }
        Ok(Value::Array(arr))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        let mut obj = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, Value>()? {
            obj.insert(key, value);
        }
        // Check if this is a serialized Untranslatable
        if obj.get(UNTRANSLATABLE_KEY) == Some(&Value::Bool(true)) {
            let article = match obj.get("article") {
                Some(Value::String(s)) => s.clone(),
                _ => return Err(de::Error::missing_field("article")),
            };
            let construct = match obj.get("construct") {
                Some(Value::String(s)) => s.clone(),
                _ => return Err(de::Error::missing_field("construct")),
            };
            return Ok(Value::Untranslatable { article, construct });
        }
        // Check if this is a serialized Unknown (RFC-036). A marker without a
        // usable `missing` list is a plain object, exactly as in
        // `From<serde_json::Value>`: the two readers must agree, and `From`
        // cannot fail. Only a well-formed sentinel becomes an Unknown.
        if obj.get(UNKNOWN_KEY) == Some(&Value::Bool(true)) {
            if let Some(missing) = obj.get(MISSING_KEY).and_then(missing_facts_from_value) {
                return Ok(Value::Unknown(missing));
            }
        }
        Ok(Value::Object(obj))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Decimal(a), Value::Decimal(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            // Two Untranslatable values are equal (like NaN == NaN in this domain)
            (Value::Untranslatable { .. }, Value::Untranslatable { .. }) => true,
            // Two Unknowns are equal whatever facts they miss (RFC-036): both
            // say "not decidable", and the provenance is diagnostics, not identity.
            // This rule exists for the test harnesses (`is unknown` compares an
            // output against an Unknown) and never decides a law: every engine
            // operation checks `contains_unknown` on its operands first and
            // propagates, so no comparison in a law reaches this arm with an
            // Unknown at any depth.
            (Value::Unknown(_), Value::Unknown(_)) => true,
            _ => false,
        }
    }
}

impl Value {
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Check if value is unknown (RFC-036): a fact nobody has yet.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Value::Unknown(_))
    }

    /// An Unknown for one missing fact (RFC-036).
    pub fn unknown(law: impl Into<String>, name: impl Into<String>, kind: MissingKind) -> Value {
        Value::Unknown(vec![MissingFact {
            law: law.into(),
            name: name.into(),
            kind,
        }])
    }

    /// The facts an Unknown misses; empty for every other variant.
    pub fn missing_facts(&self) -> &[MissingFact] {
        match self {
            Value::Unknown(missing) => missing,
            _ => &[],
        }
    }

    /// Whether an Unknown sits anywhere inside this value: the value itself,
    /// an element of an array, a field of an object, at any depth (RFC-036).
    ///
    /// A `LIST` or a `FOREACH` without `combine` may hold Unknown elements,
    /// and a structural comparison of such a container has to propagate them
    /// instead of comparing them as equal.
    pub fn contains_unknown(&self) -> bool {
        match self {
            Value::Unknown(_) => true,
            Value::Array(items) => items.iter().any(Value::contains_unknown),
            Value::Object(fields) => fields.values().any(Value::contains_unknown),
            _ => false,
        }
    }

    /// Collect the missing facts of every Unknown nested anywhere in this
    /// value, in order of appearance, into `into` (see [`Self::contains_unknown`]).
    fn collect_missing_facts(&self, into: &mut Vec<MissingFact>) {
        match self {
            Value::Unknown(missing) => into.extend(missing.iter().cloned()),
            Value::Array(items) => items.iter().for_each(|v| v.collect_missing_facts(into)),
            Value::Object(fields) => fields.values().for_each(|v| v.collect_missing_facts(into)),
            _ => {}
        }
    }

    /// The union of the missing facts of every Unknown among `values`, as one
    /// Unknown; `None` when none of them is Unknown (RFC-036).
    ///
    /// This is how an operation propagates: `ADD($huur, $partner_inkomen)`
    /// with both unknown misses both facts, in the order the operands name
    /// them, each fact once.
    pub fn merge_unknown<'a>(values: impl IntoIterator<Item = &'a Value>) -> Option<Value> {
        let facts: Vec<MissingFact> = values
            .into_iter()
            .flat_map(|v| v.missing_facts().iter().cloned())
            .collect();
        if facts.is_empty() {
            return None;
        }
        Some(Value::Unknown(dedup_facts(facts)))
    }

    /// Like [`Self::merge_unknown`], but an Unknown counts wherever it sits
    /// inside a value: `[unknown] == [1]` cannot be decided any more than
    /// `unknown == 1` can (RFC-036). Used by the structural operations
    /// (`EQUALS`, `NOT_EQUALS`, `IN`, `NOT_IN`), which compare containers
    /// element by element.
    pub fn merge_unknown_deep<'a>(values: impl IntoIterator<Item = &'a Value>) -> Option<Value> {
        let mut facts = Vec::new();
        for value in values {
            value.collect_missing_facts(&mut facts);
        }
        if facts.is_empty() {
            return None;
        }
        Some(Value::Unknown(dedup_facts(facts)))
    }

    /// Try to get value as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get value as i64
    ///
    /// For floats, truncates toward zero (like Python's `int()`).
    /// For example: `1.9` becomes `1`, `-1.9` becomes `-1`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Decimal(d) => d.trunc().to_i64(),
            _ => None,
        }
    }

    /// Try to get value as f64
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Decimal(d) => d.to_f64(),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get value as an exact [`Decimal`] (integers widen to decimal).
    pub fn as_decimal(&self) -> Option<Decimal> {
        match self {
            Value::Decimal(d) => Some(*d),
            Value::Int(i) => Some(Decimal::from(*i)),
            _ => None,
        }
    }

    /// Try to get value as string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get value as array reference
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get value as object reference
    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Check if value is untranslatable (RFC-012 taint).
    pub fn is_untranslatable(&self) -> bool {
        matches!(self, Value::Untranslatable { .. })
    }

    /// Get the type name as a static string (for error messages).
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Decimal(_) => "decimal",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Untranslatable { .. } => "untranslatable",
            Value::Unknown(_) => "unknown",
        }
    }

    /// Convert value to boolean (Python-style truthiness)
    ///
    /// `Null` and `Unknown` read as `false` here for display code only
    /// (RFC-036): no boolean context in the engine may reach this with either.
    /// The logical operations reject `Null` (`AbsentOperand`) and propagate
    /// `Unknown` before they ever ask for a truth value.
    pub fn to_bool(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Decimal(d) => !d.is_zero(),
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            Value::Untranslatable { .. } => false,
            Value::Unknown(_) => false,
        }
    }
}

/// Render the names of missing facts as `law.name, law.name`.
fn format_missing(missing: &[MissingFact]) -> String {
    missing
        .iter()
        .map(|m| format!("{}.{}", m.law, m.name))
        .collect::<Vec<_>>()
        .join(", ")
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Int(i as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        f64_to_value(f)
    }
}

impl From<Decimal> for Value {
    fn from(d: Decimal) -> Self {
        Value::Decimal(d)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Value::Null,
        }
    }
}

/// Convert an `f64` to a [`Value`], coercing whole numbers to [`Value::Int`] and
/// other finite values to an exact [`Value::Decimal`]. Non-finite values
/// (NaN/∞), which have no `Decimal` representation, map to [`Value::Null`] (the
/// missing/invalid value).
pub fn f64_to_value(v: f64) -> Value {
    // `i64::MIN as f64` is exact, but `i64::MAX as f64` rounds UP to i64::MAX+1,
    // so the upper bound is strict (`<`) — otherwise a whole f64 equal to
    // 9223372036854775808.0 would saturate to i64::MAX on `as i64`. Out-of-range
    // whole numbers fall through to the exact Decimal branch instead.
    const I64_MAX_F64: f64 = i64::MAX as f64;
    if v.fract() == 0.0 && v >= i64::MIN as f64 && v < I64_MAX_F64 {
        Value::Int(v as i64)
    } else {
        match Decimal::from_f64(v) {
            Some(d) => Value::Decimal(d),
            None => Value::Null,
        }
    }
}

/// Coerce whole-number JSON floats to Int for consistency.
fn json_number_to_value(n: &serde_json::Number) -> Value {
    if let Some(i) = n.as_i64() {
        Value::Int(i)
    } else if let Some(f) = n.as_f64() {
        f64_to_value(f)
    } else {
        Value::Null
    }
}

/// Convert an exact [`Decimal`] to a JSON number, falling back to `null` only if
/// the value somehow has no `f64` representation.
fn decimal_to_json(d: Decimal) -> serde_json::Value {
    d.to_f64()
        .and_then(serde_json::Number::from_f64)
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null)
}

/// If the JSON object carries the Untranslatable marker, build the variant.
fn json_object_as_untranslatable(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Option<Value> {
    if obj.get(UNTRANSLATABLE_KEY) != Some(&serde_json::Value::Bool(true)) {
        return None;
    }
    let article = obj
        .get("article")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let construct = obj
        .get("construct")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(Value::Untranslatable { article, construct })
}

/// If the JSON object carries the Unknown marker (RFC-036), build the variant.
///
/// A marker without a usable `missing` list is not an Unknown: it stays an
/// ordinary object, so a malformed sentinel cannot smuggle in an Unknown
/// without provenance.
fn json_object_as_unknown(obj: &serde_json::Map<String, serde_json::Value>) -> Option<Value> {
    if obj.get(UNKNOWN_KEY) != Some(&serde_json::Value::Bool(true)) {
        return None;
    }
    let missing = Value::from(obj.get(MISSING_KEY)?);
    missing_facts_from_value(&missing).map(Value::Unknown)
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => json_number_to_value(&n),
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(arr) => {
                Value::Array(arr.into_iter().map(Value::from).collect())
            }
            serde_json::Value::Object(obj) => {
                if let Some(u) = json_object_as_untranslatable(&obj) {
                    return u;
                }
                if let Some(u) = json_object_as_unknown(&obj) {
                    return u;
                }
                let map: BTreeMap<String, Value> =
                    obj.into_iter().map(|(k, v)| (k, Value::from(v))).collect();
                Value::Object(map)
            }
        }
    }
}

impl From<&serde_json::Value> for Value {
    fn from(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => json_number_to_value(n),
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => Value::Array(arr.iter().map(Value::from).collect()),
            serde_json::Value::Object(obj) => {
                if let Some(u) = json_object_as_untranslatable(obj) {
                    return u;
                }
                if let Some(u) = json_object_as_unknown(obj) {
                    return u;
                }
                let map: BTreeMap<String, Value> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::from(v)))
                    .collect();
                Value::Object(map)
            }
        }
    }
}

/// The JSON shape of an Unknown: `{"__unknown": true, "missing": [...]}`.
fn unknown_to_json(missing: &[MissingFact]) -> serde_json::Value {
    serde_json::json!({
        UNKNOWN_KEY: true,
        MISSING_KEY: missing,
    })
}

impl From<&Value> for serde_json::Value {
    fn from(v: &Value) -> Self {
        match v {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(i) => serde_json::json!(*i),
            Value::Decimal(d) => decimal_to_json(*d),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(arr) => serde_json::Value::Array(arr.iter().map(Into::into).collect()),
            Value::Object(map) => {
                serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), v.into())).collect())
            }
            Value::Untranslatable { article, construct } => {
                serde_json::json!({
                    UNTRANSLATABLE_KEY: true,
                    "article": article,
                    "construct": construct,
                })
            }
            Value::Unknown(missing) => unknown_to_json(missing),
        }
    }
}

impl From<Value> for serde_json::Value {
    fn from(v: Value) -> Self {
        match v {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(b),
            Value::Int(i) => serde_json::json!(i),
            Value::Decimal(d) => decimal_to_json(d),
            Value::String(s) => serde_json::Value::String(s),
            Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Into::into).collect())
            }
            Value::Object(map) => {
                serde_json::Value::Object(map.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
            Value::Untranslatable { article, construct } => {
                serde_json::json!({
                    UNTRANSLATABLE_KEY: true,
                    "article": article,
                    "construct": construct,
                })
            }
            Value::Unknown(missing) => unknown_to_json(&missing),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i),
            Value::Decimal(d) => write!(f, "{}", d),
            Value::String(s) => write!(f, "{}", s),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Object(obj) => {
                write!(f, "{{")?;
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Untranslatable { article, construct } => {
                write!(f, "UNTRANSLATABLE(art. {}: {})", article, construct)
            }
            Value::Unknown(missing) => write!(f, "UNKNOWN({})", format_missing(missing)),
        }
    }
}

/// Operation types supported by the engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Operation {
    // Comparison operations (5)
    Equals,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,

    // Arithmetic operations (4)
    Add,
    Subtract,
    Multiply,
    Divide,

    // Aggregate operations (2)
    Max,
    Min,

    // Rounding operations (3) — unary, with a `precision` (RFC-024)
    Round,
    Ceil,
    Floor,

    // Logical operations (3)
    And,
    Or,
    Not,

    // Conditional operations (1)
    /// IF with cases/default syntax (formerly SWITCH)
    #[serde(alias = "SWITCH")]
    If,

    // Collection operations (3)
    In,
    List,
    /// Iterate over a collection (RFC-016). Nested-only, like If and List.
    ForEach,

    // Date operations (7)
    Age,
    DateAdd,
    Date,
    DayOfWeek,
    DateDiff,
    DatePart,
    StartOf,

    // Engine-only compat aliases — accepted during deserialization but NOT in the
    // v0.5.0 schema operationType enum. YAML using these will execute correctly but
    // fail schema validation. New laws should use NOT + the positive operation instead.
    #[serde(rename = "NOT_EQUALS")]
    NotEquals,
    #[serde(rename = "IS_NULL")]
    IsNull,
    #[serde(rename = "NOT_NULL")]
    NotNull,
    #[serde(rename = "NOT_IN")]
    NotIn,
}

impl Operation {
    /// All operations that are part of the schema specification.
    /// Compat aliases (NOT_EQUALS, IS_NULL, NOT_NULL, NOT_IN) are excluded.
    ///
    /// When adding a new operation: add it here AND to a conformance level
    /// in `conformance/<version>/manifest.json`. CI will fail otherwise.
    pub const SCHEMA_OPERATIONS: &[Operation] = &[
        Operation::Equals,
        Operation::GreaterThan,
        Operation::LessThan,
        Operation::GreaterThanOrEqual,
        Operation::LessThanOrEqual,
        Operation::Add,
        Operation::Subtract,
        Operation::Multiply,
        Operation::Divide,
        Operation::Max,
        Operation::Min,
        Operation::Round,
        Operation::Ceil,
        Operation::Floor,
        Operation::And,
        Operation::Or,
        Operation::Not,
        Operation::If,
        Operation::In,
        Operation::List,
        Operation::ForEach,
        Operation::Age,
        Operation::DateAdd,
        Operation::Date,
        Operation::DayOfWeek,
        Operation::DateDiff,
        Operation::DatePart,
        Operation::StartOf,
    ];

    /// Compat aliases accepted by the engine but not in the schema.
    /// These exist for backward compatibility with older YAML files.
    pub const COMPAT_ALIASES: &[Operation] = &[
        Operation::NotEquals,
        Operation::IsNull,
        Operation::NotNull,
        Operation::NotIn,
    ];

    /// All variants of the enum. This is a manually maintained list;
    /// forgetting to add a new variant here compiles fine, but the
    /// `operation_lists_are_exhaustive` test catches it by cross-checking
    /// this list against SCHEMA_OPERATIONS + COMPAT_ALIASES.
    ///
    /// When adding a new operation: add it here AND to SCHEMA_OPERATIONS
    /// or COMPAT_ALIASES.
    pub const ALL_VARIANTS: &[Operation] = &[
        Operation::Equals,
        Operation::GreaterThan,
        Operation::LessThan,
        Operation::GreaterThanOrEqual,
        Operation::LessThanOrEqual,
        Operation::Add,
        Operation::Subtract,
        Operation::Multiply,
        Operation::Divide,
        Operation::Max,
        Operation::Min,
        Operation::Round,
        Operation::Ceil,
        Operation::Floor,
        Operation::And,
        Operation::Or,
        Operation::Not,
        Operation::If,
        Operation::In,
        Operation::List,
        Operation::ForEach,
        Operation::Age,
        Operation::DateAdd,
        Operation::Date,
        Operation::DayOfWeek,
        Operation::DateDiff,
        Operation::DatePart,
        Operation::StartOf,
        Operation::NotEquals,
        Operation::IsNull,
        Operation::NotNull,
        Operation::NotIn,
    ];

    /// Check if this is a comparison operation
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Operation::Equals
                | Operation::NotEquals
                | Operation::GreaterThan
                | Operation::LessThan
                | Operation::GreaterThanOrEqual
                | Operation::LessThanOrEqual
        )
    }

    /// Check if this is an arithmetic operation
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Operation::Add | Operation::Subtract | Operation::Multiply | Operation::Divide
        )
    }

    /// Check if this is an aggregate operation
    pub fn is_aggregate(&self) -> bool {
        matches!(self, Operation::Max | Operation::Min)
    }

    /// Check if this is a logical operation
    pub fn is_logical(&self) -> bool {
        matches!(self, Operation::And | Operation::Or | Operation::Not)
    }

    /// Check if this is a conditional operation
    pub fn is_conditional(&self) -> bool {
        matches!(self, Operation::If)
    }

    /// Check if this is a collection operation
    pub fn is_collection(&self) -> bool {
        matches!(self, Operation::In | Operation::List)
    }

    /// Check if this is a null-check operation
    pub fn is_null_check(&self) -> bool {
        matches!(self, Operation::IsNull | Operation::NotNull)
    }

    /// Get the operation name as a static uppercase string.
    ///
    /// Avoids per-invocation `format!("{:?}", op).to_uppercase()` allocations.
    pub fn name(&self) -> &'static str {
        match self {
            Operation::Equals => "EQUALS",
            Operation::GreaterThan => "GREATER_THAN",
            Operation::LessThan => "LESS_THAN",
            Operation::GreaterThanOrEqual => "GREATER_THAN_OR_EQUAL",
            Operation::LessThanOrEqual => "LESS_THAN_OR_EQUAL",
            Operation::Add => "ADD",
            Operation::Subtract => "SUBTRACT",
            Operation::Multiply => "MULTIPLY",
            Operation::Divide => "DIVIDE",
            Operation::Max => "MAX",
            Operation::Min => "MIN",
            Operation::Round => "ROUND",
            Operation::Ceil => "CEIL",
            Operation::Floor => "FLOOR",
            Operation::And => "AND",
            Operation::Or => "OR",
            Operation::Not => "NOT",
            Operation::If => "IF",
            Operation::In => "IN",
            Operation::List => "LIST",
            Operation::ForEach => "FOREACH",
            Operation::Age => "AGE",
            Operation::DateAdd => "DATE_ADD",
            Operation::Date => "DATE",
            Operation::DayOfWeek => "DAY_OF_WEEK",
            Operation::DateDiff => "DATE_DIFF",
            Operation::DatePart => "DATE_PART",
            Operation::StartOf => "START_OF",
            Operation::NotEquals => "NOT_EQUALS",
            Operation::IsNull => "IS_NULL",
            Operation::NotNull => "NOT_NULL",
            Operation::NotIn => "NOT_IN",
        }
    }
}

/// Re-export the canonical regulatory layer types from the shared crate.
pub use regelrecht_shared::RegulatoryLayer;

/// Parameter type specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    #[default]
    String,
    Number,
    Boolean,
    Amount,
    Date,
    Array,
    Object,
    /// A decision value `{basis, fields, findings, outputs}`: the
    /// article that decided, the parameters it was given and declares, its
    /// findings per case, and the outputs of its decision item (schema
    /// v0.8.4, RFC-045 §6). A value built from a recorded decision may also
    /// hold `taken_on`. At run time an object.
    Decision,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_value_bool_conversion() {
        assert!(Value::Bool(true).to_bool());
        assert!(!Value::Bool(false).to_bool());
        assert!(!Value::Null.to_bool());
        assert!(Value::Int(1).to_bool());
        assert!(!Value::Int(0).to_bool());
        assert!(Value::String("hello".to_string()).to_bool());
        assert!(!Value::String("".to_string()).to_bool());
        assert!(Value::Decimal(dec!(1.5)).to_bool());
        assert!(!Value::Decimal(dec!(0.0)).to_bool());
        // Untranslatable is falsy
        assert!(!Value::Untranslatable {
            article: "1".into(),
            construct: "test".into(),
        }
        .to_bool());
    }

    #[test]
    fn test_value_from_primitives() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42i64), Value::Int(42));
        assert_eq!(Value::from(2.75f64), Value::Decimal(dec!(2.75)));
        // Whole-number floats coerce to Int
        assert_eq!(Value::from(5.0f64), Value::Int(5));
        // Non-finite floats have no Decimal representation -> Null
        assert_eq!(Value::from(f64::NAN), Value::Null);
        assert_eq!(Value::from("test"), Value::String("test".to_string()));
    }

    #[test]
    fn test_value_as_methods() {
        let bool_val = Value::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));
        assert_eq!(bool_val.as_int(), None);

        let int_val = Value::Int(42);
        assert_eq!(int_val.as_int(), Some(42));
        assert_eq!(int_val.as_float(), Some(42.0));
        assert_eq!(Value::Decimal(dec!(12.5)).as_float(), Some(12.5));
        // A decimal goes to JSON as the number it is, by reference and by value.
        assert_eq!(
            serde_json::Value::from(&Value::Decimal(dec!(12.5))),
            serde_json::json!(12.5)
        );
        assert_eq!(
            serde_json::Value::from(Value::Decimal(dec!(12.5))),
            serde_json::json!(12.5)
        );

        let str_val = Value::String("hello".to_string());
        assert_eq!(str_val.as_str(), Some("hello"));
    }

    #[test]
    fn test_operation_categories() {
        assert!(Operation::Equals.is_comparison());
        assert!(Operation::NotEquals.is_comparison());
        assert!(Operation::Add.is_arithmetic());
        assert!(Operation::Max.is_aggregate());
        assert!(Operation::And.is_logical());
        assert!(Operation::Not.is_logical());
        assert!(Operation::If.is_conditional());
        assert!(Operation::In.is_collection());
        assert!(Operation::List.is_collection());
        assert!(Operation::IsNull.is_null_check());
        assert!(Operation::NotNull.is_null_check());
    }

    #[test]
    fn test_value_decimal_equality() {
        assert_eq!(Value::Decimal(dec!(1.5)), Value::Decimal(dec!(1.5)));
        assert_ne!(Value::Decimal(dec!(1.5)), Value::Decimal(dec!(2.5)));
        // Int and Decimal are distinct variants at the Value level (numeric
        // cross-type equality is handled by the engine's `values_equal`).
        assert_ne!(Value::Int(1), Value::Decimal(dec!(1.0)));
    }

    #[test]
    fn test_untranslatable_equality() {
        let a = Value::Untranslatable {
            article: "1".into(),
            construct: "rounding".into(),
        };
        let b = Value::Untranslatable {
            article: "2".into(),
            construct: "aggregation".into(),
        };
        // Two Untranslatable values are always equal (like NaN)
        assert_eq!(a, b);
        // Untranslatable != other types
        assert_ne!(a, Value::Null);
        assert_ne!(a, Value::Int(0));
    }

    #[test]
    fn test_untranslatable_serde_roundtrip() {
        let value = Value::Untranslatable {
            article: "2".into(),
            construct: "afronden op hele euro's".into(),
        };
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("__untranslatable"));
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_untranslatable());
    }

    fn missing(law: &str, name: &str, kind: MissingKind) -> MissingFact {
        MissingFact {
            law: law.into(),
            name: name.into(),
            kind,
        }
    }

    #[test]
    fn test_unknown_constructor_and_accessors() {
        let unknown = Value::unknown("wet_huur", "huur", MissingKind::NoData);
        assert!(unknown.is_unknown());
        assert!(!unknown.is_null());
        assert_eq!(
            unknown.missing_facts(),
            &[missing("wet_huur", "huur", MissingKind::NoData)]
        );
        // Every other variant misses nothing.
        assert!(Value::Null.missing_facts().is_empty());
        assert!(Value::Int(1).missing_facts().is_empty());
        assert_eq!(unknown.type_name(), "unknown");
        // Display code may ask for a truth value; the engine never does.
        assert!(!unknown.to_bool());
    }

    #[test]
    fn test_unknown_equality_ignores_provenance() {
        let a = Value::unknown("wet_a", "huur", MissingKind::NoData);
        let b = Value::unknown("wet_b", "partner_bsn", MissingKind::NotPassed);
        // Like Untranslatable: both say "not decidable".
        assert_eq!(a, b);
        // Unknown is neither absence nor a taint nor a value.
        assert_ne!(a, Value::Null);
        assert_ne!(
            a,
            Value::Untranslatable {
                article: "1".into(),
                construct: "x".into(),
            }
        );
        assert_ne!(a, Value::Bool(false));
    }

    #[test]
    fn test_unknown_serde_roundtrip() {
        let value = Value::Unknown(vec![
            missing("wet_huur", "huur", MissingKind::NoData),
            missing("wet_brp", "partner_bsn", MissingKind::NotPassed),
        ]);
        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "__unknown": true,
                "missing": [
                    {"law": "wet_huur", "name": "huur", "kind": "no_data"},
                    {"law": "wet_brp", "name": "partner_bsn", "kind": "not_passed"},
                ],
            })
        );
        // Through serde…
        let parsed: Value = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(parsed.missing_facts(), value.missing_facts());
        // …and through the From conversions, in both directions.
        let converted = Value::from(json.clone());
        assert_eq!(converted.missing_facts(), value.missing_facts());
        assert_eq!(serde_json::Value::from(&value), json);
        assert_eq!(serde_json::Value::from(value), json);
    }

    #[test]
    fn test_unknown_deserialization_dedups_and_orders_by_first_appearance() {
        let json = serde_json::json!({
            "__unknown": true,
            "missing": [
                {"law": "w", "name": "b", "kind": "no_data"},
                {"law": "w", "name": "a", "kind": "no_data"},
                {"law": "w", "name": "b", "kind": "no_data"},
            ],
        });
        let parsed: Value = serde_json::from_value(json).unwrap();
        assert_eq!(
            parsed.missing_facts(),
            &[
                missing("w", "b", MissingKind::NoData),
                missing("w", "a", MissingKind::NoData),
            ]
        );
    }

    #[test]
    fn test_unknown_marker_without_facts_is_not_an_unknown() {
        // An Unknown without provenance would be "null" under another name,
        // which is exactly what RFC-036 rules out. Both readers, serde
        // `Deserialize` and `From<serde_json::Value>`, keep such a sentinel
        // as the plain object it is; neither fails and neither invents facts.
        let cases = [
            serde_json::json!({"__unknown": true, "missing": []}),
            serde_json::json!({"__unknown": true}),
            serde_json::json!({
                "__unknown": true,
                "missing": [{"law": "w", "name": "x", "kind": "lost"}],
            }),
            serde_json::json!({"__unknown": true, "missing": "huur"}),
        ];
        for malformed in cases {
            let deserialized: Value = serde_json::from_value(malformed.clone())
                .expect("a malformed sentinel deserializes as an object");
            let converted = Value::from(malformed.clone());
            assert!(matches!(deserialized, Value::Object(_)), "{malformed}");
            assert_eq!(deserialized, converted, "{malformed}");
            // The object keeps its marker key, so nothing is silently dropped.
            assert_eq!(
                deserialized.as_object().and_then(|o| o.get("__unknown")),
                Some(&Value::Bool(true))
            );
        }
    }

    #[test]
    fn test_contains_unknown_looks_inside_containers() {
        let huur = Value::unknown("wet_huur", "huur", MissingKind::NoData);
        assert!(huur.contains_unknown());
        assert!(!Value::Int(1).contains_unknown());
        assert!(!Value::Null.contains_unknown());
        assert!(Value::Array(vec![Value::Int(1), huur.clone()]).contains_unknown());
        let mut record = BTreeMap::new();
        record.insert("status".to_string(), Value::String("ACTIEF".to_string()));
        record.insert(
            "bedragen".to_string(),
            Value::Array(vec![Value::Array(vec![huur.clone()])]),
        );
        assert!(Value::Object(record).contains_unknown());
        assert!(!Value::Array(vec![Value::Array(vec![Value::Null])]).contains_unknown());
    }

    #[test]
    fn test_merge_unknown_deep_unites_nested_facts() {
        let huur = Value::unknown("wet_huur", "huur", MissingKind::NoData);
        let partner = Value::unknown("wet_huur", "partner_bsn", MissingKind::NoData);
        let mut record = BTreeMap::new();
        record.insert("partner".to_string(), partner.clone());
        let left = Value::Array(vec![Value::Int(1), huur.clone()]);
        let right = Value::Object(record);
        let merged = Value::merge_unknown_deep([&left, &right, &huur]).unwrap();
        assert_eq!(
            merged.missing_facts(),
            &[
                missing("wet_huur", "huur", MissingKind::NoData),
                missing("wet_huur", "partner_bsn", MissingKind::NoData),
            ]
        );
        // The shallow merge does not see them; the deep one is what the
        // structural operations must use.
        assert_eq!(Value::merge_unknown([&left, &right]), None);
        assert_eq!(
            Value::merge_unknown_deep([&Value::Array(vec![Value::Int(1)]), &Value::Null]),
            None
        );
    }

    #[test]
    fn test_merge_unknown_is_the_ordered_union() {
        let huur = Value::unknown("wet_huur", "huur", MissingKind::NoData);
        let partner = Value::unknown("wet_huur", "partner_bsn", MissingKind::NoData);
        let merged = Value::merge_unknown([&Value::Int(5), &huur, &partner, &huur]).unwrap();
        assert_eq!(
            merged.missing_facts(),
            &[
                missing("wet_huur", "huur", MissingKind::NoData),
                missing("wet_huur", "partner_bsn", MissingKind::NoData),
            ]
        );
        // The same name under another law or another reason is another fact.
        let elsewhere = Value::unknown("wet_brp", "huur", MissingKind::NotPassed);
        let merged = Value::merge_unknown([&huur, &elsewhere]).unwrap();
        assert_eq!(merged.missing_facts().len(), 2);
        // No Unknown among the operands: nothing to propagate.
        assert_eq!(Value::merge_unknown([&Value::Int(1), &Value::Null]), None);
        assert_eq!(Value::merge_unknown(std::iter::empty()), None);
    }

    #[test]
    fn test_unknown_display_names_the_facts() {
        let value = Value::Unknown(vec![
            missing("wet_huur", "huur", MissingKind::NoData),
            missing("wet_brp", "partner_bsn", MissingKind::NotPassed),
        ]);
        assert_eq!(
            value.to_string(),
            "UNKNOWN(wet_huur.huur, wet_brp.partner_bsn)"
        );
    }

    #[test]
    fn test_value_serde_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Decimal(dec!(3.14)),
            Value::String("test".to_string()),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
        ];

        for value in values {
            let json = serde_json::to_string(&value).unwrap();
            let parsed: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value, parsed);
        }
    }

    #[test]
    fn operation_lists_are_exhaustive() {
        // ALL_VARIANTS must contain every variant. We verify this by
        // checking that ALL_VARIANTS and SCHEMA_OPERATIONS + COMPAT_ALIASES
        // contain the exact same set of operations (by name).
        let all_names: std::collections::HashSet<&str> =
            Operation::ALL_VARIANTS.iter().map(|op| op.name()).collect();
        assert_eq!(
            all_names.len(),
            Operation::ALL_VARIANTS.len(),
            "ALL_VARIANTS contains duplicates"
        );

        let classified_names: std::collections::HashSet<&str> = Operation::SCHEMA_OPERATIONS
            .iter()
            .chain(Operation::COMPAT_ALIASES.iter())
            .map(|op| op.name())
            .collect();
        assert_eq!(
            classified_names.len(),
            Operation::SCHEMA_OPERATIONS.len() + Operation::COMPAT_ALIASES.len(),
            "SCHEMA_OPERATIONS and COMPAT_ALIASES overlap"
        );

        assert_eq!(
            all_names,
            classified_names,
            "ALL_VARIANTS and SCHEMA_OPERATIONS + COMPAT_ALIASES differ.\n\
             In ALL_VARIANTS but not classified: {:?}\n\
             Classified but not in ALL_VARIANTS: {:?}",
            all_names.difference(&classified_names).collect::<Vec<_>>(),
            classified_names.difference(&all_names).collect::<Vec<_>>()
        );
    }
}
