//! `formars-core`: typed schema primitives with a dyn-safe erased view.
//! Fluent builder schemas for validation, coercion and introspection — the
//! kernel every other `formars*` crate builds on.
//!
//! A schema is a *value* you build, compose, and pass around:
//!
//! ```rust
//! use formars_core::prelude::*;
//!
//! let s = string().trim().min(2).max(5);
//! assert!(s.parse(&"ab".to_string()).is_ok());
//! ```
//!
//! Every builder simultaneously implements the typed [`Schema`] trait and the
//! object-safe [`DynSchema`] erased view from one internal representation, so
//! both views always agree. Object schemas compose any builder family into
//! struct-shaped validation over an ordered [`Value::Object`]:
//!
//! ```rust
//! use formars_core::prelude::*;
//!
//! let user = object()
//!     .field("name", string().min(1).label("Full name"))
//!     .field("age", coerced::<u32>());
//! ```
//!
//! [`Value::Object`]: formars_core::value::Value::Object

/// String-to-type coercion schemas.
pub mod coerce;
/// Error model: accumulated, path-addressed issues.
pub mod error;
/// Struct↔schema contracts and value bridging for the opt-in `formars-derive` crate.
pub mod form;
/// Curated re-exports for typical users.
pub mod prelude;
/// Custom validation rules that compose identically to built-in checks.
pub mod rule;
/// The two validation views and their shared introspection types.
pub mod schema;
/// Primitive schema builders and their constructors.
pub mod types;
/// Pragmatic hand-rolled format validators (no regex dependency in v0).
pub mod validators;
/// Dependency-free dynamic value tree and its accessors.
pub mod value;
