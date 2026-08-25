//! A plain field identifier colliding with another field's rename must be
//! rejected at compile time: resolved wire keys participate equally,
//! regardless of whether they came from `rename` or the ident itself.

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    k: u32,
    #[form(rename = "k")]
    v: String,
}

fn main() {}
