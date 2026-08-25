use std::sync::{Arc, OnceLock};

use crate::error::{
    FieldPath, FormaError, FormaIssue, IssueCode, IssueParams, ParamKey, ParamValue, Sink,
};
use crate::rule::{ClosureRule, Rule};
use crate::schema::{ConstraintDesc, DynSchema, FieldMeta, Schema, ShapeKind, ShapeNode};
use crate::value::Value;

/// Builtin string constraints, stored in declaration order (the single IR).
#[derive(Clone, Debug, PartialEq)]
pub enum StrCheck {
    /// Minimum length (inclusive).
    Min(usize),
    /// Maximum length (inclusive).
    Max(usize),
    /// Exact length.
    Length(usize),
    /// Rejects empty strings.
    NonEmpty,
    /// Pragmatic email format check.
    Email,
    /// Pragmatic URL format check.
    Url,
    /// Pragmatic UUID format check.
    Uuid,
}

/// String builder schema: one `Vec<StrCheck>` IR + boxed rules drive the typed
/// parse, the erased view and `shape()` (single representation, three projections).
pub struct StringSchema {
    checks: Vec<StrCheck>,
    rules: Vec<Arc<dyn Rule<str>>>,
    meta: FieldMeta,
    fail_fast: bool,
    trim: bool,
    shape_cache: OnceLock<ShapeNode>,
}

impl Clone for StringSchema {
    fn clone(&self) -> Self {
        Self {
            checks: self.checks.clone(),
            rules: self.rules.clone(),
            meta: self.meta.clone(),
            fail_fast: self.fail_fast,
            trim: self.trim,
            shape_cache: OnceLock::new(),
        }
    }
}

/// Structural debug output.
impl std::fmt::Debug for StringSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StringSchema")
            .field("checks", &self.checks)
            .field("rules", &self.rules)
            .field("fail_fast", &self.fail_fast)
            .field("trim", &self.trim)
            .finish_non_exhaustive()
    }
}

/// Creates an empty schema via [`string()`].
impl Default for StringSchema {
    fn default() -> Self {
        Self {
            checks: Vec::new(),
            rules: Vec::new(),
            meta: FieldMeta::default(),
            fail_fast: false,
            trim: false,
            shape_cache: OnceLock::new(),
        }
    }
}

impl StringSchema {
    /// Requires at least `n` characters.
    #[must_use]
    pub fn min(mut self, n: usize) -> Self {
        self.checks.push(StrCheck::Min(n));
        self
    }

    /// Requires at most `n` characters.
    #[must_use]
    pub fn max(mut self, n: usize) -> Self {
        self.checks.push(StrCheck::Max(n));
        self
    }

    /// Requires exactly `n` characters.
    #[must_use]
    pub fn length(mut self, n: usize) -> Self {
        self.checks.push(StrCheck::Length(n));
        self
    }

    /// Rejects empty strings after normalization.
    #[must_use]
    pub fn nonempty(mut self) -> Self {
        self.checks.push(StrCheck::NonEmpty);
        self
    }

    /// Requires a plausible email address (pragmatic v0 validator).
    #[must_use]
    pub fn email(mut self) -> Self {
        self.checks.push(StrCheck::Email);
        self
    }

    /// Requires a plausible URL (pragmatic v0 validator).
    #[must_use]
    pub fn url(mut self) -> Self {
        self.checks.push(StrCheck::Url);
        self
    }

    /// Requires a plausible UUID (pragmatic v0 validator).
    #[must_use]
    pub fn uuid(mut self) -> Self {
        self.checks.push(StrCheck::Uuid);
        self
    }

    /// Normalization flag: the trimmed view is validated regardless of where
    /// this appears in the chain (`max(3).trim()` ≡ `trim().max(3)`).
    #[must_use]
    pub fn trim(mut self) -> Self {
        self.trim = true;
        self
    }

    /// Appends a refinement closure; runs strictly after builtin checks pass (RF-1).
    #[must_use]
    pub fn refine<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
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
        R: Rule<str> + 'static,
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

    fn bridge(v: &Value) -> Result<&str, IssueCode> {
        v.as_str().ok_or(IssueCode::TypeMismatch)
    }

    fn run_checks(&self, raw: &str, path: &FieldPath) -> Vec<FormaIssue> {
        self.run_checks_with(raw, path, self.fail_fast)
    }

    /// Path/fail-fast-parameterized kernel: object schemas call this with a
    /// joined path and an inherited fail-fast flag (D2).
    fn run_checks_with(&self, raw: &str, path: &FieldPath, fail_fast: bool) -> Vec<FormaIssue> {
        let value = if self.trim { raw.trim() } else { raw };
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
                if let Some(rejection) = rule.validate(value) {
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

fn params_of(pairs: Vec<(&str, ParamValue)>) -> IssueParams {
    pairs
        .into_iter()
        .map(|(k, v)| (ParamKey::from(k), v))
        .collect()
}

impl Schema for StringSchema {
    type Input = String;
    type Output = String;

    fn parse(&self, input: &Self::Input) -> Result<Self::Output, FormaError> {
        let issues = self.run_checks(input.as_str(), &FieldPath::ROOT);
        if issues.is_empty() {
            Ok(input.clone())
        } else {
            Err(FormaError { issues })
        }
    }
}

impl DynSchema for StringSchema {
    fn validate_value(&self, v: &Value) -> Vec<FormaIssue> {
        match Self::bridge(v) {
            Ok(s) => self.run_checks(s, &FieldPath::ROOT),
            Err(code) => vec![crate::schema::issue_at_root(
                code,
                "value is not a string".into(),
                Vec::new(),
            )],
        }
    }

    fn shape(&self) -> &ShapeNode {
        self.shape_cache.get_or_init(|| ShapeNode {
            kind: ShapeKind::Str,
            constraints: self
                .checks
                .iter()
                .map(|c| ConstraintDesc {
                    code: code_of(c),
                    params: params_of_check(c),
                })
                .collect(),
        })
    }

    fn metadata(&self) -> &FieldMeta {
        &self.meta
    }
}

/// Lossless `usize` to parameter conversion; saturates at [`i64::MAX`] (unreachable
/// for real string lengths on supported targets).
fn usize_to_param_i64(n: usize) -> ParamValue {
    ParamValue::I64(i64::try_from(n).unwrap_or(i64::MAX))
}

fn evaluate_check(check: &StrCheck, value: &str) -> Option<FormaIssue> {
    let (code, message, params) = match check {
        StrCheck::Min(n) => {
            if value.chars().count() >= *n {
                return None;
            }
            (
                IssueCode::Min,
                "string is shorter than the minimum length",
                vec![("min", usize_to_param_i64(*n))],
            )
        }
        StrCheck::Max(n) => {
            if value.chars().count() <= *n {
                return None;
            }
            (
                IssueCode::Max,
                "string is longer than the maximum length",
                vec![("max", usize_to_param_i64(*n))],
            )
        }
        StrCheck::Length(n) => {
            if value.chars().count() == *n {
                return None;
            }
            (
                IssueCode::Length,
                "string length does not match",
                vec![("expected", usize_to_param_i64(*n))],
            )
        }
        StrCheck::NonEmpty => {
            if !value.is_empty() {
                return None;
            }
            (IssueCode::Empty, "string must not be empty", Vec::new())
        }
        StrCheck::Email => {
            if crate::validators::is_plausible_email(value) {
                return None;
            }
            (
                IssueCode::Email,
                "not a plausible email address",
                Vec::new(),
            )
        }
        StrCheck::Url => {
            if crate::validators::is_plausible_url(value) {
                return None;
            }
            (IssueCode::Url, "not a plausible URL", Vec::new())
        }
        StrCheck::Uuid => {
            if crate::validators::is_plausible_uuid(value) {
                return None;
            }
            (IssueCode::Uuid, "not a plausible UUID", Vec::new())
        }
    };
    Some(crate::schema::issue_at_root(
        code,
        message.into(),
        params_of(params),
    ))
}

fn code_of(c: &StrCheck) -> IssueCode {
    match c {
        StrCheck::Min(_) => IssueCode::Min,
        StrCheck::Max(_) => IssueCode::Max,
        StrCheck::Length(_) => IssueCode::Length,
        StrCheck::NonEmpty => IssueCode::Empty,
        StrCheck::Email => IssueCode::Email,
        StrCheck::Url => IssueCode::Url,
        StrCheck::Uuid => IssueCode::Uuid,
    }
}

fn params_of_check(c: &StrCheck) -> IssueParams {
    match c {
        StrCheck::Min(n) => params_of(vec![("min", usize_to_param_i64(*n))]),
        StrCheck::Max(n) => params_of(vec![("max", usize_to_param_i64(*n))]),
        StrCheck::Length(n) => params_of(vec![("expected", usize_to_param_i64(*n))]),
        _ => Vec::new(),
    }
}

/// Creates a new empty [`StringSchema`].
#[must_use]
pub fn string() -> StringSchema {
    StringSchema::default()
}

impl crate::schema::ObjectChild for StringSchema {
    fn validate_at(
        &self,
        v: &Value,
        path: &FieldPath,
        fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>> {
        match Self::bridge(v) {
            Ok(s) => {
                let issues = self.run_checks_with(s, path, fail_fast);
                if issues.is_empty() {
                    Ok(Value::from(s))
                } else {
                    Err(issues)
                }
            }
            Err(code) => Err(vec![FormaIssue {
                path: path.clone(),
                code,
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
impl crate::schema::sealed::Sealed for StringSchema {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IssueCode;

    fn codes(s: &StringSchema, input: &str) -> Vec<IssueCode> {
        s.run_checks(input, &FieldPath::ROOT)
            .iter()
            .map(|i| i.code.clone())
            .collect()
    }

    #[test]
    fn sc3_trim_then_max_on_trimmed_input() {
        let s = string().trim().max(3);
        assert!(
            codes(&s, " ab ").is_empty(),
            "trimmed value 'ab' has length 2"
        );
        assert_eq!(codes(&s, " abcd "), vec![IssueCode::Max]);
    }

    #[test]
    fn sc3_max_then_trim_is_equivalent() {
        let a = string().trim().max(3);
        let b = string().max(3).trim();
        assert_eq!(codes(&a, " abcd "), codes(&b, " abcd "));
    }

    #[test]
    fn sc3_min_rejects_short() {
        assert_eq!(codes(&string().min(3), "ab"), vec![IssueCode::Min]);
    }

    #[test]
    fn sc3_length_exact_match() {
        assert!(codes(&string().length(2), "ab").is_empty());
        assert_eq!(codes(&string().length(3), "ab"), vec![IssueCode::Length]);
    }

    #[test]
    fn sc3_min_max_count_chars_not_bytes() {
        // "héllo" is 5 chars but 6 bytes; char counting is the pinned contract.
        assert!(codes(&string().min(5), "h\u{e9}llo").is_empty());
        assert!(codes(&string().max(5), "h\u{e9}llo").is_empty());
        assert!(codes(&string().length(5), "h\u{e9}llo").is_empty());
        assert_eq!(codes(&string().max(4), "h\u{e9}llo"), vec![IssueCode::Max]);
    }

    #[test]
    fn sc3_nonempty_rejects_empty() {
        assert_eq!(codes(&string().nonempty(), ""), vec![IssueCode::Empty]);
    }

    #[test]
    fn rf1_refine_skipped_when_earlier_fails() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let s = string().min(5).refine(|s| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            s.contains('x')
        });
        let codes: Vec<_> = codes(&s, "ab");
        assert_eq!(codes, vec![IssueCode::Min]);
        assert_eq!(
            CALLS.load(Ordering::SeqCst),
            0,
            "refine must never be invoked"
        );
    }

    #[test]
    fn rf1_refine_runs_once_after_constraints_pass() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let s = string().min(2).refine(|_s| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            true
        });
        assert!(codes(&s, "abc").is_empty());
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rf1_clone_preserves_refinements() {
        let original = string().refine(|s| s.contains('x'));
        let cloned = original.clone();
        assert!(
            codes(&cloned, "ab").contains(&IssueCode::Refine),
            "clone must reject what the original rejects"
        );
        assert!(codes(&cloned, "ax").is_empty());
    }

    #[test]
    fn rf2_two_failing_refines_accumulate_in_order() {
        let s = string().refine(|_| false).refine(|_| false);
        let issues = s.run_checks("q", &FieldPath::ROOT);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].code, IssueCode::Refine);
        assert_eq!(issues[1].code, IssueCode::Refine);
        assert_eq!(issues[0].message, issues[1].message);
    }

    #[test]
    fn rf2_refines_run_after_builtins_not_before() {
        let s = string().min(10).email();
        let codes: Vec<_> = codes(&s, "ab");
        assert_eq!(
            codes.len(),
            2,
            "both builtin issues collected before refines would run"
        );
    }
}
