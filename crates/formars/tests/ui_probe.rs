//! FA-IN-4 probe: the Leptos UI tier through ONLY two import families —
//! `use formars::prelude::*;` and `use leptos::prelude::*;` (`Owner` comes
//! from the leptos prelude, so the facade needs no dev-dependency; design
//! D5). Rendering stays unmounted inside owner scopes, mirroring member-crate
//! practice.

#![cfg(feature = "ui")]

use formars::prelude::*;
use leptos::prelude::*;

fn in_owner<T>(f: impl FnOnce() -> T) -> T {
    Owner::new().with(f)
}

#[test]
fn hk_smoke_use_form_registers_and_context_resolves() {
    in_owner(|| {
        let mut form = use_form(ValidateOn::Blur);
        form.controller
            .register(FieldPath::key("email"), Box::new(string()))
            .expect("fresh registration");
        assert!(
            form.controller.field(&FieldPath::key("email")).is_some(),
            "registered path must resolve to a handle"
        );

        // Context round trip: a descendant scope reaches the SAME controller.
        Owner::new().with(|| {
            let ctrl = use_form_controller();
            ctrl.field(&FieldPath::key("email"))
                .unwrap()
                .set_str("from-child");
        });
        assert_eq!(
            form.controller
                .field(&FieldPath::key("email"))
                .unwrap()
                .get_str(),
            Some("from-child".to_string()),
            "descendant write must hit the parent's field state"
        );
    });
}

#[test]
fn hk2_try_form_controller_none_outside_provider() {
    in_owner(|| {
        assert!(
            try_form_controller().is_none(),
            "no provider anywhere: try variant must return None"
        );
    });
}

#[test]
fn seven_ui_names_resolve_through_prelude() {
    in_owner(|| {
        let mut form = use_form(ValidateOn::Blur);
        let email = form
            .controller
            .register(FieldPath::key("name"), Box::new(string()))
            .expect("fresh registration");
        email.set_str("ada");

        // Component + outcome names build real (unmounted) views in scope.
        let ctrl_for_outcome = form.controller.clone();
        let rejected = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let rejected_for_cb = std::sync::Arc::clone(&rejected);
        let on_outcome = move |outcome: SubmitOutcome<(), FormaError>| match outcome {
            SubmitOutcome::Success(()) => {}
            SubmitOutcome::HandlerError(server) => {
                ctrl_for_outcome.apply_server_errors(&server);
            }
            SubmitOutcome::ValidationFailed(_err) => {
                use std::sync::atomic::Ordering;
                rejected_for_cb.fetch_add(1, Ordering::Relaxed);
            }
        };
        let _view = view! {
            <Form
                controller=form.controller.clone()
                on_outcome
                on_submit=|_snap: FormSnapshot| async { Ok::<(), FormaError>(()) }
                submit_label="Go".to_string()
            >
                <TextField field=email label="Email".to_string() />
            </Form>
        };

        // Hook type resolves in type position.
        let _: UseForm = use_form(ValidateOn::Submit);
    });
}
