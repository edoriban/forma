//! Docs-vs-behavior conformance (negative branch): `type Age = u32;` is NOT
//! resolved by name or trait bounds — it falls through to nested composition
//! and fails the `FormSchema` bound, exactly as the corrected crate docs
//! state.

use formars_derive::FormSchema;

type Age = u32;

#[derive(FormSchema)]
struct S {
    age: Age,
}

fn main() {}
