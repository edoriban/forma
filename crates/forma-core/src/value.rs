/// Dependency-free dynamic value tree — the erased currency of the [`crate::schema::DynSchema`] view.
///
/// Object entries preserve insertion order (DV-1) and integer/float variants
/// are kept strictly apart so number fidelity cannot silently degrade.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Absent value.
    Null,
    /// Boolean.
    Bool(bool),
    /// Signed integer with exact precision.
    I64(i64),
    /// IEEE-754 double.
    F64(f64),
    /// UTF-8 string.
    String(Box<str>),
    /// Ordered sequence.
    Array(Vec<Value>),
    /// Insertion-ordered map.
    Object(Object),
}

/// Insertion-ordered object: entries stay in declaration order (DV-1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Object {
    /// Key-value pairs in insertion order.
    pub entries: Vec<(StringKey, Value)>,
}

/// Owned object key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StringKey(Box<str>);

impl StringKey {
    /// Creates a key from a string slice.
    #[must_use]
    pub fn new(s: &str) -> Self {
        Self(s.into())
    }
}

impl AsRef<str> for StringKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Object {
    /// Creates an empty object.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Inserts a key-value pair. An existing key keeps its position but its
    /// value is replaced (last write wins, matching JSON semantics); a new
    /// key is appended at the end (insertion order preserved).
    pub fn insert(&mut self, key: &str, value: Value) {
        match self.entries.iter_mut().find(|(k, _)| k.as_ref() == key) {
            Some((_, v)) => *v = value,
            None => self.entries.push((StringKey(key.into()), value)),
        }
    }

    /// Iterates entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&StringKey, &Value)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// Linear-scan lookup over the ordered entries; `Some` only when the
    /// key is present (presence is structural — the value itself is not
    /// inspected, so a present [`Value::Null`] still counts).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(k, _)| k.as_ref() == key)
            .map(|(_, v)| v)
    }
}

/// Lossless conversion of validated schema outputs into [`Value`]s.
///
/// Implemented for the realistic form-coercion targets; `u64`/`usize` are
/// deliberately excluded because they do not fit the `I64` fidelity rules
/// (DV-1) without silent truncation.
///
/// The exclusion is enforced at compile time: `coerced::<u64>()` is a valid
/// standalone schema, but it cannot back an object field because the
/// [`crate::schema::ObjectChild`] impl requires `T: ToValue`.
///
/// ```compile_fail,E0277
/// use forma_core::prelude::*;
///
/// let _ = object().field("n", coerced::<u64>()); // u64 lacks ToValue
/// ```
pub trait ToValue {
    /// Converts the value into its exact [`Value`] representation.
    fn to_value(self) -> Value;
}

impl ToValue for i8 {
    fn to_value(self) -> Value {
        Value::I64(i64::from(self))
    }
}

impl ToValue for i16 {
    fn to_value(self) -> Value {
        Value::I64(i64::from(self))
    }
}

impl ToValue for i32 {
    fn to_value(self) -> Value {
        Value::I64(i64::from(self))
    }
}

impl ToValue for i64 {
    fn to_value(self) -> Value {
        Value::I64(self)
    }
}

impl ToValue for u8 {
    fn to_value(self) -> Value {
        Value::I64(i64::from(self))
    }
}

impl ToValue for u16 {
    fn to_value(self) -> Value {
        Value::I64(i64::from(self))
    }
}

impl ToValue for u32 {
    fn to_value(self) -> Value {
        Value::I64(i64::from(self))
    }
}

impl ToValue for f32 {
    fn to_value(self) -> Value {
        Value::F64(f64::from(self))
    }
}

impl ToValue for f64 {
    fn to_value(self) -> Value {
        Value::F64(self)
    }
}

impl ToValue for String {
    fn to_value(self) -> Value {
        Value::String(self.into())
    }
}

impl ToValue for bool {
    fn to_value(self) -> Value {
        Value::Bool(self)
    }
}

impl Value {
    /// Strict accessor: `Some` only for the String variant.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Strict accessor: `Some` only for the Bool variant.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Strict: never widens or narrows between integer and float variants.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(i) => Some(*i),
            _ => None,
        }
    }

    /// Strict: never coerces from `I64`; explicit converters provided instead.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(f) => Some(*f),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s.into())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Self::I64(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::F64(f)
    }
}

#[cfg(test)]
mod tests {
    use super::{Object, ToValue, Value};

    #[test]
    fn d5_get_finds_present_key() {
        let mut o = Object::new();
        o.insert("name", Value::String("Ada".into()));
        assert_eq!(o.get("name"), Some(&Value::String("Ada".into())));
    }

    #[test]
    fn d5_get_misses_absent_key() {
        let mut o = Object::new();
        o.insert("name", Value::String("Ada".into()));
        assert_eq!(o.get("age"), None);
    }

    #[test]
    fn d5_get_on_empty_object() {
        let o = Object::new();
        assert_eq!(o.get("anything"), None);
    }

    #[test]
    fn d4_tovalue_mappings_are_pinned() {
        assert_eq!((-1i8).to_value(), Value::I64(-1));
        assert_eq!((-2i16).to_value(), Value::I64(-2));
        assert_eq!((-3i32).to_value(), Value::I64(-3));
        assert_eq!((-4i64).to_value(), Value::I64(-4));
        assert_eq!(1u8.to_value(), Value::I64(1));
        assert_eq!(2u16.to_value(), Value::I64(2));
        assert_eq!(3u32.to_value(), Value::I64(3));
        assert_eq!(0.5f32.to_value(), Value::F64(0.5));
        assert_eq!(1.5f64.to_value(), Value::F64(1.5));
        assert_eq!(String::from("hi").to_value(), Value::String("hi".into()));
        assert_eq!(true.to_value(), Value::Bool(true));
        assert_eq!(false.to_value(), Value::Bool(false));
    }

    #[test]
    fn dv1_duplicate_insert_last_write_wins_in_place() {
        let mut o = Object::new();
        o.insert("a", Value::I64(1));
        o.insert("b", Value::I64(2));
        o.insert("a", Value::I64(3));
        assert_eq!(o.get("a"), Some(&Value::I64(3)), "last write wins");
        assert_eq!(o.entries.len(), 2, "no duplicate entry appended");
        let keys: Vec<&str> = o.iter().map(|(k, _)| k.as_ref()).collect();
        assert_eq!(keys, vec!["a", "b"], "original position kept");
    }

    #[test]
    fn dv1_ordered_entries_preserved() {
        let mut o = Object::new();
        o.insert("b", Value::I64(1));
        o.insert("a", Value::I64(2));
        o.insert("c", Value::I64(3));
        let keys: Vec<&str> = o.iter().map(|(k, _)| k.as_ref()).collect();
        assert_eq!(keys, vec!["b", "a", "c"]);
    }

    #[test]
    fn dv1_number_fidelity_i64_max_and_f64() {
        let v = Value::I64(i64::MAX);
        assert_eq!(v.as_i64(), Some(i64::MAX));
        let f = Value::F64(0.1);
        assert_eq!(f.as_f64(), Some(0.1));
        assert_eq!(f.as_i64(), None, "strict: no silent F64-to-I64 cast");
    }

    #[test]
    fn accessor_strictness_no_silent_casts() {
        assert_eq!(Value::String("x".into()).as_str(), Some("x"));
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
        assert_eq!(Value::Null.as_str(), None);
        assert_eq!(Value::F64(1.0).as_i64(), None);
        assert_eq!(Value::I64(1).as_f64(), None, "strict: no silent I64-to-F64");
    }

    #[test]
    fn from_impls_build_expected_variants() {
        assert_eq!(Value::from("hi"), Value::String("hi".into()));
        assert_eq!(Value::from(String::from("hi")), Value::String("hi".into()));
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(7i64), Value::I64(7));
        assert_eq!(Value::from(0.5f64), Value::F64(0.5));
    }
}

#[cfg(feature = "serde")]
pub mod json;
