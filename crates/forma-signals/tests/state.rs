//! FSM-3..FSM-7: touched/dirty transitions, memo↔schema parity, multi-error
//! accumulation, reset.

use forma_core::prelude::*;
use forma_signals::{FormController, ValidateOn};
use reactive_graph::traits::{Get, Set};

fn controller() -> FormController {
    FormController::new(ValidateOn::Blur)
}

#[test]
fn fsm3_touched_false_then_true_after_mark_touched() {
    let mut c = controller();
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    let h = c.field(&FieldPath::key("a")).unwrap();
    assert!(!h.touched().get());
    c.mark_touched(&FieldPath::key("a"));
    assert!(h.touched().get());
}

#[test]
fn fsm3_touched_survives_subsequent_value_change() {
    let mut c = controller();
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    c.mark_touched(&FieldPath::key("a"));
    let h = c.field(&FieldPath::key("a")).unwrap();
    h.value().set(Value::from("new"));
    assert!(h.touched().get());
}

#[test]
fn fsm4_mutation_marks_dirty() {
    let mut c = controller();
    c.register(FieldPath::key("name"), Box::new(string()))
        .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    assert_eq!(h.value().get(), Value::from(""), "fresh field starts empty");
    h.value().set(Value::from("ed"));
    assert!(h.dirty().get());
}

#[test]
fn fsm4_write_back_to_initial_clears_dirty() {
    let mut c = controller();
    c.register(FieldPath::key("name"), Box::new(string()))
        .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    h.value().set(Value::from("ed"));
    assert!(h.dirty().get());
    h.value().set(Value::from(""));
    assert!(!h.dirty().get());
}

#[test]
fn fsm4_reset_restores_registration_initial_and_clears_dirty() {
    let mut c = controller();
    c.register(FieldPath::key("name"), Box::new(string()))
        .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    h.value().set(Value::from("preset"));
    c.reset();
    let h = c.field(&FieldPath::key("name")).unwrap();
    assert_eq!(
        h.value().get(),
        Value::from(""),
        "reset restores the registration initial, not the last-set value"
    );
    assert!(!h.dirty().get(), "restored initial is clean");
    h.value().set(Value::from("other"));
    assert!(h.dirty().get());
}

#[test]
fn fsm4_register_initial_preset_lifecycle_dirty_tracks_snapshot() {
    let mut c = controller();
    c.register_initial(
        FieldPath::key("name"),
        Box::new(string()),
        Value::from("preset"),
    )
    .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    assert!(
        !h.dirty().get(),
        "field registered with preset must start clean"
    );
    h.value().set(Value::from("other"));
    assert!(h.dirty().get());
    h.value().set(Value::from("preset"));
    assert!(
        !h.dirty().get(),
        "write-back to the preset initial clears dirty"
    );
}

#[test]
fn fsm4_reset_anchors_to_the_registered_preset_value() {
    let mut c = controller();
    c.register_initial(
        FieldPath::key("name"),
        Box::new(string()),
        Value::from("preset"),
    )
    .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    h.value().set(Value::from("other"));
    assert!(h.dirty().get());
    c.reset();
    let h = c.field(&FieldPath::key("name")).unwrap();
    assert_eq!(
        h.value().get(),
        Value::from("preset"),
        "reset restores preset"
    );
    assert!(!h.dirty().get());
}

#[test]
fn fsm4_legacy_register_keeps_empty_string_anchor() {
    let mut c = controller();
    c.register(FieldPath::key("name"), Box::new(string()))
        .unwrap();
    let h = c.field(&FieldPath::key("name")).unwrap();
    assert_eq!(h.value().get(), Value::from(""), "legacy anchor unchanged");
    h.value().set(Value::from("edited"));
    assert!(h.dirty().get());
}

#[test]
fn fsm5_error_memo_tracks_value_edits() {
    let mut c = controller();
    c.register(FieldPath::key("email"), Box::new(string().min(8).email()))
        .unwrap();
    let h = c.field(&FieldPath::key("email")).unwrap();
    h.value().set(Value::from("ab"));
    let codes: Vec<_> = h.errors().get().iter().map(|i| i.code.clone()).collect();
    assert!(
        codes.contains(&IssueCode::Min),
        "min violation expected, got {codes:?}"
    );
    h.value().set(Value::from("user@example.com"));
    assert!(h.errors().get().is_empty());
}

#[test]
fn fsm5_memo_matches_direct_schema_validate_value() {
    let schema = string().min(3).email();
    let mut c = controller();
    c.register(FieldPath::key("email"), Box::new(schema.clone()))
        .unwrap();
    let h = c.field(&FieldPath::key("email")).unwrap();
    let v = Value::from("nope");
    h.value().set(v.clone());
    let direct = crate_stamp(&schema, &v);
    assert_eq!(h.errors().get(), direct);
}

/// Validates `v` against `schema` and stamps ROOT issues with the field path,
/// mirroring the controller's internal seam.
fn crate_stamp(schema: &dyn DynSchema, v: &Value) -> Vec<FormaIssue> {
    schema
        .validate_value(v)
        .into_iter()
        .map(|mut i| {
            if i.path == FieldPath::ROOT {
                i.path = FieldPath::key("email");
            }
            i
        })
        .collect()
}

#[test]
fn fsa3_string_setter_validates_equivalent_to_manual_value() {
    let mut c = controller();
    c.register(FieldPath::key("age"), Box::new(coerced::<u32>()))
        .unwrap();
    let h = c.field(&FieldPath::key("age")).unwrap();
    h.set_str("42");
    assert_eq!(h.get_str(), Some("42".to_string()));
    let via_setter = h.errors().get();

    let mut manual = controller();
    manual
        .register(FieldPath::key("age"), Box::new(coerced::<u32>()))
        .unwrap();
    let mh = manual.field(&FieldPath::key("age")).unwrap();
    mh.value().set(Value::from("42"));
    assert_eq!(via_setter, mh.errors().get(), "setter must be equivalent");
    assert!(
        via_setter.is_empty(),
        "valid string must pass the coerced schema"
    );
}

#[test]
fn fsa3_typed_setters_construct_exact_variants() {
    let mut c = controller();
    c.register(FieldPath::key("n"), Box::new(number::<i64>()))
        .unwrap();
    c.register(FieldPath::key("f"), Box::new(number::<f64>()))
        .unwrap();
    c.register(FieldPath::key("b"), Box::new(bool())).unwrap();
    let hn = c.field(&FieldPath::key("n")).unwrap();
    let hf = c.field(&FieldPath::key("f")).unwrap();
    let hb = c.field(&FieldPath::key("b")).unwrap();
    hn.set_i64(7);
    hf.set_f64(1.5);
    hb.set_bool(true);
    assert_eq!(hn.value().get(), Value::I64(7));
    assert_eq!(hf.value().get(), Value::F64(1.5));
    assert_eq!(hb.value().get(), Value::Bool(true));
}

#[test]
fn fsm6_two_violations_accumulated_in_declaration_order() {
    let mut c = controller();
    c.register(FieldPath::key("email"), Box::new(string().min(10).email()))
        .unwrap();
    let h = c.field(&FieldPath::key("email")).unwrap();
    h.value().set(Value::from("ab"));
    let codes: Vec<_> = h.errors().get().iter().map(|i| i.code.clone()).collect();
    assert_eq!(
        codes,
        vec![IssueCode::Min, IssueCode::Email],
        "declaration order expected"
    );
}

#[test]
fn fsm7_reset_restores_untouched_clean_initial_consistent() {
    let mut c = controller();
    c.register(FieldPath::key("a"), Box::new(string().min(5)))
        .unwrap();
    let path = FieldPath::key("a");
    c.mark_touched(&path);
    c.field(&path).unwrap().value().set(Value::from("ab"));
    let h = c.field(&path).unwrap();
    assert!(h.touched().get() && h.dirty().get() && !h.errors().get().is_empty());

    c.reset();
    let h = c.field(&path).unwrap();
    assert!(!h.touched().get(), "reset clears touched");
    assert!(!h.dirty().get(), "reset restores initial");
    assert_eq!(
        h.errors().get(),
        string()
            .min(5)
            .validate_value(&Value::from(""))
            .into_iter()
            .map(|mut i| {
                i.path = path.clone();
                i
            })
            .collect::<Vec<_>>(),
        "errors consistent with restored initial value"
    );
}
