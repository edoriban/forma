use formars_derive::FormSchema;

#[derive(FormSchema)]
struct Signup {
    #[form(rename = "a")]
    #[form(rename = "b")]
    name: String,
}

fn main() {}
