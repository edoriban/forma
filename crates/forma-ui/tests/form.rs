//! FU-FM-2/3/5 public-contract guards over the composed future.
//!
//! These exercise exactly what `<Form>`'s spawned task performs per attempt,
//! through the public API only. The deeper glue (guard ordering, outcome
//! routing through the internal seams) is covered natively in
//! `src/form.rs::glue`; `spawn_local` itself is exercised on wasm32 (FU-IN-3).

use forma_core::prelude::*;
use forma_signals::{FieldPath, FormController, FormSnapshot, ValidateOn};
use futures::executor::block_on;
use reactive_graph::traits::{Get, Set};
use std::cell::Cell;
use std::rc::Rc;

fn controller() -> FormController {
    let mut c = FormController::new(ValidateOn::Submit);
    c.register(FieldPath::key("email"), Box::new(string().min(8).email()))
        .unwrap();
    c
}

#[test]
fn fm2_submitted_flip_precedes_handler_await_point() {
    let c = controller();
    c.field(&FieldPath::key("email"))
        .unwrap()
        .set_str("user@example.com");
    // `<Form>` flips submitted synchronously BEFORE driving the future.
    c.submitted().set(true);
    let observed = Rc::new(Cell::new(false));
    let view = c.clone();
    let obs = observed.clone();
    block_on(c.on_submit(move |_snap| {
        // at handler construction time the flip has already happened
        assert!(
            view.submitted().get(),
            "submitted must read true before any handler await point"
        );
        obs.set(true);
        async { Ok::<(), std::convert::Infallible>(()) }
    }))
    .ok();
    assert!(observed.get(), "handler must have run for a valid form");
    assert!(c.submitted().get());
}

#[test]
fn fm3_non_send_handler_future_accepted_and_completed() {
    let c = controller();
    c.field(&FieldPath::key("email"))
        .unwrap()
        .set_str("user@example.com");
    let state = Rc::new(Cell::new(0u8));
    let captured = state.clone();
    let result = block_on(c.on_submit(move |_snap| {
        let captured = captured.clone();
        async move {
            captured.set(captured.get() + 1);
            Ok::<u8, std::convert::Infallible>(captured.get())
        }
    }));
    assert_eq!(result.unwrap(), 1, "non-Send future ran to completion");
}

#[test]
fn fm5_server_error_roundtrip_ready_via_apply_server_errors() {
    let c = controller();
    c.field(&FieldPath::key("email"))
        .unwrap()
        .set_str("user@example.com");
    let payload = FormaError {
        issues: vec![FormaIssue {
            path: FieldPath::key("email"),
            code: IssueCode::Refine,
            message: "email already taken".into(),
            params: Vec::new(),
        }],
    };
    let err = block_on(c.on_submit(|_snap: FormSnapshot| async { Err::<(), _>(payload) }))
        .expect_err("handler failed");
    // Caller-side Handler arm handling — exactly what the on_outcome closure does:
    match err {
        forma_signals::SubmitError::Handler(e) => c.apply_server_errors(&e),
        forma_signals::SubmitError::Validation(_) => {
            panic!("expected Handler arm, got Validation")
        }
    }
    let visible = c
        .field(&FieldPath::key("email"))
        .unwrap()
        .visible_errors()
        .get();
    assert!(
        visible.iter().any(|i| matches!(i.code, IssueCode::Refine)),
        "server issue must appear in visible_errors after roundtrip"
    );
}
