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

/// Non-finite float spellings re-coerce (spec Domain 2): `f64::from_str`
/// accepts "NaN"/"inf"/"-inf" (case-insensitive, std), so the canonical
/// display renderings round-trip through `CoercedSchema::<f64>` coercion.
#[test]
fn sc8_nonfinite_float_spellings_recoerce_via_f64_from_str() {
    let nan = coerced::<f64>().parse(&"NaN".to_string()).unwrap();
    assert!(nan.is_nan(), "\"NaN\" coerces to a NaN value");

    let pos_inf = coerced::<f64>().parse(&"inf".to_string()).unwrap();
    assert!(
        pos_inf.is_infinite() && pos_inf.is_sign_positive(),
        "\"inf\" coerces to +infinity"
    );

    let neg_inf = coerced::<f64>().parse(&"-inf".to_string()).unwrap();
    assert!(
        neg_inf.is_infinite() && neg_inf.is_sign_negative(),
        "\"-inf\" coerces to -infinity"
    );
}
