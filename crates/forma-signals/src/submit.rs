//! The submit boundary: point-in-time snapshots, sync validation gating,
//! and the composed handler future.

use forma_core::error::{FieldPath, FormaError};
use forma_core::value::Value;

/// Point-in-time copy of every field value, taken at the submit gate in
/// registry declaration order. Plain owned data — later edits cannot mutate
/// it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormSnapshot {
    pub(crate) entries: Vec<(FieldPath, Value)>,
}

impl FormSnapshot {
    /// The captured value for `path`, if the field was registered.
    #[must_use]
    pub fn get(&self, path: &FieldPath) -> Option<&Value> {
        self.entries.iter().find(|(p, _)| p == path).map(|(_, v)| v)
    }

    /// Iterates `(path, value)` pairs in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&FieldPath, &Value)> {
        self.entries.iter().map(|(p, v)| (p, v))
    }

    /// Number of captured fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no fields were registered at capture time.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Why a composed submit future failed.
///
/// `Validation` means the synchronous pre-handler gate rejected the form and
/// the user handler was never constructed; `Handler` carries the handler's
/// own error after it ran to completion.
#[derive(Debug)]
pub enum SubmitError<E> {
    /// Whole-form validation failed before the handler was invoked.
    Validation(FormaError),
    /// The handler ran and resolved with an error.
    Handler(E),
}

impl<E: std::fmt::Debug> std::fmt::Display for SubmitError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(err) => write!(f, "validation failed: {err}"),
            Self::Handler(e) => write!(f, "submit handler failed: {e:?}"),
        }
    }
}

impl<E: std::fmt::Debug> std::error::Error for SubmitError<E> {}
