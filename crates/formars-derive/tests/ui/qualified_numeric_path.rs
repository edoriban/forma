//! Qualified numeric path (`::core::primitive::u32`) gets the same targeted
//! name-mapping diagnostic as qualified `String`.

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    n: ::core::primitive::u32,
}

fn main() {}
