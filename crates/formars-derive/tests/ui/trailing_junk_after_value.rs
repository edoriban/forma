use formars_derive::FormSchema;

#[derive(FormSchema)]
struct S {
    #[form(label = "x" junk)]
    name: String,
}

fn main() {}
