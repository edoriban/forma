//! Two renames resolving to the same wire key must be rejected at compile
//! time: object lookup is last-write-wins in core, so silent acceptance
//! would corrupt data.

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    #[form(rename = "k")]
    a: u32,
    #[form(rename = "k")]
    b: String,
}

fn main() {}
