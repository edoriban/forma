//! FSV-1..FSV-6: validation timing modes, sync revalidation, display gate.

use forma_core::prelude::*;
use forma_signals::{FormController, ValidateOn};
use reactive_graph::traits::{Get, Set};

fn blur_form() -> FormController {
    FormController::new(ValidateOn::Blur)
}

#[test]
fn fsv1_controller_default_applies_unless_overridden() {
    let mut c = blur_form();
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    c.register_with(FieldPath::key("b"), Box::new(string()), ValidateOn::Change)
        .unwrap();
    assert_eq!(
        c.effective_validate_on(&FieldPath::key("a")),
        Some(ValidateOn::Blur),
        "unoverridden field uses controller default"
    );
    assert_eq!(
        c.effective_validate_on(&FieldPath::key("b")),
        Some(ValidateOn::Change),
        "override wins over controller default"
    );
}

#[test]
fn fsv2_revalidate_effect_synchronous_within_call() {
    let mut c = blur_form();
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let h = c.field(&FieldPath::key("email")).unwrap();
    h.value().set(Value::from("ab"));
    c.revalidate(&FieldPath::key("email"));
    assert!(
        !h.errors().get().is_empty(),
        "violation observable immediately after revalidate returns"
    );
}

#[test]
fn fsv3_untouched_blur_field_hides_invalid_errors() {
    let mut c = blur_form();
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let h = c.field(&FieldPath::key("email")).unwrap();
    h.value().set(Value::from("ab"));
    assert!(
        h.visible_errors().get().is_empty(),
        "untouched Blur field must hide issues"
    );
}

#[test]
fn fsv3_mark_touched_reveals_immediately() {
    let mut c = blur_form();
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let path = FieldPath::key("email");
    c.field(&path).unwrap().value().set(Value::from("ab"));
    c.mark_touched(&path);
    let h = c.field(&path).unwrap();
    assert!(
        !h.visible_errors().get().is_empty(),
        "touch reveals issues with no further calls"
    );
}

#[test]
fn fsv4_change_mode_memo_tracks_each_edit() {
    let mut c = FormController::new(ValidateOn::Change);
    c.register(FieldPath::key("name"), Box::new(string().min(3)))
        .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    h.value().set(Value::from("ab"));
    assert_eq!(h.errors().get().len(), 1);
    h.value().set(Value::from("abc"));
    assert_eq!(h.errors().get().len(), 0);
    h.value().set(Value::from("a"));
    assert_eq!(h.errors().get().len(), 1);
}

#[test]
fn fsv5_submit_mode_edits_stay_hidden_until_submit_attempt() {
    let mut c = FormController::new(ValidateOn::Submit);
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let h = c.field(&FieldPath::key("email")).unwrap();
    for v in ["ab", "abc", "a"] {
        h.value().set(Value::from(v));
        assert!(
            h.visible_errors().get().is_empty(),
            "pre-submit edits stay hidden ({v})"
        );
    }
    c.submitted().set(true);
    assert!(
        !h.visible_errors().get().is_empty(),
        "submit attempt reveals persistent violation"
    );
}

#[test]
fn fsv6_gate_truth_table_change_blur_touched_or_submitted() {
    let mut c = blur_form();
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let path = FieldPath::key("email");
    c.field(&path).unwrap().value().set(Value::from("ab"));
    let h = c.field(&path).unwrap();
    assert!(
        h.visible_errors().get().is_empty(),
        "touched=false => hidden"
    );
    c.mark_touched(&path);
    assert!(
        !h.visible_errors().get().is_empty(),
        "touched=true => shown"
    );
}

#[test]
fn fsv6_gate_truth_table_submit_mode_submitted_only() {
    let mut c = FormController::new(ValidateOn::Submit);
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let path = FieldPath::key("email");
    c.field(&path).unwrap().value().set(Value::from("ab"));
    c.mark_touched(&path);
    let h = c.field(&path).unwrap();
    assert!(
        h.visible_errors().get().is_empty(),
        "Submit mode: touch alone must not reveal"
    );
    c.submitted().set(true);
    assert!(
        !h.visible_errors().get().is_empty(),
        "Submit mode: submitted reveals even untouched-by-blur fields"
    );
}
