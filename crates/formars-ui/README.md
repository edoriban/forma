# formars-ui

Leptos **0.8** companion crate for [`formars-signals`](../formars-signals): the
`use_form` hook, the `<Form>` submit wrapper, and the `<TextField>` field
binding. All reactive state lives in `formars-signals`; this crate adds only
the UI seam and passes `ArcRwSignal`/`ArcMemo` values straight through with
zero adapter layers.

## Consumer feature-combo support matrix (FU-DEP-6)

The library crate is feature-neutral (no `[features]` section); consumer
apps pick the leptos runtime feature:

| Consumer combo | leptos feature | Status |
|----------------|----------------|--------|
| Client-side rendered app | `csr` | ✅ supported in v0 |
| Hydrated app | `hydrate` | ✅ supported in v0 |
| Server-side rendered app (islands carry interactive parts) | `ssr` | ✅ supported in v0 (markup; submit execution is client-only) |
| Any other combination | — | ❌ unsupported-for-v0 |

## Quickstart

```rust
use formars_core::prelude::*; // schema builders live in the validation layer
use formars_ui::*;

#[component]
pub fn Signup() -> impl IntoView {
    view! {
        <>
            <SignupForm />
        </>
    }
}

fn SignupForm() -> impl IntoView {
    use leptos::prelude::*;
    use leptos::view;

    let mut form = use_form(ValidateOn::Blur);
    let email = form
        .controller
        .register_initial(
            FieldPath::key("email"),
            Box::new(string().email()),
            Value::from("preset@example.com"),
        )
        .expect("fresh registration");
    let controller = form.controller.clone();

    let handler = move |snapshot: FormSnapshot| {
        let payload = snapshot.get(&FieldPath::key("email")).cloned();
        async move { Ok::<Option<Value>, FormaError>(payload) }
    };
    let on_outcome = move |outcome: SubmitOutcome<Option<Value>, FormaError>| match outcome {
        SubmitOutcome::Success(_) => {}
        SubmitOutcome::HandlerError(server) => form.controller.apply_server_errors(&server),
        SubmitOutcome::ValidationFailed(_) => { /* fields already show issues */ }
    };

    view! {
        <Form controller on_outcome on_submit=handler submit_label="Save".to_string()>
            <TextField field=email label="Email".to_string() />
        </Form>
    }
}
```

Nested components resolve the same controller via
`use_form_controller()` / `try_form_controller()`; `<TextField>` itself takes
the handle as a REQUIRED prop (omitting it is a compile error) and never
self-registers from context.

## Dependency note: effects availability (FU-DEP-3)

Both `leptos` and `reactive_graph` are declared `default-features = false`
here. In `reactive_graph` 0.2.x the effect module
(`reactive_graph::effect`) is unconditional — the optional `effects`
feature gates only debug diagnostics, so no `effects` feature edge appears
in the feature graph. That is EXPECTED and harmless: `formars-signals` code
paths never schedule effects (`FS-DEP-2` intact); only `<TextField>`'s
display effect uses the always-present module. Verify with
`cargo tree -e features -p formars-ui`: reactive_graph feature edges come from
the leptos subtree only, never from formars-signals.

## SSR limitation (FU-FM-6)

`<Form>` markup may render server-side, but submit execution is
client-path only in v0: `spawn_local` needs a local-task executor that
server threads don't have, and the composed future is deliberately not
`Send`, so `tokio::spawn` can never drive it.

## IME / caret caveat (FU-TF-2)

DOM values are rewritten only when they differ from the signal string, so
same-value re-renders never move the caret and mid-composition IME state is
not clobbered by identical writes. Residual composition edge cases vary by
engine — verify manually in a browser before release.

## No-provider blur behavior (L-1)

Without an ancestor `use_form`, `<TextField>`'s blur resolves no controller
and skips touch-marking. The component still renders and binds correctly.

## Double-submit race shield (L-2)

Attempts are guarded against an in-flight submit and the built-in button is
reactively disabled while submitting. A rapid double-click slipping past the
guard before `is_submitting` flips is a known v0 edge; the disabled state is
the primary shield.
