use forma_derive::FormSchema;

#[derive(FormSchema)]
struct Borrow<'a> {
    name: &'a str,
}

fn main() {}
