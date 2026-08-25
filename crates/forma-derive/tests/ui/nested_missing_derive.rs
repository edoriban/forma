use forma_derive::FormSchema;

struct Inner {
    q: String,
}

#[derive(FormSchema)]
struct Outer {
    inner: Inner,
}

fn main() {}
