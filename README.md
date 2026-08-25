# Formars

Schema validation for Rust. The core is builder-first and macro-free — schemas are values you compose by hand; `#[derive(FormSchema)]` is available as an opt-in convenience via the separate [`formars-derive`](crates/formars-derive) crate.

> **Status: pre-0.1.** The core pieces are implemented — `formars-core` (schemas,
> values, errors), `formars-signals` (headless reactive form controller),
> `formars-ui` (Leptos components), and `formars-derive` (`#[derive(FormSchema)]`) —
> but the API is still allowed to change before the first release.

## Why

Rust validation crates usually ask you to describe your rules in attributes on a struct.
`formars` takes the other route, the one `zod` popularized: a schema is a *value* you build,
compose, and pass around.

```rust
use formars_core::prelude::*;
use formars_core::value::Object;

let user = object()
    .field("email", string().email())
    .field("age", coerced::<u32>()); // HTML form inputs arrive as strings

let mut input = Object::new();
input.insert("email", Value::from("ada@example.com"));
input.insert("age", Value::from("36"));

let parsed = user.parse(&input).expect("valid input");
assert_eq!(parsed.get("age"), Some(&Value::I64(36)));
```

## Goals

- **Fluent composition, not macros.** Schemas are built by chaining; type-state builders keep
  invalid combinations from compiling.
- **Typed errors.** Validation failures come back as `Result` with structured error paths, never
  a panic and never a bare `String`.
- **Zero-cost where it counts.** Generics over `dyn Trait` unless erasure buys something real.
- **Small core.** Framework glue (starting with Leptos form hooks) stays behind a feature flag or
  in a companion crate so the core stays lean and `no_std`-friendly where possible.
- **Zero dependencies where it matters.** `formars-core` has zero runtime dependencies; the derive
  lives outside it so non-macro users never pay for `syn`.

## Non-goals

- A sprawling attribute language on top of the derive. Core stays macro-free; `formars-derive`
  provides opt-in `#[derive(FormSchema)]` support (v0 scope: named-field structs with a closed
  six-key attribute set).
- Being a serialization library. `formars` validates and parses; `serde` deserializes.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check

# wasm32 gate. The proc-macro crate is excluded (proc-macro crates cannot be
# built FOR wasm32); consumers targeting wasm MAY still use the derive, since
# the macro executes on the host compiler.
cargo build --target wasm32-unknown-unknown --workspace --exclude formars-derive
```

## License

MIT — see [LICENSE](LICENSE).
