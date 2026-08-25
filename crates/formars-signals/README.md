# formars-signals

Headless reactive form controller on `reactive_graph`: an insertion-ordered
field registry, per-field validation timing (`Change`/`Blur`/`Submit`),
touched/dirty tracking, a submit boundary with an in-flight bracket, and
server-error merging — no DOM, no spawner, wasm-friendly.

```rust
use formars_core::prelude::*;
use formars_signals::{FormController, ValidateOn};

let mut form = FormController::new(ValidateOn::Blur);
form.register(FieldPath::key("email"), Box::new(string().email()))?;
```

Pair it with [`formars-ui`](https://docs.rs/formars-ui) for Leptos
components, or drive it headless from any reactive consumer. See the
[repository](https://github.com/edoriban/formars) and the
[`formars`](https://docs.rs/formars) facade.

License: MIT — see [LICENSE](LICENSE).
