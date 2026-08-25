use formars_derive::FormSchema;

struct Inner {
    q: String,
}

#[derive(FormSchema)]
struct Outer {
    #[form(label = "Inner")]
    inner: Inner,
}

fn main() {}
