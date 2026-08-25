# formars-core

Typed schema primitives with a dyn-safe erased view for form validation:
builder-first composition (`object()`, `string()`, `coerced::<T>()`, ...),
a `Value` tree with JSON interop behind the `serde` feature, path-addressed
typed errors, and the `Schema`/`DynSchema` dual view.

```rust
use formars_core::prelude::*;

let schema = object().field("email", string().email());
assert!(schema.validate_value(&Value::from("nope")).len() == 1);
```

This crate is macro-free and dependency-light; `formars-derive` adds
`#[derive(FormSchema)]` on top of it. See the
[repository](https://github.com/edoriban/formars) and the
[`formars`](https://docs.rs/formars) facade for the full toolkit.

License: MIT — see [LICENSE](LICENSE).
