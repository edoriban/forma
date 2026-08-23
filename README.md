# forma

Schema validation for Rust, composed with a fluent builder API — no derive macros, no codegen.

> **Status: early design.** No public API yet. Expect everything below to change.

## Why

Rust validation crates usually ask you to describe your rules in attributes on a struct.
`forma` takes the other route, the one `zod` popularized: a schema is a *value* you build,
compose, and pass around.

```rust
// Sketch — not yet implemented.
let user = object()
    .field("email", string().email())
    .field("age", number().int().min(18));

let parsed = user.parse(input)?;
```

## Goals

- **Fluent composition, not macros.** Schemas are built by chaining; type-state builders keep
  invalid combinations from compiling.
- **Typed errors.** Validation failures come back as `Result` with structured error paths, never
  a panic and never a bare `String`.
- **Zero-cost where it counts.** Generics over `dyn Trait` unless erasure buys something real.
- **Small core.** Framework glue (starting with Leptos form hooks) stays behind a feature flag or
  in a companion crate so the core stays lean and `no_std`-friendly where possible.

## Non-goals

- A `#[derive(Validate)]` attribute language.
- Being a serialization library. `forma` validates and parses; `serde` deserializes.

## Development

```bash
cargo test
cargo clippy --all-targets --all-features
cargo fmt --check
```

## License

MIT — see [LICENSE](LICENSE).
