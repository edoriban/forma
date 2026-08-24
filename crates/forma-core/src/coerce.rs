use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::error::{FormaError, FormaIssue, IssueCode};
use crate::schema::{ConstraintDesc, DynSchema, FieldMeta, Schema, ShapeKind, ShapeNode};
use crate::value::{ToValue, Value};

/// Parses strings into `T: FromStr` — the duality citizen with
/// `Input = String`, `Output = T` (SC-8), mirroring HTML form input.
pub struct CoercedSchema<T: FromStr> {
    meta: FieldMeta,
    fail_fast: bool,
    shape_cache: OnceLock<ShapeNode>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: FromStr> Clone for CoercedSchema<T> {
    fn clone(&self) -> Self {
        Self {
            meta: self.meta.clone(),
            fail_fast: self.fail_fast,
            shape_cache: OnceLock::new(),
            _marker: PhantomData,
        }
    }
}

/// Creates an empty schema via [`coerced()`].
impl<T: FromStr> Default for CoercedSchema<T> {
    fn default() -> Self {
        Self {
            meta: FieldMeta::default(),
            fail_fast: false,
            shape_cache: OnceLock::new(),
            _marker: PhantomData,
        }
    }
}

/// Type-erased debug output.
impl<T: FromStr> std::fmt::Debug for CoercedSchema<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoercedSchema").finish()
    }
}

impl<T: FromStr> CoercedSchema<T> {
    /// Flag present for API uniformity; coercion emits at most one issue anyway.
    #[must_use]
    pub fn fail_fast(mut self) -> Self {
        self.fail_fast = true;
        self
    }

    /// Sets the UI label metadata slot.
    #[must_use]
    pub fn label(mut self, label: &'static str) -> Self {
        self.meta.label = Some(label.into());
        self
    }

    /// Sets the UI description metadata slot.
    #[must_use]
    pub fn description(mut self, description: &'static str) -> Self {
        self.meta.description = Some(description.into());
        self
    }

    /// Sets the UI placeholder metadata slot.
    #[must_use]
    pub fn placeholder(mut self, placeholder: &'static str) -> Self {
        self.meta.placeholder = Some(placeholder.into());
        self
    }

    fn bridge(v: &Value) -> Result<Option<&str>, ()> {
        match v {
            Value::String(s) => Ok(Some(s)),
            _ => Err(()),
        }
    }

    fn coerce(input: &str) -> Result<T, FormaIssue> {
        T::from_str(input).map_err(|_| {
            crate::schema::issue_at_root(
                IssueCode::Coerce,
                format!("cannot parse {input:?} into the target type").into(),
                Vec::new(),
            )
        })
    }
}

impl<T: FromStr> Schema for CoercedSchema<T> {
    type Input = String;
    type Output = T;

    fn parse(&self, input: &Self::Input) -> Result<Self::Output, FormaError> {
        Self::coerce(input).map_err(|issue| FormaError {
            issues: vec![issue],
        })
    }
}

impl<T: FromStr + 'static> DynSchema for CoercedSchema<T> {
    fn validate_value(&self, v: &Value) -> Vec<FormaIssue> {
        match Self::bridge(v) {
            Ok(Some(s)) => match Self::coerce(s) {
                Ok(_) => Vec::new(),
                Err(issue) => vec![issue],
            },
            _ => vec![crate::schema::issue_at_root(
                IssueCode::TypeMismatch,
                "value is not a string".into(),
                Vec::new(),
            )],
        }
    }

    fn shape(&self) -> &ShapeNode {
        self.shape_cache.get_or_init(|| ShapeNode {
            kind: ShapeKind::Coerced,
            constraints: vec![ConstraintDesc {
                code: IssueCode::Coerce,
                params: Vec::new(),
            }],
        })
    }

    fn metadata(&self) -> &FieldMeta {
        &self.meta
    }
}

/// Creates a schema that parses strings into `T` (HTML form inputs always yield strings).
#[must_use]
pub fn coerced<T: FromStr>() -> CoercedSchema<T> {
    CoercedSchema::default()
}

impl<T: FromStr + ToValue + 'static> crate::schema::ObjectChild for CoercedSchema<T> {
    fn validate_at(
        &self,
        v: &Value,
        path: &crate::error::FieldPath,
        _fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>> {
        match Self::bridge(v) {
            Ok(Some(s)) => match Self::coerce(s) {
                Ok(t) => Ok(t.to_value()),
                Err(mut issue) => {
                    issue.path = path.clone();
                    Err(vec![issue])
                }
            },
            _ => Err(vec![FormaIssue {
                path: path.clone(),
                code: IssueCode::TypeMismatch,
                message: "value is not a string".into(),
                params: Vec::new(),
            }]),
        }
    }

    fn shape_node(&self) -> ShapeNode {
        self.shape().clone()
    }

    fn meta(&self) -> &FieldMeta {
        &self.meta
    }
}

impl<T: FromStr + ToValue> crate::schema::sealed::Sealed for CoercedSchema<T> {}

#[cfg(test)]
mod tests {
    use crate::coerce::coerced;
    use crate::error::IssueCode;
    use crate::schema::{DynSchema, Schema};
    use crate::value::Value;

    #[test]
    fn sc8_coercion_success() {
        let s = coerced::<u32>();
        assert_eq!(s.parse(&"42".to_string()), Ok(42u32));
    }

    #[test]
    fn sc8_coercion_failure_yields_coerce_code() {
        let s = coerced::<u32>();
        let err = s.parse(&"abc".to_string()).unwrap_err();
        assert!(!err.issues.is_empty());
        assert_eq!(err.first().unwrap().code, IssueCode::Coerce);
        assert!(!err.first().unwrap().message.is_empty());
    }

    #[test]
    fn dv5_form_input_string_via_dyn_dynschema() {
        let s: Box<dyn DynSchema> = Box::new(coerced::<u32>());
        let issues = s.validate_value(&Value::String("42".into()));
        assert!(
            issues.is_empty(),
            "HTML-form-style string input must validate via coercion"
        );
        let bad = s.validate_value(&Value::String("abc".into()));
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].code, IssueCode::Coerce);
    }

    #[test]
    fn bridge_type_mismatch_never_panics() {
        let s: Box<dyn DynSchema> = Box::new(coerced::<u32>());
        let issues = s.validate_value(&Value::Bool(true));
        assert_eq!(issues.len(), 1);
    }
}
