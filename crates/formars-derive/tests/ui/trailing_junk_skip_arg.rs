use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    #[form(skip(x))]
    name: String,
}

fn main() {}
