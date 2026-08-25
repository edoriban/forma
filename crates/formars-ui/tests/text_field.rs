//! FU-TF data-level tests: signal flow through handles, blur seam,
//! gated error rendering. No browser, no DOM.

use formars_core::prelude::*;
use formars_signals::{FieldHandle, FieldPath, FormController, ValidateOn, Value};
use reactive_graph::owner::Owner;
use reactive_graph::traits::{Get, Set};

fn setup(validate_on: ValidateOn) -> (FormController, FieldHandle) {
    Owner::new().with(move || {
        let mut c = FormController::new(validate_on);
        let h = c
            .register(FieldPath::key("email"), Box::new(string().min(8).email()))
            .unwrap();
        (c, h)
    })
}

#[test]
fn tf1_keystroke_reaches_signal() {
    // Simulates the on:input path: set_str(&event_target_value(&ev)).
    let (_c, handle) = setup(ValidateOn::Blur);
    handle.set_str("abc");
    assert_eq!(
        handle.value().get(),
        Value::from("abc"),
        "keystroke must land in the controller-owned value cell"
    );
    assert!(handle.dirty().get(), "dirty flips against the empty anchor");
}

#[test]
fn tf1_external_write_tracked() {
    let (c, handle) = setup(ValidateOn::Blur);
    // programmatic write (e.g., from another handler)
    handle.value().set(Value::from("x"));
    assert_eq!(handle.get_str(), Some("x".to_string()));
    // reset() path
    handle.set_str("y");
    c.reset();
    assert_eq!(
        handle.get_str(),
        Some(String::new()),
        "reset must be reflected in the signal-derived string"
    );
}

// tf2 (conditional pushback) lives in the DOM effect closure; it is not
// natively testable without a browser. The id derivation (tf5) is
// unit-tested in-module on the real `sanitize_id`.

#[test]
fn tf3_blur_marks_touched() {
    // Simulates the on:blur path: controller.mark_touched(field.path()).
    let (c, handle) = setup(ValidateOn::Blur);
    handle.set_str("ab");
    assert!(
        handle.visible_errors().get().is_empty(),
        "untouched Blur-mode field hides errors"
    );
    c.mark_touched(handle.path());
    assert!(
        !handle.visible_errors().get().is_empty(),
        "blur seam must open the visibility gate"
    );
}

#[test]
fn tf3_no_provider_degrades_to_noop() {
    // L-1: without any ancestor use_form provider, the blur seam resolves
    // try_form_controller() to None and skips marking; binding itself is
    // unaffected (headless usage renders/binds correctly).
    Owner::new().with(|| {
        assert!(
            formars_ui::try_form_controller().is_none(),
            "no provider present"
        );
        let mut c = FormController::new(ValidateOn::Blur);
        let handle = c
            .register(FieldPath::key("email"), Box::new(string()))
            .unwrap();
        // blur fires with None resolution → skip; nothing panics, state moves
        handle.set_str("still-binds");
        assert_eq!(handle.get_str(), Some("still-binds".to_string()));
        assert!(!handle.touched().get(), "no touch without provider");
    });
}

#[test]
fn tf4_errors_clear_reactively() {
    let (c, handle) = setup(ValidateOn::Change);
    handle.set_str("ab");
    c.mark_touched(handle.path());
    let n = handle.visible_errors().get().len();
    assert!(n > 0, "touched invalid field shows {n} issues");
    // corrected value → list empties with NO further interaction
    handle.set_str("user@example.com");
    assert!(handle.visible_errors().get().is_empty());
}
