//! A raw identifier field colliding with another field's rename must be
//! rejected at compile time: `r#type` resolves to wire key `type` (the `r#`
//! prefix is Rust syntax, never part of the object key), and resolved wire
//! keys participate equally regardless of their origin.

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    r#type: u32,
    #[form(rename = "type")]
    v: String,
}

fn main() {}
