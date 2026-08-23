//! The two validation views and their shared introspection types.

use std::borrow::Cow;

use crate::error::{FormaError, FormaIssue, IssueCode, IssueParams};
use crate::value::Value;

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a forma `Schema`",
    label = "expected a schema built from the `string()`, `number()`, `bool()` or `coerced()` builders"
)]
/// The typed validation view.
///
/// `Err` always carries at least one issue. v0 primitives set
/// `Input == Output`; [`crate::coerce::CoercedSchema`] is the first
/// transform-shaped citizen (`Input = String`, `Output = T`).
pub trait Schema {
    /// What the schema consumes.
    type Input;
    /// What the schema produces on success.
    type Output;

    /// Validates and parses `input`; all violated constraints accumulate into one error.
    fn parse(&self, input: &Self::Input) -> Result<Self::Output, FormaError>;
}

/// The object-safe erased view over a [`Value`] tree (DV-2).
///
/// Both views execute from the same internal representation, so results always
/// agree with the typed path (SC-9). Usable as `Box<dyn DynSchema>`.
///
/// The `Send + Sync` supertraits make erased schemas thread-safe and safe to
/// hold in reactive/memo contexts (`Arc<Box<dyn DynSchema>>` captured by
/// shared closures), so introspection can be cached across threads.
pub trait DynSchema: Send + Sync {
    /// Validates an erased value; issues mirror what the typed parse would report.
    fn validate_value(&self, v: &Value) -> Vec<FormaIssue>;
    /// Introspection node describing this schema's kind and constraints,
    /// memoized in a per-instance cache on first call.
    fn shape(&self) -> &ShapeNode;
    /// UI-facing metadata slots attached to this schema.
    fn metadata(&self) -> &FieldMeta;
}

/// Introspection projection of a schema: primitive kind plus declared constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeNode {
    /// Which primitive family this schema validates.
    pub kind: ShapeKind,
    /// Declared constraints in declaration order.
    pub constraints: Vec<ConstraintDesc>,
}

/// Primitive families distinguishable through erasure.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeKind {
    /// String schema.
    Str,
    /// Number schema; `integer` marks i64-family schemas.
    Number {
        /// True when the schema is over i64.
        integer: bool,
    },
    /// Boolean schema.
    Bool,
    /// Coercing string-to-type schema.
    Coerced,
}

/// One declared constraint as data — derived from the same check vector the
/// kernels run, so introspection cannot drift from behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintDesc {
    /// Stable constraint identifier.
    pub code: IssueCode,
    /// Constraint parameters (e.g. the declared minimum).
    pub params: IssueParams,
}

/// UI-facing metadata slots (DV-3); unknown extras survive round-trips.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldMeta {
    /// Short display label.
    pub label: Option<Cow<'static, str>>,
    /// Longer description.
    pub description: Option<Cow<'static, str>>,
    /// Input placeholder text.
    pub placeholder: Option<Cow<'static, str>>,
    /// Extensible key-value extras for UI layers.
    pub extra: Vec<(Cow<'static, str>, Value)>,
}

/// Builds an issue addressed to ROOT with the given code, message and params.
pub(crate) fn issue_at_root(
    code: IssueCode,
    message: Cow<'static, str>,
    params: IssueParams,
) -> FormaIssue {
    FormaIssue {
        path: crate::error::FieldPath::ROOT,
        code,
        message,
        params,
    }
}
