//! SC-8 coercion and DV-5 coerced-through-dyn scenarios.

use formars_core::coerce::coerced;
use formars_core::prelude::*;
use formars_core::schema::DynSchema;

#[test]
fn sc8_coercion_success() {
    assert_eq!(coerced::<u32>().parse(&"42".to_string()), Ok(42u32));
}

#[test]
fn sc8_coercion_failure_code() {
    let err = coerced::<u32>().parse(&"abc".to_string()).unwrap_err();
    assert_eq!(err.first().unwrap().code, IssueCode::Coerce);
    let msg = &err.first().unwrap().message;
    assert!(!msg.is_empty(), "non-garbage diagnostics expected");
    assert!(msg.contains("abc"), "message echoes the offending input");
}

#[test]
fn dv5_form_input_string_via_dyn_dynschema() {
    let s: Box<dyn DynSchema> = Box::new(coerced::<u32>());
    let issues = s.validate_value(&Value::String("42".into()));
    assert!(
        issues.is_empty(),
        "HTML-form string input validates via coercion"
    );

    let issues = s.validate_value(&Value::String("not-a-number".into()));
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::Coerce);
}

#[test]
fn dv5_coerced_type_mismatch_never_panics() {
    let s: Box<dyn DynSchema> = Box::new(coerced::<u32>());
    for v in [Value::Bool(true), Value::I64(42), Value::Null] {
        let issues = s.validate_value(&v);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, IssueCode::TypeMismatch);
    }
}
