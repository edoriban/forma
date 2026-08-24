//! FU-IN-1/2/3 compile-time probes: Send/Sync static assertions on the new
//! public types, an inventory audit where every mandated re-export is
//! referenced BY NAME (rename/removal breaks the build), and a
//! single-import example shape using NO direct `forma_signals` import.
//! Schema builders come from `forma-core` (the validation layer), not from
//! forma-signals — that edge is unchanged from the base design.

use forma_core::prelude::*;
use forma_ui::{
    FieldHandle, FieldPath, Form, FormController, FormSnapshot, FormaError, FormaIssue, IssueCode,
    RegisterError, SubmitError, SubmitOutcome, TextField, UseForm, ValidateOn, Value, use_form,
};

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}

    // new public types
    assert_send_sync::<UseForm>();
    assert_send_sync::<SubmitOutcome<(), ()>>();
    // re-exported controller must stay freely shareable across tasks
    assert_send_sync::<FormController>();
};

/// Every FU-IN-1 re-export plus hook accessors, referenced by name.
#[test]
fn inventory_audit_references_every_public_item() {
    let names = [
        std::any::type_name::<FormController>(),
        std::any::type_name::<FieldHandle>(),
        std::any::type_name::<ValidateOn>(),
        std::any::type_name::<RegisterError>(),
        std::any::type_name::<SubmitError<String>>(),
        std::any::type_name::<FormSnapshot>(),
        std::any::type_name::<SubmitOutcome<(), String>>(),
        std::any::type_name::<FormaError>(),
        std::any::type_name::<FormaIssue>(),
        std::any::type_name::<FieldPath>(),
        std::any::type_name::<IssueCode>(),
        std::any::type_name::<Value>(),
        std::any::type_name::<UseForm>(),
    ];
    for name in names {
        assert!(!name.is_empty());
    }
    // hook accessors resolve as function items
    let hook: fn(ValidateOn) -> UseForm = forma_ui::use_form;
    let expect_hook: fn() -> FormController = forma_ui::use_form_controller;
    let try_hook: fn() -> Option<FormController> = forma_ui::try_form_controller;
    // constructible outside any provider (context provisioning degrades to a no-op)
    let probe = hook(ValidateOn::Change);
    assert!(
        probe
            .controller
            .field(&FieldPath::key("unregistered"))
            .is_none()
    );
    let _ = (expect_hook, try_hook);
}

/// Single-import example: hook + components + error types with NO direct
/// `forma_signals` import anywhere in this file.
#[test]
fn one_import_example_shape_compiles() {
    // example is built but never mounted; silence dead-code diagnostics
    #[allow(
        dead_code,
        reason = "shape-only compile probe, intentionally not invoked"
    )]
    fn example_view() -> impl leptos::prelude::IntoView {
        use leptos::prelude::*;
        use leptos::view;

        Owner::new().with(|| {
            let mut form = use_form(ValidateOn::Blur);
            let registration: Result<FieldHandle, RegisterError> = form
                .controller
                .register(FieldPath::key("email"), Box::new(string()));
            if let Ok(handle) = registration {
                handle.set_str("user@example.com");
            }
            let controller = form.controller.clone();
            let email = controller.field(&FieldPath::key("email")).unwrap();

            let on_outcome = move |outcome: SubmitOutcome<String, FormaError>| match outcome {
                SubmitOutcome::Success(value) => drop(value),
                SubmitOutcome::HandlerError(server) => {
                    controller.apply_server_errors(&server);
                }
                SubmitOutcome::ValidationFailed(err) => {
                    drop(err);
                }
            };

            let handler = |snapshot: FormSnapshot| {
                let _ = snapshot;
                async { Ok::<String, FormaError>(String::new()) }
            };
            let issue_code: Option<IssueCode> = None;
            assert!(issue_code.is_none());

            view! {
                <Form
                    controller=form.controller.clone()
                    on_outcome=on_outcome
                    on_submit=handler
                    submit_label="Save".to_string()
                >
                    <TextField field=email label="Email".to_string() />
                </Form>
            }
        })
    }
}
