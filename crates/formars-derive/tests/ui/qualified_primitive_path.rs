//! Qualified primitive path (`::std::string::String`) must get the targeted
//! name-mapping diagnostic — never E0277 bound noise suggesting
//! `#[derive(FormSchema)]` on the primitive.

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    s: ::std::string::String,
}

fn main() {}
