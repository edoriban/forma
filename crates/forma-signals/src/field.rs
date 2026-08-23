//! Per-field state cells, handles, and validation-timing modes.

use std::sync::Arc;

use reactive_graph::computed::ArcMemo;
use reactive_graph::signal::ArcRwSignal;
use reactive_graph::traits::{Get, Set};

use forma_core::error::{FieldPath, FormaIssue};
use forma_core::schema::DynSchema;
use forma_core::value::Value;

/// When a field's validation issues become visible to the consuming UI.
///
/// The controller default is set at construction; individual fields override
/// it at registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidateOn {
    /// Errors track every edit; visibility follows the touched/submitted gate.
    Change,
    /// Errors become active once the field is marked touched (the blur seam).
    Blur,
    /// Error visibility activates only after a submit attempt.
    Submit,
}

/// Accessor view over one registered field's reactive cells.
#[derive(Clone)]
pub struct FieldHandle {
    pub(crate) path: FieldPath,
    pub(crate) value: ArcRwSignal<Value>,
    pub(crate) touched: ArcRwSignal<bool>,
    pub(crate) dirty: ArcMemo<bool>,
    pub(crate) errors: ArcMemo<Vec<FormaIssue>>,
    pub(crate) visible_errors: ArcMemo<Vec<FormaIssue>>,
    pub(crate) validate_on: ValidateOn,
}

impl FieldHandle {
    /// The field's registration path.
    #[must_use]
    pub fn path(&self) -> &FieldPath {
        &self.path
    }

    /// The field's value signal — the single source of truth for this field.
    #[must_use]
    pub fn value(&self) -> ArcRwSignal<Value> {
        self.value.clone()
    }

    /// Whether the field has been marked touched (blur seam).
    #[must_use]
    pub fn touched(&self) -> ArcRwSignal<bool> {
        self.touched.clone()
    }

    /// True when the current value differs from the register-time snapshot.
    #[must_use]
    pub fn dirty(&self) -> ArcMemo<bool> {
        self.dirty.clone()
    }

    /// Schema-derived issues for the current value, stamped with this path.
    #[must_use]
    pub fn errors(&self) -> ArcMemo<Vec<FormaIssue>> {
        self.errors.clone()
    }

    /// Display-gated issues: schema issues behind the mode-dependent gate plus
    /// live server issues for an unedited value.
    #[must_use]
    pub fn visible_errors(&self) -> ArcMemo<Vec<FormaIssue>> {
        self.visible_errors.clone()
    }

    /// The effective validation-timing mode for this field.
    #[must_use]
    pub fn validate_on(&self) -> ValidateOn {
        self.validate_on
    }

    /// Current value as an owned `String`, only for the `Value::String`
    /// variant.
    #[must_use]
    pub fn get_str(&self) -> Option<String> {
        self.value.get().as_str().map(str::to_owned)
    }

    /// Sets `Value::String(s)` exactly — no coercion.
    pub fn set_str(&self, s: &str) {
        self.value.set(Value::from(s));
    }

    /// Sets `Value::I64(v)` exactly.
    pub fn set_i64(&self, v: i64) {
        self.value.set(Value::I64(v));
    }

    /// Sets `Value::F64(v)` exactly.
    pub fn set_f64(&self, v: f64) {
        self.value.set(Value::F64(v));
    }

    /// Sets `Value::Bool(b)` exactly.
    pub fn set_bool(&self, b: bool) {
        self.value.set(Value::Bool(b));
    }
}

/// Controller-owned state for one registered field.
pub(crate) struct FieldCell {
    pub(crate) initial: Value,
    pub(crate) value: ArcRwSignal<Value>,
    pub(crate) touched: ArcRwSignal<bool>,
    pub(crate) schema: Arc<dyn DynSchema>,
    pub(crate) validate_on: ValidateOn,
    pub(crate) server: ArcRwSignal<Vec<FormaIssue>>,
    pub(crate) server_baseline: ArcRwSignal<Value>,
    pub(crate) visible_errors: ArcMemo<Vec<FormaIssue>>,
}
