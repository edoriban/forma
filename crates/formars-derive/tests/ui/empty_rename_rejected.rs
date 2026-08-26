use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    #[form(rename = "")]
    name: String,
}

fn main() {}
