//! Leptos 0.8 companion crate for `forma-signals`: the `use_form` hook,
//! the `<Form>` submit wrapper, and the `<TextField>` field binding.
//!
//! # Dependency note: effects availability (FU-DEP-3)
//!
//! Both `leptos` and `reactive_graph` are declared here with
//! `default-features = false`. In `reactive_graph` 0.2.x the effect module
//! (`reactive_graph::effect`) is UNCONDITIONAL — the optional `effects`
//! feature gates only debug diagnostics, so no `effects` feature edge
//! appears in `cargo tree -e features` at all. This is EXPECTED and
//! harmless: `forma-signals` remains intact because its code paths never
//! schedule effects (`FS-DEP-2` holds); only this crate's `<TextField>`
//! display effect uses the always-present module.
//!
//! # SSR submit exclusion (FU-FM-6)
//!
//! `<Form>` markup may render server-side, but submit execution is
//! client-path only in v0: [`leptos::task::spawn_local`] requires a
//! local-task executor unavailable on server threads, and the composed
//! submit future is deliberately NOT `Send`, so it can never be driven by
//! `tokio::spawn`.
//!
//! # Context resolution behavior (FU-HK-2)
//!
//! [`use_form`] provisions the controller as Leptos context.
//! [`use_form_controller()`] PANICS outside any `use_form` tree;
//! [`try_form_controller()`] is the graceful `Option`-returning variant.
//!
//! # IME / caret caveat (FU-TF-2)
//!
//! `<TextField>` pushes signal content back to the DOM only when it differs
//! from the current DOM value, which prevents caret jumps from same-value
//! writes and avoids fighting mid-composition IME state. Residual
//! composition edge cases vary by rendering engine; treat them as a known
//! limitation and verify manually in a browser.
//!
//! # No-provider blur behavior (L-1)
//!
//! When no ancestor called [`use_form`], `<TextField>`'s blur seam resolves
//! no controller and skips touch-marking: the component still renders and
//! binds values correctly.
//!
//! # Double-submit race shield (L-2)
//!
//! `<Form>` guards each attempt against an in-flight submit and renders its
//! built-in button reactively disabled while submitting. A pathological
//! rapid double-click that slips two attempts past the guard before
//! `is_submitting` flips cannot be fully excluded natively; the reactive
//! disabled state is the primary shield and this residual race is a known
//! v0 edge.

mod form;
mod text_field;
mod use_form;

pub use form::{Form, SubmitOutcome};
pub use use_form::{UseForm, try_form_controller, use_form, use_form_controller};

// One-import DX (FU-IN-1): everything a consumer needs from the base layer.
pub use forma_signals::{
    FieldHandle, FieldPath, FormController, FormSnapshot, FormaError, FormaIssue, IssueCode,
    RegisterError, SubmitError, ValidateOn, Value,
};

pub use text_field::TextField;
