use formars_derive::FormSchema;

#[derive(FormSchema)]
struct Borrow<'a> {
    name: &'a str,
}

fn main() {}
