# formars

Schema validation for Rust, behind one dependency and one import. Schemas are
**values** you compose with builders — not attributes on structs — then parse,
validate, and inspect. Opt-in feature tiers add reactive headless form state,
Leptos UI components, and a derive macro.

```rust
use formars::prelude::*;

let user = object()
    .field("email", string().email())
    .field("age", coerced::<u32>()); // HTML form inputs arrive as strings

let mut input = formars::formars_core::value::Object::new();
input.insert("email", Value::from("ada@example.com"));
input.insert("age", Value::from("36"));

let parsed = user.parse(&input).expect("valid input");
assert_eq!(parsed.get("age"), Some(&Value::I64(36)));
```

This is the default tier — schemas and typed errors, zero extra dependencies.
Feature flags unlock the rest:

| Tier                | Feature                  | Adds to your graph                               |
|---------------------|--------------------------|--------------------------------------------------|
| Validate-only       | *(default)*              | nothing beyond `formars-core` (zero deps)        |
| Headless controller | `signals`                | `reactive_graph` 0.2.x                           |
| Leptos UI           | `ui` (implies `signals`) | `leptos` 0.8.x + one shared `reactive_graph` copy |
| Derive macro        | `derive`                 | syn/quote/proc-macro2 (host-side only)           |

Enable tiers in `Cargo.toml`:

```toml
[dependencies]
formars = { version = "0.1", features = ["ui", "derive"] }
```

- Reactive forms: `FormController` registers fields, tracks touched/dirty,
  brackets submits, and merges server errors — headless and wasm-friendly
  (`formars-signals`).
- Leptos components: `<Form>` submit wrapper with double-submit shielding,
  `<TextField>` with bidirectional value bridging (`formars-ui`, Leptos 0.8).
- Derive: `#[derive(FormSchema)]` generates a companion schema from a plain
  named-field struct (`formars-derive`; the macro runs on the host compiler,
  so wasm consumers pay nothing extra).

## Status

Pre-0.1: the API is still allowed to change before the first release.

## Repository

See the [workspace README](https://github.com/edoriban/formars) for the full
guide, architecture notes, and the CI gate battery.

## License

MIT — see [LICENSE](LICENSE).
