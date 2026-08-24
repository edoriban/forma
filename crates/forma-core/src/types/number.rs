use std::marker::PhantomData;
use std::sync::OnceLock;

use crate::error::{
    FieldPath, FormaError, FormaIssue, IssueCode, IssueParams, ParamKey, ParamValue, Sink,
};
use crate::rule::{ClosureRule, Rule};
use crate::schema::{ConstraintDesc, DynSchema, FieldMeta, Schema, ShapeKind, ShapeNode};
use crate::value::Value;

/// Private supertrait sealing [`NumberValue`].
mod sealed {
    pub trait Sealed {}
    impl Sealed for f64 {}
    impl Sealed for i64 {}
}

/// Numeric primitives supported by [`NumberSchema`]: `f64` and `i64`.
///
/// Sealed so downstream crates cannot add families.
pub trait NumberValue:
    sealed::Sealed + Copy + PartialOrd + std::fmt::Debug + Send + Sync + 'static
{
    #[doc(hidden)]
    fn zero() -> Self;
    #[doc(hidden)]
    fn is_integral(self) -> bool;
    #[doc(hidden)]
    fn is_finite_num(self) -> bool;
    #[doc(hidden)]
    fn to_param(self) -> ParamValue;
    #[doc(hidden)]
    fn to_value(self) -> Value;
    #[doc(hidden)]
    fn try_from_value(v: &Value) -> Option<Self>;
}

impl NumberValue for f64 {
    fn zero() -> Self {
        0.0
    }
    fn is_integral(self) -> bool {
        self.fract() == 0.0
    }
    fn is_finite_num(self) -> bool {
        self.is_finite()
    }
    fn to_param(self) -> ParamValue {
        ParamValue::F64(self)
    }
    fn to_value(self) -> Value {
        Value::F64(self)
    }
    fn try_from_value(v: &Value) -> Option<Self> {
        v.as_f64()
    }
}

impl NumberValue for i64 {
    fn zero() -> Self {
        0
    }
    fn is_integral(self) -> bool {
        true
    }
    fn is_finite_num(self) -> bool {
        true
    }
    fn to_param(self) -> ParamValue {
        ParamValue::I64(self)
    }
    fn to_value(self) -> Value {
        Value::I64(self)
    }
    fn try_from_value(v: &Value) -> Option<Self> {
        v.as_i64()
    }
}

/// Builtin numeric constraints over `T`, stored in declaration order.
#[derive(Clone, Debug, PartialEq)]
pub enum NumCheck<T> {
    /// Inclusive lower bound.
    Min(T),
    /// Inclusive upper bound.
    Max(T),
    /// Strictly greater than zero.
    Positive,
    /// Rejects NaN/infinity.
    Finite,
    /// Rejects fractional values.
    Int,
}

/// Numeric builder schema over `f64` or `i64`; same single-IR design as strings.
pub struct NumberSchema<T: NumberValue> {
    checks: Vec<NumCheck<T>>,
    rules: Vec<Box<dyn Rule<T>>>,
    meta: FieldMeta,
    fail_fast: bool,
    shape_cache: OnceLock<ShapeNode>,
    _marker: PhantomData<fn() -> T>,
}

/// Clones carry checks/meta/flags but not boxed rules (rules are not clonable);
/// build fresh instances when erasure needs identical behavior.
impl<T: NumberValue> Clone for NumberSchema<T> {
    fn clone(&self) -> Self {
        Self {
            checks: self.checks.clone(),
            rules: Vec::new(),
            meta: self.meta.clone(),
            fail_fast: self.fail_fast,
            shape_cache: OnceLock::new(),
            _marker: PhantomData,
        }
    }
}

/// Creates an empty schema via [`number()`].
impl<T: NumberValue> Default for NumberSchema<T> {
    fn default() -> Self {
        Self {
            checks: Vec::new(),
            rules: Vec::new(),
            meta: FieldMeta::default(),
            fail_fast: false,
            shape_cache: OnceLock::new(),
            _marker: PhantomData,
        }
    }
}

/// Structural debug output.
impl<T: NumberValue> std::fmt::Debug for NumberSchema<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NumberSchema")
            .field("checks", &self.checks)
            .field("fail_fast", &self.fail_fast)
            .finish_non_exhaustive()
    }
}

impl<T: NumberValue> NumberSchema<T> {
    /// Requires `value >= n`.
    #[must_use]
    pub fn min(mut self, n: T) -> Self {
        self.checks.push(NumCheck::Min(n));
        self
    }

    /// Requires `value <= n`.
    #[must_use]
    pub fn max(mut self, n: T) -> Self {
        self.checks.push(NumCheck::Max(n));
        self
    }

    /// Rejects values with a fractional part (always passes for i64).
    #[must_use]
    pub fn int(mut self) -> Self {
        self.checks.push(NumCheck::Int);
        self
    }

    /// Requires strictly positive (zero fails).
    #[must_use]
    pub fn positive(mut self) -> Self {
        self.checks.push(NumCheck::Positive);
        self
    }

    /// Rejects NaN/infinity (always passes for i64).
    #[must_use]
    pub fn finite(mut self) -> Self {
        self.checks.push(NumCheck::Finite);
        self
    }

    /// Appends a refinement closure; runs strictly after builtin checks pass.
    #[must_use]
    pub fn refine<F>(mut self, f: F) -> Self
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let ordinal = self.rules.len();
        self.rules
            .push(Box::new(ClosureRule::with_ordinal(ordinal, f)));
        self
    }

    /// Appends a custom [`Rule`] implementation.
    #[must_use]
    pub fn rule<R>(mut self, rule: R) -> Self
    where
        R: Rule<T> + 'static,
    {
        self.rules.push(Box::new(rule));
        self
    }

    /// Stops at the first violated constraint with exactly one issue (ER-4).
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

    fn bridge(v: &Value) -> Result<T, IssueCode> {
        T::try_from_value(v).ok_or(IssueCode::TypeMismatch)
    }

    fn run_checks(&self, value: T, path: &FieldPath) -> Vec<FormaIssue> {
        self.run_checks_with(value, path, self.fail_fast)
    }

    /// Path/fail-fast-parameterized kernel: object schemas call this with a
    /// joined path and an inherited fail-fast flag (D2).
    fn run_checks_with(&self, value: T, path: &FieldPath, fail_fast: bool) -> Vec<FormaIssue> {
        let mut sink = Sink::new(path, fail_fast);
        for check in &self.checks {
            if let Some(issue) = evaluate_check(check, value)
                && !sink.push(issue)
            {
                return sink.finish().issues;
            }
        }
        if sink.issues.is_empty() && !self.rules.is_empty() {
            for rule in &self.rules {
                if let Some(rejection) = rule.validate(&value) {
                    let issue = crate::schema::issue_at_root(
                        rejection.code.unwrap_or(IssueCode::Refine),
                        rejection.message,
                        rejection.params,
                    );
                    if !sink.push(issue) {
                        break;
                    }
                }
            }
        }
        sink.finish().issues
    }
}

fn to_params(pairs: Vec<(&str, ParamValue)>) -> IssueParams {
    pairs
        .into_iter()
        .map(|(k, v)| (ParamKey::from(k), v))
        .collect()
}

fn evaluate_check<T: NumberValue>(check: &NumCheck<T>, value: T) -> Option<FormaIssue> {
    let (code, message, params) = match check {
        NumCheck::Min(n) => {
            if value >= *n {
                return None;
            }
            (
                IssueCode::Min,
                "number is below the minimum",
                vec![("min", n.to_param())],
            )
        }
        NumCheck::Max(n) => {
            if value <= *n {
                return None;
            }
            (
                IssueCode::Max,
                "number is above the maximum",
                vec![("max", n.to_param())],
            )
        }
        NumCheck::Positive => {
            if value > T::zero() {
                return None;
            }
            (IssueCode::Positive, "number must be positive", Vec::new())
        }
        NumCheck::Finite => {
            if value.is_finite_num() {
                return None;
            }
            (IssueCode::Finite, "number must be finite", Vec::new())
        }
        NumCheck::Int => {
            if value.is_integral() {
                return None;
            }
            (IssueCode::Int, "number must be an integer", Vec::new())
        }
    };
    Some(crate::schema::issue_at_root(
        code,
        message.into(),
        to_params(params),
    ))
}

fn code_of<T>(c: &NumCheck<T>) -> IssueCode {
    match c {
        NumCheck::Min(_) => IssueCode::Min,
        NumCheck::Max(_) => IssueCode::Max,
        NumCheck::Positive => IssueCode::Positive,
        NumCheck::Finite => IssueCode::Finite,
        NumCheck::Int => IssueCode::Int,
    }
}

impl<T: NumberValue> Schema for NumberSchema<T> {
    type Input = T;
    type Output = T;

    fn parse(&self, input: &Self::Input) -> Result<Self::Output, FormaError> {
        let issues = self.run_checks(*input, &FieldPath::ROOT);
        if issues.is_empty() {
            Ok(*input)
        } else {
            Err(FormaError { issues })
        }
    }
}

impl<T: NumberValue> DynSchema for NumberSchema<T> {
    fn validate_value(&self, v: &Value) -> Vec<FormaIssue> {
        match Self::bridge(v) {
            Ok(n) => self.run_checks(n, &FieldPath::ROOT),
            Err(code) => vec![crate::schema::issue_at_root(
                code,
                "value is not the expected number type".into(),
                Vec::new(),
            )],
        }
    }

    fn shape(&self) -> &ShapeNode {
        self.shape_cache.get_or_init(|| ShapeNode {
            kind: ShapeKind::Number { integer: false },
            constraints: self
                .checks
                .iter()
                .map(|c| ConstraintDesc {
                    code: code_of(c),
                    params: match c {
                        NumCheck::Min(n) => vec![("min".into(), n.to_param())],
                        NumCheck::Max(n) => vec![("max".into(), n.to_param())],
                        _ => Vec::new(),
                    },
                })
                .collect(),
        })
    }

    fn metadata(&self) -> &FieldMeta {
        &self.meta
    }
}

/// Creates a new empty [`NumberSchema`] over `f64` or `i64`.
#[must_use]
pub fn number<T: NumberValue>() -> NumberSchema<T> {
    NumberSchema::default()
}

impl<T: NumberValue> crate::schema::ObjectChild for NumberSchema<T> {
    fn validate_at(
        &self,
        v: &Value,
        path: &FieldPath,
        fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>> {
        match Self::bridge(v) {
            Ok(n) => {
                let issues = self.run_checks_with(n, path, fail_fast);
                if issues.is_empty() {
                    Ok(n.to_value())
                } else {
                    Err(issues)
                }
            }
            Err(code) => Err(vec![FormaIssue {
                path: path.clone(),
                code,
                message: "value is not the expected number type".into(),
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
impl<T: NumberValue> crate::schema::sealed::Sealed for NumberSchema<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IssueCode;

    fn codes(s: &NumberSchema<f64>, v: f64) -> Vec<IssueCode> {
        s.run_checks(v, &FieldPath::ROOT)
            .iter()
            .map(|i| i.code.clone())
            .collect()
    }

    #[test]
    fn sc5_finite_rejects_nan_and_inf() {
        let s = number::<f64>().finite();
        assert_eq!(codes(&s, f64::NAN), vec![IssueCode::Finite]);
        assert_eq!(codes(&s, f64::INFINITY), vec![IssueCode::Finite]);
        assert!(codes(&s, 1.5).is_empty());
    }

    #[test]
    fn sc5_positive_zero_boundary() {
        let s = number::<f64>().positive();
        assert_eq!(codes(&s, 0.0), vec![IssueCode::Positive]);
        assert_eq!(codes(&s, -0.1), vec![IssueCode::Positive]);
        assert!(codes(&s, 0.1).is_empty());
    }

    #[test]
    fn sc5_int_check_rejects_fraction() {
        let s = number::<f64>().int();
        assert_eq!(codes(&s, 2.5), vec![IssueCode::Int]);
        assert!(codes(&s, 2.0).is_empty());
    }

    #[test]
    fn sc5_i64_max_fidelity_through_parse() {
        let s = number::<i64>().min(i64::MAX);
        assert_eq!(s.parse(&i64::MAX), Ok(i64::MAX));
    }
}
