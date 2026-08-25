//! The two validation views and their shared introspection types.

use std::borrow::Cow;
use std::fmt;

use crate::error::{FieldPath, FormaError, FormaIssue, IssueCode, IssueParams};
use crate::value::Value;

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a formars `Schema`",
    label = "expected a schema built from the `string()`, `number()`, `bool()` or `coerced()` builders"
)]
/// The typed validation view.
///
/// `Err` always carries at least one issue. v0 primitives set
/// `Input == Output`; [`crate::coerce::CoercedSchema`] is the first
/// transform-shaped citizen (`Input = String`, `Output = T`), and
/// [`crate::types::object::ObjectSchema`] makes the ordered [`crate::value::Object`]
/// the struct-shaped currency (`Input = Object`, `Output = Object`).
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
    /// Object schema; declared fields in declaration order.
    Object {
        /// Declared fields, derived from the same registry the kernel walks.
        fields: Vec<ObjectFieldDesc>,
    },
}

/// One declared object field as introspection data — key plus the child's
/// own shape projection, taken from the same `Vec` the kernel walks so
/// introspection cannot drift (DV-6).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectFieldDesc {
    /// Declared key (same `Box<str>` as the kernel's field registry).
    pub key: Box<str>,
    /// Child projection, produced by the child's own `shape()`.
    pub child: ShapeNode,
}

/// Sealing module: keeps the [`ObjectChild`] extension point crate-internal.
pub mod sealed {
    /// Prevents downstream implementations of [`super::ObjectChild`].
    pub trait Sealed {}
}

/// Contract for anything that can back an [`crate::types::object::ObjectSchema`]
/// field: path-aware validation with a fail-fast override, plus an
/// introspection projection (D3).
///
/// Public only so the `ObjectSchema::field` signature is nameable; sealed,
/// so only the builtin families implement it.
pub trait ObjectChild: fmt::Debug + Send + Sync + sealed::Sealed {
    /// Validates `v` addressed at `path` (already joined by the caller);
    /// on full success returns the validated output converted to `Value`.
    fn validate_at(
        &self,
        v: &Value,
        path: &FieldPath,
        fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>>;
    /// Introspection projection (delegates to the child's own `shape()`).
    fn shape_node(&self) -> ShapeNode;
    /// UI-facing metadata slots of the child (serves `ObjectSchema::field_meta`).
    fn meta(&self) -> &FieldMeta;
}

/// Backs an [`crate::types::object::ObjectSchema`] field slot with any schema
/// whose composed representation is an [`ObjectSchema`] (e.g. a derive
/// companion from the separate `formars-derive` crate).
///
/// Pure delegation (NE-2): joined paths, inherited fail-fast and introspection
/// come from the wrapped schema's own [`ObjectSchema::validate_at`] kernel —
/// never reimplemented. The only contract is [`AsRef<ObjectSchema>`], so the
/// adapter stays narrow while [`ObjectChild`] remains sealed.
#[derive(Debug)]
pub struct Nested<S> {
    inner: S,
}

impl<S> Nested<S> {
    /// Wraps a schema whose composed representation is an [`ObjectSchema`].
    #[must_use]
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> ObjectChild for Nested<S>
where
    S: fmt::Debug + Send + Sync + AsRef<crate::types::object::ObjectSchema>,
{
    fn validate_at(
        &self,
        v: &Value,
        path: &FieldPath,
        fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>> {
        self.inner.as_ref().validate_at(v, path, fail_fast)
    }

    fn shape_node(&self) -> ShapeNode {
        self.inner.as_ref().shape_node()
    }

    fn meta(&self) -> &FieldMeta {
        self.inner.as_ref().meta()
    }
}

// Explicit seal registration: `ObjectChild` carries the sealed supertrait, and
// this is the single new in-crate implementor alongside the builtin families.
impl<S> sealed::Sealed for Nested<S> {}

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
