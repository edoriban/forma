//! FSM-1/FSM-2: registry round-trip, unknown lookup, duplicate rejection.

use forma_core::prelude::*;
use forma_signals::{FormController, RegisterError, ValidateOn};
use reactive_graph::traits::{Get, Set};

fn controller() -> FormController {
    FormController::new(ValidateOn::Blur)
}

#[test]
fn fsm2_register_returns_handle_and_lookup_some() {
    let mut c = controller();
    let handle = c
        .register(FieldPath::key("email"), Box::new(string().email()))
        .expect("fresh registration");
    assert_eq!(handle.path(), &FieldPath::key("email"));
    let looked_up = c.field(&FieldPath::key("email"));
    assert!(looked_up.is_some());
    assert_eq!(
        looked_up.unwrap().path(),
        handle.path(),
        "lookup must address the same field"
    );
}

#[test]
fn fsm2_unknown_path_lookup_none() {
    let mut c = controller();
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    c.register(FieldPath::key("b"), Box::new(string())).unwrap();
    assert!(c.field(&FieldPath::key("c")).is_none());
}

#[test]
fn fsm2_duplicate_registration_rejected_with_register_error() {
    let mut c = controller();
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    let Err(dup) = c.register(FieldPath::key("a"), Box::new(string())) else {
        panic!("expected Duplicate error, got Ok");
    };
    match dup {
        RegisterError::Duplicate { path } => {
            assert_eq!(path, FieldPath::key("a"));
        }
    }
}

#[test]
fn fsm1_handle_write_reaches_controller_state() {
    let mut c = controller();
    let handle = c
        .register(FieldPath::key("email"), Box::new(string()))
        .unwrap();
    handle.value().set(Value::from("edited"));
    let via_controller = c.field(&FieldPath::key("email")).unwrap().value().get();
    assert_eq!(via_controller, Value::from("edited"));
    assert_eq!(
        handle.value().get(),
        Value::from("edited"),
        "handle and controller share one source of truth"
    );
}

#[test]
fn fsm1_reset_restores_controller_wide_consistency() {
    let mut c = controller();
    for name in ["a", "b"] {
        let h = c
            .register(FieldPath::key(name), Box::new(string().min(2)))
            .unwrap();
        h.value().set(Value::from("changed!"));
        assert!(h.dirty().get(), "{name} should be dirty after edit");
    }
    c.reset();
    for name in ["a", "b"] {
        let h = c.field(&FieldPath::key(name)).unwrap();
        assert_eq!(h.value().get(), Value::from(""));
        assert!(!h.dirty().get(), "{name} must be clean after reset");
    }
}
