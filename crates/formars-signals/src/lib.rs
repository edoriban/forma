//! `formars-signals`: headless reactive form controller built directly on
//! `reactive_graph`.
//!
//! [`FormController`] owns an insertion-ordered registry of fields — one
//! `ArcRwSignal<Value>` plus touched/dirty cells per field — and derives every
//! other quantity as a pure `ArcMemo`. The crate is UI-agnostic: no Leptos,
//! no effects, no spawner configuration. All public reactive types are from
//! the Arc family (`Send + Sync`), safe for headless tests and SSR.
//!
//! # Disposed-owner hazard: accessors misreport
//!
//! `reactive_graph` accessors MISREPORT rather than error on disposed owners:
//! a `get()`/`set()` on a signal whose owner was dropped either panics with a
//! misleading "already been disposed" message or silently treats a transient
//! lock failure the same way. This crate treats any `None` from such an
//! accessor as "not usable" and degrades defensively — see `begin_attempt`
//! in `formars-ui`, which treats a `try_maybe_update` `None` as not-acquired.
//! Guidance: NEVER rely on `get()` after owner disposal; drop the handle
//! instead.
//!
//! Async is confined to the composed future returned by
//! [`FormController::on_submit`]; the consuming layer owns scheduling.
//!
//! # End-to-end example
//!
//! ```
//! use formars_core::prelude::*;
//! use formars_signals::{FormController, SubmitError, ValidateOn};
//! use futures::executor::block_on;
//! use reactive_graph::traits::{Get, Set};
//!
//! // Build a form with a controller-default timing mode.
//! let mut c = FormController::new(ValidateOn::Blur);
//! let email = c
//!     .register_with(
//!         FieldPath::key("email"),
//!         Box::new(string().min(8).email()),
//!         ValidateOn::Change,
//!     )
//!     .expect("fresh registration");
//!
//! // Edits flow through the handle; derived memos track synchronously.
//! email.value().set(Value::from("user@example.com"));
//! assert!(email.errors().get().is_empty());
//! assert!(c.validate().is_ok());
//!
//! // Submit is one composed future: sync gate, snapshot, is_submitting bracket.
//! let outcome = block_on(c.on_submit(|snapshot| async move {
//!     let payload = format!("{:?}", snapshot.get(&FieldPath::key("email")));
//!     Ok::<String, std::convert::Infallible>(payload)
//! }));
//! assert!(outcome.is_ok());
//!
//! // Server errors merge back onto their addressed fields.
//! c.apply_server_errors(&FormaError {
//!     issues: vec![formars_core::error::FormaIssue {
//!         path: FieldPath::key("ghost"),
//!         code: IssueCode::Refine,
//!         message: "unknown field".into(),
//!         params: Vec::new(),
//!     }],
//! });
//! assert!(c.form_errors().get().iter().any(|i| i.path.to_string() == "ghost"));
//! ```

mod controller;
mod field;
mod submit;
mod validation;

pub use controller::{FormController, RegisterError};
pub use field::{FieldHandle, ValidateOn};
pub use submit::{FormSnapshot, SubmitError};

pub use formars_core::error::{FieldPath, FormaError, FormaIssue, IssueCode};
pub use formars_core::value::Value;
