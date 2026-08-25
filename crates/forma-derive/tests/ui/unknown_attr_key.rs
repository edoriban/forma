use forma_derive::FormSchema;

#[derive(FormSchema)]
struct Signup {
    #[form(lable = "Name")]
    name: String,
}

fn main() {}
