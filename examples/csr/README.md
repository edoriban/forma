# formars CSR example

Standalone Leptos 0.8 CSR app (not a workspace member). It consumes the
toolkit exclusively through the `formars` umbrella crate with the `ui`
feature — one dependency, one import (`use formars::prelude::*;`).
Dev: `trunk serve`. Build: `trunk build --release` or `cargo build --target wasm32-unknown-unknown`.
