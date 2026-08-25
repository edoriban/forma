//! # formars — one dependency, one import, pay per tier.
//!
//! `formars` is the umbrella entry point to the toolkit: schema validation
//! for Rust with builder-first, macro-free composition, plus opt-in layers
//! for reactive headless controllers, Leptos UI components, and a derive
//! macro. Everything here is a curated re-export of an owning member crate —
//! nothing is invented at this layer.
//!
//! ## Feature matrix
//!
//! | Tier                | Feature                 | Adds to your graph                          | Where to look next     |
//! |---------------------|-------------------------|---------------------------------------------|------------------------|
//! | Validate-only       | *(default)*             | nothing beyond `formars-core` (zero deps)   | [`formars_core`]       |
//! | Headless controller | `signals`               | `reactive_graph` 0.2.x                      | `formars-signals`      |
//! | Leptos UI           | `ui` (implies `signals`) | `leptos` 0.8.x + one shared `reactive_graph` copy | `formars-ui`     |
//! | Derive macro        | `derive`                | syn/quote/proc-macro2 (host-side only)      | `formars-derive`       |
//!
//! ## Tier 1 — validate-only (default)
//!
//! No feature flag, no extra dependencies: schemas are values you compose,
//! parse, and inspect. This tier is fully visible on this page because the
//! core mirror below is inlined.
//!
//! ```rust
//! use formars::prelude::*;
//!
//! let signup = object()
//!     .field("email", string().min(8).email())
//!     .field("age", coerced::<u32>()); // HTML inputs arrive as strings
//!
//! let mut input = formars::formars_core::value::Object::new();
//! input.insert("email", Value::from("ada@example.com"));
//! input.insert("age", Value::from("36"));
//!
//! let parsed = signup.parse(&input).expect("valid input");
//! assert_eq!(parsed.get("age"), Some(&Value::I64(36)));
//! ```
//!
//! Failures accumulate into one typed error, addressed by path:
//!
//! ```rust
//! use formars::prelude::*;
//!
//! let schema = object().field("email", string().email());
//! let bad = {
//!     let mut o = formars::formars_core::value::Object::new();
//!     o.insert("email", Value::from("not-an-email"));
//!     o
//! };
//! let Err(err) = schema.parse(&bad) else {
//!     unreachable!("invalid input must fail");
//! };
//!
//! let issue = err.first().expect("at least one issue");
//! assert_eq!(issue.path.to_string(), "email");
//! assert_eq!(issue.code, IssueCode::Email);
//! ```
//!
//! For deep reference — rule composition, coercion semantics, JSON interop —
//! see the `formars-core` documentation; everything there resolves identically
//! through the advanced mirror path `formars::formars_core`.
//!
//! ## Tier 2 — headless controller (`signals`)
//!
//! Enabling `signals` adds `reactive_graph` 0.2.x to your graph and exposes
//! the UI-agnostic `FormController` family plus the `Get`/`Set`/`Read`
//! traits, so one import drives an entire form without any rendering stack.
//! The shape of the story:
//!
//! ```text
//! use formars::prelude::*;
//!
//! let mut c = FormController::new(ValidateOn::Blur);
//! let email = c.register_with(
//!     FieldPath::key("email"),
//!     Box::new(string().min(8).email()),
//!     ValidateOn::Change,
//! )?;
//! email.value().set(Value::from("ada@example.com"));
//! assert!(c.validate().is_ok());
//! ```
//!
//! Rich docs live in the `formars-signals` crate (submit pipeline, snapshots,
//! server-error merge-back).
//!
//! ## Tier 3 — Leptos UI (`ui`, implies `signals`)
//!
//! Enabling `ui` adds `leptos` 0.8.x to your graph and binds the controller
//! into components: the `use_form` hook family, `<Form>` submit handling with
//! `SubmitOutcome`, and `<TextField>` bindings. Sketch:
//!
//! ```text
//! use formars::prelude::*;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn Signup() -> impl IntoView {
//!     let mut form = use_form(ValidateOn::Blur);
//!     let email = form.controller
//!         .register(FieldPath::key("email"), Box::new(string().min(8).email()))
//!         .expect("fresh registration");
//!     view! {
//!         <Form controller=form.controller /* ... */>
//!             <TextField field=email label="Email".to_string() />
//!         </Form>
//!     }
//! }
//! ```
//!
//! Rich docs live in the `formars-ui` crate (context model, IME caveat, SSR
//! exclusion).
//!
//! ## Tier 4 — derive macro (`derive`)
//!
//! Enabling `derive` adds the host-side `syn`/`quote`/`proc-macro2` compile
//! cost and re-exports exactly one item: the `FormSchema` derive macro
//! (available as `formars::FormSchema`).
//!
//! ### `FormSchema`: trait and macro coexist
//!
//! The name `FormSchema` is deliberately shared by two namespace-disjoint
//! items — the always-on core TRAIT (in [`prelude`], connecting a struct to
//! its companion schema) and this derive MACRO. Within one glob scope you can
//! derive it AND use it as a trait bound, exactly like `serde::Serialize`.
//! The `#[form(..)]` helper attributes activate automatically wherever the
//! imported derive is in scope — there is no separate `use formars::form`;
//! helper attributes registered by a derive are not importable items.
//!
//! ```text
//! use formars::prelude::*;
//!
//! #[derive(formars::FormSchema)]
//! struct Signup {
//!     #[form(label = "Your name")]
//!     name: String,
//! }
//!
//! // Same scope: the trait still names the companion-schema contract.
//! fn takes<T: FormSchema>(_t: &T) {}
//! ```
//!
//! Rich docs live in the `formars-derive` crate (attribute reference,
//! conformance matrix).
//!
//! ## Effects/unification note
//!
//! With `ui` enabled, `leptos` enters your graph, and `reactive_graph`'s
//! effect machinery is therefore PRESENT in the build (its effect module is
//! unconditional; only debug diagnostics are feature-gated upstream). This
//! does not compromise the signals layer: `formars-signals` code paths never
//! schedule effects, so nothing fires unless a component creates one. The
//! mitigation for consumers wanting lean graphs is feature granularity itself:
//! the default build is core-only and pulls none of this in.
//!
//! ## Versioning policy
//!
//! members and the `formars` umbrella move together; a breaking change
//! anywhere bumps all five.

#[doc(inline)]
pub use formars_core;

/// Headless reactive form controller built directly on `reactive_graph`.
/// Available on crate feature `signals` only.
#[cfg(feature = "signals")]
pub use formars_signals;

/// Leptos UI components and hooks: the [`use_form`](crate::prelude::use_form)
/// hook family plus `<Form>`/`<TextField>`. Available on crate feature `ui`
/// only.
#[cfg(feature = "ui")]
pub use formars_ui;

/// `#[derive(FormSchema)]` — importing this derive also activates
/// `#[form(..)]` attributes on the annotated struct's fields (helper
/// attributes are registered by the derive; they are not separately
/// importable items).
///
/// Note: coexists with the `FormSchema` TRAIT in [`prelude`] — different
/// namespaces (macro vs type/trait), matching the `serde::Serialize`
/// precedent.
///
/// Available on crate feature `derive` only.
#[cfg(feature = "derive")]
pub use formars_derive::FormSchema;

pub mod prelude;
