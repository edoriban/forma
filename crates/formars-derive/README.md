# formars-derive

Opt-in `#[derive(FormSchema)]` companion for [`formars-core`]: derives a
companion schema type from a plain named-field struct — nested composition,
renames, skips, metadata slots, and builder overrides via a small closed
`#[form(..)]` attribute grammar.

```rust
use formars_core::prelude::*;
use formars_derive::FormSchema;

#[derive(FormSchema)]
struct Signup {
    #[form(label = "Email")]
    email: String,
    age: u32,
}

let issues = SignupSchema::new()
    .validate_value(&Value::from("not-an-object"));
```

The macro executes on the host compiler only — wasm consumers pay nothing
extra. See the [repository](https://github.com/edoriban/formars) and the
[`formars`](https://docs.rs/formars) facade for the full toolkit.

License: MIT — see [LICENSE](LICENSE).
