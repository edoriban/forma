//! Accumulated, path-addressed validation errors.
//!
//! [`FormaError`] carries every violated constraint from one parse call as an
//! ordered vector of [`FormaIssue`]s; each issue is addressed by a
//! [`FieldPath`], tagged with a stable [`IssueCode`], and parameterized for
//! deterministic display.
//!
//! [`FormaError`]: crate::error::FormaError
//! [`FormaIssue`]: crate::error::FormaIssue
//! [`FieldPath`]: crate::error::FieldPath
//! [`IssueCode`]: crate::error::IssueCode

use std::borrow::Cow;
use std::fmt;

/// Address of the field a [`FormaIssue`] refers to.
///
/// Top-level primitives report [`FieldPath::ROOT`]; object schemas join
/// field segments via [`FieldPath::join`], so nested issues render as
/// dotted paths (`user.email`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldPath {
    segments: Vec<Segment>,
}

/// One step in a [`FieldPath`]: either an object key or an array index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Segment {
    /// Object key segment (e.g. `user`).
    Key(Box<str>),
    /// Array index segment rendered as `[2]`.
    Index(usize),
}

impl FieldPath {
    /// The empty path — address of a top-level primitive value.
    pub const ROOT: Self = Self {
        segments: Vec::new(),
    };

    /// Creates a path addressing a single top-level key.
    #[must_use]
    pub fn key(name: &str) -> Self {
        Self {
            segments: vec![Segment::Key(name.into())],
        }
    }

    /// Creates a path addressing a single top-level index.
    #[must_use]
    pub fn index(i: usize) -> Self {
        Self {
            segments: vec![Segment::Index(i)],
        }
    }

    /// Returns a new path extended with one more segment; the receiver is untouched.
    #[must_use]
    pub fn join(&self, seg: Segment) -> Self {
        let mut segments = self.segments.clone();
        segments.push(seg);
        Self { segments }
    }
}

/// A key renders raw iff it is non-empty and contains none of the trigger
/// characters (`.`, `[`, `]`, `` ` ``) and no control character; otherwise
/// it renders backtick-wrapped with an unambiguous, prefix-free escape
/// grammar: embedded backticks doubled, backslashes doubled, every control
/// character written `\u{XX}` (lowercase hex, minimum two digits). Raw and
/// quoted renderings are PREFIX-DISJOINT (backtick is always a trigger), so
/// the mapping stays injective over structurally distinct keys.
fn write_key(out: &mut String, k: &str) {
    use std::fmt::Write as _;
    if k.is_empty() || k.contains(['.', '[', ']', '`']) || k.chars().any(char::is_control) {
        out.push('`');
        for ch in k.chars() {
            match ch {
                '`' => out.push_str("``"),
                '\\' => out.push_str("\\\\"),
                // Infallible on `String`; uniform lowercase `\u{XX}`, min 2 digits.
                c if c.is_control() => {
                    let _ = write!(out, "\\u{{{:02x}}}", c as u32);
                }
                c => out.push(c),
            }
        }
        out.push('`');
    } else {
        out.push_str(k);
    }
}

impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        for seg in &self.segments {
            match seg {
                Segment::Key(k) => {
                    if !out.is_empty() {
                        out.push('.');
                    }
                    write_key(&mut out, k);
                }
                Segment::Index(i) => {
                    out.push('[');
                    out.push_str(&i.to_string());
                    out.push(']');
                }
            }
        }
        f.write_str(&out)
    }
}

/// Stable, enumerable identifiers for every builtin constraint (ER-3), plus
/// [`IssueCode::Custom`] for consumer-defined ones.
///
/// Matchable today; additive variants in minor releases until v1 freezes it,
/// so downstream matches should include a wildcard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueCode {
    /// Value is shorter than the declared minimum length.
    Min,
    /// Value exceeds the declared maximum length/bound.
    Max,
    /// Length does not equal the declared exact length.
    Length,
    /// String does not look like an email address.
    Email,
    /// String does not look like a URL.
    Url,
    /// String does not look like a UUID.
    Uuid,
    /// String is empty where emptiness is forbidden.
    Empty,
    /// Number has a fractional part where an integer is required.
    Int,
    /// Number is zero or negative where positivity is required.
    Positive,
    /// Float is NaN or infinite where finiteness is required.
    Finite,
    /// Boolean does not equal the expected value.
    BoolEquals,
    /// String-to-type coercion failed.
    Coerce,
    /// A refinement closure rejected the value.
    Refine,
    /// Erased-view value has the wrong variant for this schema.
    TypeMismatch,
    /// Declared object field is absent from the input object (purely
    /// structural: decided by key existence, never by the value found).
    Required,
    /// Consumer-defined constraint identifier, carried verbatim from a
    /// [`RefineRejection`](crate::rule::RefineRejection).
    ///
    /// Builtins never produce this variant; it exists so a consumer running
    /// several custom rules can tell them apart. [`IssueCode::Refine`]
    /// collapses every rule into one code, which defeats keying a
    /// translation table off [`IssueCode`]. The payload is an opaque
    /// consumer namespace: formars neither interprets nor validates it.
    Custom(Box<str>),
}

/// Key of one entry in [`IssueParams`].
pub type ParamKey = Box<str>;

/// Scalar parameter values carried by issues — dependency-free and ordered.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// Signed integer parameter.
    I64(i64),
    /// Float parameter.
    F64(f64),
    /// Boolean parameter.
    Bool(bool),
    /// String parameter.
    Str(Box<str>),
}

/// Ordered `(key, value)` pairs attached to an issue for deterministic display.
pub type IssueParams = Vec<(ParamKey, ParamValue)>;

/// One violated constraint: where ([`FieldPath`]), what ([`IssueCode`]),
/// human-readable message, and structured parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct FormaIssue {
    /// Address of the field under validation (ROOT for top-level primitives;
    /// dot-joined key segments once validation descends into objects).
    pub path: FieldPath,
    /// Stable constraint identifier.
    pub code: IssueCode,
    /// Static message for builtins; dynamic allowed for custom rules.
    pub message: Cow<'static, str>,
    /// Ordered parameters (e.g. `min = 3`).
    pub params: IssueParams,
}

/// Every constraint violated during one parse call, in declaration order (ER-1).
///
/// `parse()`/`validate_value()` always return at least one issue on failure,
/// but the field is public so no non-empty guarantee is claimed at the type
/// level — hence `first()` returns an [`Option`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormaError {
    /// All issues collected before the error was returned, in declaration order.
    pub issues: Vec<FormaIssue>,
}

impl FormaError {
    /// The earliest-collected issue, or `None` for a hand-constructed empty error.
    #[must_use]
    pub fn first(&self) -> Option<&FormaIssue> {
        self.issues.first()
    }

    /// Iterates only the issues addressed to `path`.
    pub fn issues_for<'a>(&'a self, path: &'a FieldPath) -> impl Iterator<Item = &'a FormaIssue> {
        self.issues.iter().filter(move |i| &i.path == path)
    }
}

impl fmt::Display for FormaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.issues.first() {
            Some(first) => {
                write!(
                    f,
                    "{} validation issue(s); first: {}",
                    self.issues.len(),
                    first.message
                )
            }
            None => f.write_str("0 validation issues"),
        }
    }
}

impl std::error::Error for FormaError {}

/// Internal accumulator consulted by validation kernels.
pub(crate) struct Sink<'p> {
    path: &'p FieldPath,
    fail_fast: bool,
    /// Collected issues so far.
    pub issues: Vec<FormaIssue>,
}

impl<'p> Sink<'p> {
    pub(crate) fn new(path: &'p FieldPath, fail_fast: bool) -> Self {
        Self {
            path,
            fail_fast,
            issues: Vec::new(),
        }
    }

    /// Appends the issue (stamping it with the sink's path); returns `false`
    /// when a fail-fast stop is signaled — kernels break their loop on `false`,
    /// which is what guarantees exactly one issue under `.fail_fast()`.
    pub(crate) fn push(&mut self, mut issue: FormaIssue) -> bool {
        issue.path = self.path.clone();
        self.issues.push(issue);
        !self.fail_fast
    }

    /// Consumes the sink into the final error.
    pub(crate) fn finish(self) -> FormaError {
        FormaError {
            issues: self.issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldPath, FormaError, FormaIssue, IssueCode, ParamValue, Segment, Sink};

    fn issue(path: FieldPath, code: IssueCode, msg: &'static str) -> FormaIssue {
        FormaIssue {
            path,
            code,
            message: msg.into(),
            params: Vec::new(),
        }
    }

    #[test]
    fn display_root_is_empty() {
        assert_eq!(FieldPath::ROOT.to_string(), "");
    }

    #[test]
    fn display_single_key() {
        assert_eq!(FieldPath::key("email").to_string(), "email");
    }

    #[test]
    fn display_nested_keys() {
        let p = FieldPath::key("user").join(Segment::Key("email".into()));
        assert_eq!(p.to_string(), "user.email");
    }

    #[test]
    fn display_mixed_key_index() {
        let p = FieldPath::key("items")
            .join(Segment::Index(2))
            .join(Segment::Key("qty".into()));
        assert_eq!(p.to_string(), "items[2].qty");
    }

    #[test]
    fn join_does_not_mutate_receiver() {
        let base = FieldPath::key("user");
        let child = base.join(Segment::Key("email".into()));
        assert_eq!(base.to_string(), "user");
        assert_eq!(child.to_string(), "user.email");
    }

    #[test]
    fn paths_with_equal_segments_are_equal() {
        assert_eq!(FieldPath::key("a"), FieldPath::key("a"));
        assert_ne!(FieldPath::key("a"), FieldPath::index(0));
    }

    #[test]
    fn er2_first_returns_earliest() {
        let e = FormaError {
            issues: vec![
                issue(FieldPath::ROOT, IssueCode::Min, "min"),
                issue(FieldPath::ROOT, IssueCode::Email, "email"),
            ],
        };
        assert_eq!(e.first().unwrap().code, IssueCode::Min);
        assert_eq!(e.first().unwrap().message, "min");
    }

    #[test]
    fn er2_issues_for_path_lookup() {
        let email_path = FieldPath::key("email");
        let e = FormaError {
            issues: vec![
                issue(email_path.clone(), IssueCode::Email, "bad email"),
                issue(FieldPath::key("age"), IssueCode::Min, "too young"),
                issue(email_path.clone(), IssueCode::Max, "long"),
            ],
        };
        let got: Vec<_> = e.issues_for(&email_path).map(|i| i.code.clone()).collect();
        assert_eq!(got, vec![IssueCode::Email, IssueCode::Max]);
    }

    #[test]
    fn first_is_none_for_empty_error() {
        assert!(FormaError { issues: Vec::new() }.first().is_none());
    }

    #[test]
    fn error_display_mentions_count_and_first_message() {
        let e = FormaError {
            issues: vec![issue(FieldPath::ROOT, IssueCode::Min, "too short")],
        };
        let d = e.to_string();
        assert!(d.contains('1'), "count missing: {d}");
        assert!(d.contains("too short"), "first message missing: {d}");
    }

    #[test]
    fn error_display_none_safe_when_empty() {
        let d = FormaError { issues: Vec::new() }.to_string();
        assert!(!d.is_empty());
    }

    #[test]
    fn issue_params_preserve_order_and_values() {
        let i = FormaIssue {
            path: FieldPath::ROOT,
            code: IssueCode::Min,
            message: "min".into(),
            params: vec![
                ("min".into(), ParamValue::I64(3)),
                ("got".into(), ParamValue::Str("a".into())),
            ],
        };
        assert_eq!(i.params[0].0.as_ref(), "min");
        assert_eq!(i.params[1].1, ParamValue::Str("a".into()));
    }

    #[test]
    fn issue_code_is_matchable_and_comparable() {
        let c = IssueCode::Coerce;
        assert_eq!(c, IssueCode::Coerce);
        assert_ne!(c, IssueCode::Refine);
        let name = match c {
            IssueCode::Coerce => "coerce",
            _ => "other",
        };
        assert_eq!(name, "coerce");
    }

    #[test]
    fn sink_accumulate_collects_all() {
        let root = FieldPath::ROOT;
        let mut s = Sink::new(&root, false);
        assert!(s.push(issue(FieldPath::ROOT, IssueCode::Min, "a")));
        assert!(s.push(issue(FieldPath::ROOT, IssueCode::Email, "b")));
        assert_eq!(s.issues.len(), 2);
        assert_eq!(s.finish().issues.len(), 2);
    }

    #[test]
    fn er6_required_code_is_matchable_and_displays_sensibly() {
        let c = IssueCode::Required;
        assert_eq!(c, IssueCode::Required);
        assert_ne!(c, IssueCode::Min);
        let name = match c {
            IssueCode::Required => "required",
            _ => "other",
        };
        assert_eq!(name, "required");
    }

    #[test]
    fn er6_required_carries_joined_path_in_issue() {
        let p = FieldPath::key("email");
        let i = issue(p.clone(), IssueCode::Required, "missing");
        assert_eq!(i.path, p);
        assert_eq!(i.path.to_string(), "email");
    }

    #[test]
    fn custom_codes_stay_distinguishable_from_each_other_and_from_refine() {
        let decimal = IssueCode::Custom("decimal".into());
        let currency = IssueCode::Custom("currency".into());
        assert_ne!(decimal, currency);
        assert_ne!(decimal, IssueCode::Refine);
        let key = match &decimal {
            IssueCode::Custom(name) => format!("validation.{name}"),
            _ => "validation.generic".to_owned(),
        };
        assert_eq!(key, "validation.decimal");
    }

    #[test]
    fn sink_fail_fast_signals_stop_after_first_push() {
        let root = FieldPath::ROOT;
        let mut s = Sink::new(&root, true);
        assert!(!s.push(issue(FieldPath::ROOT, IssueCode::Min, "a")));
        assert_eq!(s.finish().issues.len(), 1);
    }
}
