use std::sync::{Arc, OnceLock};

use crate::error::{FieldPath, FormaError, FormaIssue, IssueCode, ParamValue, Sink};
use crate::rule::{ClosureRule, Rule};
use crate::schema::{ConstraintDesc, DynSchema, FieldMeta, Schema, ShapeKind, ShapeNode};
use crate::value::Value;

/// Builtin boolean constraints.
#[derive(Clone, Debug, PartialEq)]
pub enum BoolCheck {
    /// Value must equal the wrapped boolean.
    Equals(bool),
}

/// Boolean builder schema; same single-IR dual-view design.
pub struct BoolSchema {
    checks: Vec<BoolCheck>,
    rules: Vec<Arc<dyn Rule<bool>>>,
    meta: FieldMeta,
    fail_fast: bool,
    shape_cache: OnceLock<ShapeNode>,
}

impl Clone for BoolSchema {
    fn clone(&self) -> Self {
        Self {
            checks: self.checks.clone(),
            rules: self.rules.clone(),
            meta: self.meta.clone(),
            fail_fast: self.fail_fast,
            shape_cache: OnceLock::new(),
        }
    }
}

/// Creates an empty schema via [`bool()`].
impl Default for BoolSchema {
    fn default() -> Self {
        Self {
            checks: Vec::new(),
            rules: Vec::new(),
            meta: FieldMeta::default(),
            fail_fast: false,
            shape_cache: OnceLock::new(),
        }
    }
}

/// Structural debug output.
impl std::fmt::Debug for BoolSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoolSchema")
            .field("checks", &self.checks)
            .field("fail_fast", &self.fail_fast)
            .finish_non_exhaustive()
    }
}

impl BoolSchema {
    /// Requires the value to equal `expected`; mismatch yields exactly one issue.
    #[must_use]
    pub fn equals(mut self, expected: bool) -> Self {
        self.checks.push(BoolCheck::Equals(expected));
        self
    }

    /// Appends a refinement closure; runs strictly after builtin checks pass.
    #[must_use]
    pub fn refine<F>(mut self, f: F) -> Self
    where
        F: Fn(&bool) -> bool + Send + Sync + 'static,
    {
        let ordinal = self.rules.len();
        self.rules
            .push(Arc::new(ClosureRule::with_ordinal(ordinal, f)));
        self
    }

    /// Appends a custom [`Rule`] implementation.
    #[must_use]
    pub fn rule<R>(mut self, rule: R) -> Self
    where
        R: Rule<bool> + 'static,
    {
        self.rules.push(Arc::new(rule));
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

    fn bridge(v: &Value) -> Result<bool, IssueCode> {
        v.as_bool().ok_or(IssueCode::TypeMismatch)
    }

    fn run_checks(&self, value: bool, path: &FieldPath) -> Vec<FormaIssue> {
        self.run_checks_with(value, path, self.fail_fast)
    }

    /// Path/fail-fast-parameterized kernel: object schemas call this with a
    /// joined path and an inherited fail-fast flag (D2).
    fn run_checks_with(&self, value: bool, path: &FieldPath, fail_fast: bool) -> Vec<FormaIssue> {
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

fn evaluate_check(check: &BoolCheck, value: bool) -> Option<FormaIssue> {
    match check {
        BoolCheck::Equals(expected) => {
            if value == *expected {
                None
            } else {
                Some(crate::schema::issue_at_root(
                    IssueCode::BoolEquals,
                    "boolean does not equal the expected value".into(),
                    vec![("expected".into(), ParamValue::Bool(*expected))],
                ))
            }
        }
    }
}

impl Schema for BoolSchema {
    type Input = bool;
    type Output = bool;

    fn parse(&self, input: &Self::Input) -> Result<Self::Output, FormaError> {
        let issues = self.run_checks(*input, &FieldPath::ROOT);
        if issues.is_empty() {
            Ok(*input)
        } else {
            Err(FormaError { issues })
        }
    }
}

impl DynSchema for BoolSchema {
    fn validate_value(&self, v: &Value) -> Vec<FormaIssue> {
        match Self::bridge(v) {
            Ok(b) => self.run_checks(b, &FieldPath::ROOT),
            Err(code) => vec![crate::schema::issue_at_root(
                code,
                "value is not a boolean".into(),
                Vec::new(),
            )],
        }
    }

    fn shape(&self) -> &ShapeNode {
        self.shape_cache.get_or_init(|| ShapeNode {
            kind: ShapeKind::Bool,
            constraints: self
                .checks
                .iter()
                .map(|c| ConstraintDesc {
                    code: IssueCode::BoolEquals,
                    params: match c {
                        BoolCheck::Equals(expected) => {
                            vec![("expected".into(), ParamValue::Bool(*expected))]
                        }
                    },
                })
                .collect(),
        })
    }

    fn metadata(&self) -> &FieldMeta {
        &self.meta
    }
}

/// Creates a new empty [`BoolSchema`].
#[must_use]
pub fn bool() -> BoolSchema {
    BoolSchema::default()
}

impl crate::schema::ObjectChild for BoolSchema {
    fn validate_at(
        &self,
        v: &Value,
        path: &FieldPath,
        fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>> {
        match Self::bridge(v) {
            Ok(b) => {
                let issues = self.run_checks_with(b, path, fail_fast);
                if issues.is_empty() {
                    Ok(Value::Bool(b))
                } else {
                    Err(issues)
                }
            }
            Err(code) => Err(vec![FormaIssue {
                path: path.clone(),
                code,
                message: "value is not a boolean".into(),
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
impl crate::schema::sealed::Sealed for BoolSchema {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IssueCode;

    #[test]
    fn sc6_equals_accepts_matching_value() {
        let s = bool().equals(true);
        assert_eq!(s.parse(&true), Ok(true));
    }

    #[test]
    fn sc6_clone_preserves_refinements() {
        let original = bool().refine(|b| *b);
        let cloned = original.clone();
        assert!(
            cloned.parse(&false).is_err(),
            "clone must reject what the original rejects"
        );
        assert!(cloned.parse(&true).is_ok());
    }

    #[test]
    fn sc6_equals_rejects_with_exactly_one_issue() {
        let s = bool().equals(false);
        let err = s.parse(&true).unwrap_err();
        assert_eq!(err.issues.len(), 1);
        assert_eq!(err.issues[0].code, IssueCode::BoolEquals);
        assert_eq!(err.issues[0].params[0].1, ParamValue::Bool(false));
    }
}
