use forma_derive::FormSchema;

#[derive(FormSchema)]
struct Wrapper<T> {
    inner: T,
}

fn main() {}
