//! Curated re-exports: one import line per adoption tier.
//!
//! Each name is sourced from its semantic owner layer (core types from
//! `formars_core`, controller vocabulary from `formars_signals`,
//! component/hook vocabulary from `formars_ui`) — never the reverse.

pub use formars_core::prelude::*;

/// Headless reactive controller vocabulary. Available on crate feature
/// `signals` only.
#[cfg(feature = "signals")]
pub use formars_signals::{
    FieldHandle, FormController, FormSnapshot, RegisterError, SubmitError, ValidateOn,
};

/// Reactive read/write traits (`reactive_graph`). Available on crate feature
/// `signals` only.
#[cfg(feature = "signals")]
pub use reactive_graph::traits::{Get, Read, Set};

/// Leptos component/hook vocabulary. Available on crate feature `ui` only.
#[cfg(feature = "ui")]
pub use formars_ui::{
    Form, SubmitOutcome, TextField, UseForm, try_form_controller, use_form, use_form_controller,
};
