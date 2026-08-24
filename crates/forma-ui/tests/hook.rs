//! FU-HK-1..4: hook construction, context model, prefill ergonomics.
//! Native-only harness: owner scopes via `Owner::new().with(...)`.

use forma_core::prelude::*;
use forma_signals::{FieldPath, ValidateOn, Value};
use forma_ui::{try_form_controller, use_form, use_form_controller};
use reactive_graph::owner::Owner;
use reactive_graph::traits::Get;

fn in_owner<T>(f: impl FnOnce() -> T) -> T {
    Owner::new().with(f)
}

fn email_schema() -> Box<dyn DynSchema> {
    Box::new(string())
}

#[test]
fn hk1_construction_in_owner_scope() {
    in_owner(|| {
        let mut form = use_form(ValidateOn::Blur);
        form.controller
            .register(FieldPath::key("email"), email_schema())
            .expect("fresh registration");
        assert!(
            form.controller.field(&FieldPath::key("email")).is_some(),
            "registered path must resolve to a handle"
        );
    });
}

#[test]
fn hk1_clone_survives_closure() {
    in_owner(|| {
        let mut form = use_form(ValidateOn::Blur);
        form.controller
            .register(FieldPath::key("email"), email_schema())
            .unwrap();
        let handle = form.controller.field(&FieldPath::key("email")).unwrap();
        let later = move || handle.set_str("later");
        later();
        assert_eq!(
            form.controller
                .field(&FieldPath::key("email"))
                .unwrap()
                .get_str(),
            Some("later".to_string()),
            "write through the closure-captured clone must be visible on the original"
        );
    });
}

#[test]
fn hk2_context_round_trip() {
    Owner::new().with(|| {
        let mut form = use_form(ValidateOn::Blur);
        form.controller
            .register(FieldPath::key("email"), email_schema())
            .unwrap();
        Owner::new().with(|| {
            let resolved = use_form_controller();
            resolved
                .field(&FieldPath::key("email"))
                .unwrap()
                .set_str("from-child");
        });
        assert_eq!(
            form.controller
                .field(&FieldPath::key("email"))
                .unwrap()
                .get_str(),
            Some("from-child".to_string()),
            "descendant write through context must hit the parent's field state"
        );
    });
}

#[test]
fn hk2_try_none_without_provider() {
    in_owner(|| {
        assert!(
            try_form_controller().is_none(),
            "no provider anywhere: try variant must return None"
        );
    });
}

#[test]
#[should_panic(expected = "context")]
fn hk2_expect_panics_without_provider() {
    in_owner(|| {
        let _ = use_form_controller();
    });
}

/// Per M-1: the "nested field resolves via context" scenario is realized by
/// CALLER-WRITTEN wrappers over `use_form_controller()` that construct a
/// handle and pass it down explicitly — `<TextField>` never self-registers.
#[test]
fn hk3_nested_access_via_caller_wrapper() {
    Owner::new().with(|| {
        let form = use_form(ValidateOn::Blur);
        Owner::new().with(|| {
            // caller-written nested wrapper body
            let mut ctrl = use_form_controller();
            let handle = ctrl
                .register_initial(
                    FieldPath::key("nested.email"),
                    Box::new(string()),
                    Value::from("preset"),
                )
                .unwrap();
            handle.set_str("edited-in-wrapper");
        });
        let h = form
            .controller
            .field(&FieldPath::key("nested.email"))
            .unwrap();
        assert_eq!(
            h.get_str(),
            Some("edited-in-wrapper".to_string()),
            "wrapper's writes and the parent's view must agree"
        );
    });
}

#[test]
fn hk4_prefill_clean() {
    in_owner(|| {
        let mut form = use_form(ValidateOn::Blur);
        let handle = form
            .controller
            .register_initial(
                FieldPath::key("name"),
                Box::new(string()),
                Value::from("preset"),
            )
            .unwrap();
        assert_eq!(handle.get_str(), Some("preset".to_string()));
        assert!(!handle.dirty().get(), "prefilled field starts clean");
    });
}

#[test]
fn hk4_reset_restores_preset() {
    in_owner(|| {
        let mut form = use_form(ValidateOn::Blur);
        let handle = form
            .controller
            .register_initial(
                FieldPath::key("name"),
                Box::new(string()),
                Value::from("preset"),
            )
            .unwrap();
        handle.set_str("mutated");
        assert!(handle.dirty().get());
        form.controller.reset();
        assert_eq!(handle.get_str(), Some("preset".to_string()));
        assert!(!handle.dirty().get(), "reset restores the preset anchor");
    });
}
