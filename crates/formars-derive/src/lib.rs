//! `formars-derive`: opt-in `#[derive(FormSchema)]` macro generating
//! `formars-core` companion schemas.
//!
//! Snapshot regen protocol: trybuild `.stderr` snapshots regenerate ONLY on
//! the pinned gate toolchain 1.98.0 (see `.github/workflows/ci.yml`) via
//! `TRYBUILD=overwrite cargo test -p formars-derive --test ui`, diffs
//! hand-reviewed; never per-toolchain.
//!
//! `formars-core` is builder-first and macro-free; this crate is the optional
//! ergonomic layer that removes the hand-composition step. Deriving
//! `FormSchema` on a plain named-field struct emits a companion schema type
//! whose single composed representation is a real `::formars_core::ObjectSchema`
//! assembled through the public builders — the walk kernel, joined paths,
//! fail-fast descent and dual-view parity are all inherited from core, never
//! reimplemented.
//!
//! # Example
//!
//! Typed parse, erased validation and nesting, all from one derive:
//!
//! ```
//! use formars_core::prelude::*;
//! use formars_core::value::{Object, Value};
//! use formars_derive::FormSchema;
//!
//! #[derive(FormSchema)]
//! struct Signup {
//!     #[form(label = "Full name")]
//!     name: String,
//!     age: u32, // HTML-form strings coerce into numeric fields
//!     address: Address,
//! }
//!
//! #[derive(FormSchema)]
//! struct Address {
//!     city: String,
//! }
//!
//! // ONE composed representation serves both views (SC-9).
//! let schema = <Signup as FormSchema>::form_schema();
//!
//! // Typed parse: struct in, validated struct out.
//! let parsed = schema.parse(&Signup {
//!     name: "Ada".into(),
//!     age: 36,
//!     address: Address { city: "London".into() },
//! });
//! assert!(parsed.is_ok());
//!
//! // Erased view over a raw Value tree (what formars-signals/formars-ui consume).
//! let mut addr = Object::new();
//! addr.insert("city", Value::from("London"));
//! let mut input = Object::new();
//! input.insert("name", Value::from("Ada"));
//! input.insert("age", Value::from("abc")); // not coercible into u32
//! input.insert("address", Value::Object(addr));
//!
//! let issues = schema.validate_value(&Value::Object(input));
//! assert_eq!(issues.len(), 1);
//! assert_eq!(issues[0].code, IssueCode::Coerce);
//! assert_eq!(issues[0].path.to_string(), "age");
//! ```
//!
//! # Attribute reference (v0 closed grammar)
//!
//! Exactly six keys are recognized inside `#[form(..)]`; anything else is a
//! hard compile error listing the valid keys.
//!
//! | Attribute | Example | Effect |
//! |-----------|---------|--------|
//! | `schema = <expr>` | `#[form(schema = string().trim().email())]` | Full override: `expr` replaces the default mapping at this field's position |
//! | `rename = "wire"` | `#[form(rename = "user_email")]` | Remaps the object key everywhere (lookup, reconstruction, `shape()`, `field_meta`); the Rust identifier is untouched |
//! | `skip` | `#[form(skip)]` | Field excluded from schema and bridging entirely; passed through untouched on typed parse |
//! | `label = ".."` | `#[form(label = "Email")]` | Populates `FieldMeta::label`, reachable via `metadata()` / `field_meta` |
//! | `description = ".."` | `#[form(description = "..")]` | Populates `FieldMeta::description` |
//! | `placeholder = ".."` | `#[form(placeholder = "..")]` | Populates `FieldMeta::placeholder` |
//!
//! Rules: one attribute list per field; duplicate keys error; `skip` cannot be
//! combined with any other key; container-level attributes do not exist in v0
//! and are rejected as unrecognized.
//!
//! # Supported field types (default mapping)
//!
//! | Type(s) | Mapping |
//! |---------|---------|
//! | `String` | `string()` |
//! | `bool` | `bool()` |
//! | `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `f32`, `f64` | `coerced::<T>()` (HTML forms deliver strings) |
//! | Any type implementing `FormSchema` | Nested composition via `Nested::new(<T as FormSchema>::form_schema())` |
//! | Anything else (aliases included) | Compile error naming the field, suggesting `#[form(schema = ..)]` |
//!
//! Mapping is strictly NAME-based: only a single-segment path whose final
//! segment is exactly an identifier from the table above resolves to its
//! builder. Type aliases (`type Age = u32;`) are NOT resolved by trait
//! bounds: they fall through to nested composition and compile only if the
//! aliased type independently implements `FormSchema` + `FormBridge`.
//! Fully-qualified primitive paths (`::std::string::String`,
//! `::core::primitive::u32`) are not mapped by name either; the derive
//! rejects them with a targeted compile error suggesting the bare name or
//! `#[form(schema = ..)]`.
//!
//! # Generated contract
//!
//! For `struct X { .. }` the derive emits, in the same module:
//!
//! - `struct XSchema` taking the STRUCT'S visibility (a private struct yields
//!   a private companion; `pub` yields `pub`) owning its composed
//!   `::formars_core::ObjectSchema`, with an `XSchema::new()` constructor;
//! - `impl Schema for XSchema` (`Input = Output = X`) — bridges `X` to an
//!   object via `FormBridge`, validates, reconstructs;
//! - `impl DynSchema for XSchema` — pure pass-through to the composed object;
//! - `impl AsRef<::formars_core::ObjectSchema> for XSchema`;
//! - `impl FormSchema for X` and `impl FormBridge for X`.
//!
//! # Companion naming (reserved suffix)
//!
//! `struct Foo` generates its companion `FooSchema` **in the same module**.
//! If a pre-existing item named `FooSchema` is already in scope there, the
//! generated definition collides with it and rustc reports E0428 ("the name
//! `FooSchema` is defined multiple times") pointing INTO generated code,
//! with no hint about the derive as the cause. Convention: never name items
//! `{Struct}Schema` — that suffix is reserved for generated companions.
//!
//! Nested fields carry a **dual bound**: their type must implement BOTH
//! `FormSchema` (composition) AND `FormBridge` (typed-parse bridging). A
//! hand-written `FormSchema` impl without `FormBridge` fails compilation.
//!
//! Skipped fields must implement `Clone`: typed parse copies them verbatim
//! from the input. Because such a struct declines erased reconstruction in
//! v0, it must NOT be used as a nested child — the parent compiles but typed
//! parse fails at runtime with `TypeMismatch` ("validated output is missing
//! this field").
//!
//! Deviation from the spec's soft zero-sized-companion preference: `XSchema`
//! stores its composed `ObjectSchema` instead of being a ZST. This buys the
//! natural `AsRef` used by nested composition and keeps `shape()`
//! memoization; construction cost equals hand-building once.
//!
//! # Package-name coupling
//!
//! All generated code references core items via absolute `::formars_core::..`
//! paths (proc-macros have no `$crate` escape hatch). Renaming or shadowing
//! the `formars-core` package is therefore a breaking change for derive users.

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod attrs;
mod expansion;

/// Derives a `formars` form schema companion for the annotated struct.
///
/// See the crate-level docs for the supported shapes and attribute grammar.
#[proc_macro_derive(FormSchema, attributes(form))]
pub fn derive_form_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match expansion::expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
