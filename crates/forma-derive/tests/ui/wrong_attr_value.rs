use forma_derive::FormSchema;

#[derive(FormSchema)]
struct Signup {
    #[form(rename = 42)]
    name: String,
}

fn main() {}
