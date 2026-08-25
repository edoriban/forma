use formars_derive::FormSchema;

#[derive(FormSchema)]
struct Signup {
    #[form(skip, rename = "wire")]
    secret: String,
}

fn main() {}
