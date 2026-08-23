use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

use crate::value::{Object, StringKey, Value};

fn to_json(v: &Value) -> Result<JsonValue, String> {
    Ok(match v {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::I64(i) => JsonValue::Number((*i).into()),
        Value::F64(f) => JsonValue::Number(
            JsonNumber::from_f64(*f)
                .ok_or_else(|| format!("cannot serialize non-finite float {f}"))?,
        ),
        Value::String(s) => JsonValue::String(s.to_string()),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(to_json(item)?);
            }
            JsonValue::Array(out)
        }
        Value::Object(obj) => {
            let mut map = JsonMap::new();
            for (k, val) in obj.iter() {
                map.insert(k.as_ref().to_string(), to_json(val)?);
            }
            JsonValue::Object(map)
        }
    })
}

fn from_json(jv: &JsonValue) -> Value {
    match jv {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::I64(i)
            } else {
                Value::F64(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        JsonValue::String(s) => Value::String(s.as_str().into()),
        JsonValue::Array(items) => Value::Array(items.iter().map(from_json).collect()),
        JsonValue::Object(map) => {
            let mut obj = Object::new();
            for (k, v) in map {
                obj.entries
                    .push((StringKey(k.as_str().into()), from_json(v)));
            }
            Value::Object(obj)
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        to_json(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let jv = JsonValue::deserialize(deserializer)?;
        Ok(from_json(&jv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_mapping_is_strict_for_i64() {
        assert_eq!(from_json(&serde_json::json!(42)), Value::I64(42));
        assert!(matches!(from_json(&serde_json::json!(2.5)), Value::F64(_)));
    }

    /// Integers above i64::MAX cannot fit our model: documented lossy F64.
    #[test]
    fn u64_beyond_i64_max_maps_to_f64_lossy() {
        let v = from_json(&serde_json::json!(u64::MAX));
        assert_eq!(v, Value::F64(u64::MAX as f64));
    }

    #[test]
    fn non_finite_float_rejected_at_serialization() {
        assert!(to_json(&Value::F64(f64::NAN)).is_err());
    }
}
