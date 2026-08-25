// `#[form]` is a helper attribute of the derive: on a struct WITHOUT the
// derive, rustc itself rejects it as unregistered.
#[form(label = "nope")]
struct Plain {
    name: String,
}

fn main() {}
