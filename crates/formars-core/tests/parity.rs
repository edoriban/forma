//! SC-9/DV-2 dual-view invariant matrix: typed `parse` vs erased `validate_value`.

use formars_core::prelude::*;
use formars_core::types::{BoolSchema, NumberSchema, StringSchema};

fn check_typed<S>(name: &str, desc: &str, valid: bool, value: &Value, input: &S::Input, schema: &S)
where
    S: Schema + DynSchema,
{
    let typed = schema.parse(input);
    let erased = schema.validate_value(value);

    match (valid, &typed) {
        (true, Ok(_)) => {
            assert!(
                erased.is_empty(),
                "PARITY {name}/{desc}: valid input but erased view reported {erased:?}"
            );
        }
        (false, Err(e)) => {
            assert!(
                !e.issues.is_empty(),
                "PARITY {name}/{desc}: failing parse returned ZERO issues"
            );
            assert_eq!(
                e.issues, erased,
                "PARITY MISMATCH {name}/{desc}:\n  typed  : {:#?}\n  erased : {:#?}",
                e.issues, erased
            );
        }
        (false, Ok(_)) => panic!("PARITY {name}/{desc}: expected rejection, typed parse succeeded"),
        (true, Err(e)) => panic!("PARITY {name}/{desc}: expected acceptance, got {e:?}"),
    }
}

fn check_erased(name: &str, desc: &str, valid: bool, value: &Value, schema: &dyn DynSchema) {
    let issues = schema.validate_value(value);
    if valid {
        assert!(
            issues.is_empty(),
            "ERASED {name}/{desc}: expected acceptance, got {issues:?}"
        );
    } else {
        assert!(
            !issues.is_empty(),
            "ERASED {name}/{desc}: expected rejection"
        );
    }
}

#[test]
fn sc9_string_family_parity() {
    let table: Vec<(&str, StringSchema, &'static str, Value, String, bool)> = vec![
        (
            "min3/ok",
            string().min(3),
            "ok",
            Value::from("abc"),
            "abc".to_string(),
            true,
        ),
        (
            "min3/short",
            string().min(3),
            "short",
            Value::from("ab"),
            "ab".to_string(),
            false,
        ),
        (
            "trim_max3/padded_ok",
            string().trim().max(3),
            "padded",
            Value::from(" ab "),
            " ab ".to_string(),
            true,
        ),
        (
            "max3_trim/padded_over",
            string().max(3).trim(),
            "padded",
            Value::from(" abcd "),
            " abcd ".to_string(),
            false,
        ),
        (
            "nonempty/empty",
            string().nonempty(),
            "empty",
            Value::from(""),
            String::new(),
            false,
        ),
        (
            "min10_email/both_fail",
            string().min(10).email(),
            "ab",
            Value::from("ab"),
            "ab".to_string(),
            false,
        ),
        (
            "email/ok",
            string().email(),
            "ok",
            Value::from("user@example.com"),
            "user@example.com".to_string(),
            true,
        ),
    ];
    for (name, schema, iname, value, typed_input, valid) in table {
        check_typed(name, iname, valid, &value, &typed_input, &schema);
        let boxed: Box<dyn DynSchema> = Box::new(schema.clone());
        check_erased(name, iname, valid, &value, boxed.as_ref());
        let _ = &schema;
    }
}

type NumRowI64 = (
    &'static str,
    NumberSchema<i64>,
    &'static str,
    Value,
    i64,
    bool,
);
type NumRow = (
    &'static str,
    NumberSchema<f64>,
    &'static str,
    Value,
    f64,
    bool,
);

#[test]
fn sc9_number_f64_parity() {
    let table: Vec<NumRow> = vec![
        (
            "plain/ok",
            number::<f64>(),
            "ok",
            Value::F64(6.0),
            6.0,
            true,
        ),
        (
            "finite/nan",
            number::<f64>().finite(),
            "nan",
            Value::F64(f64::NAN),
            f64::NAN,
            false,
        ),
        (
            "finite/inf",
            number::<f64>().finite(),
            "inf",
            Value::F64(f64::INFINITY),
            f64::INFINITY,
            false,
        ),
        (
            "positive/zero",
            number::<f64>().positive(),
            "zero",
            Value::F64(0.0),
            0.0,
            false,
        ),
        (
            "int/frac",
            number::<f64>().int(),
            "frac",
            Value::F64(2.5),
            2.5,
            false,
        ),
        (
            "min5/below",
            number::<f64>().min(5.0),
            "below",
            Value::F64(1.0),
            1.0,
            false,
        ),
        (
            "min5/above",
            number::<f64>().min(5.0),
            "above",
            Value::F64(5.0),
            5.0,
            true,
        ),
    ];
    for (name, schema, iname, value, typed_input, valid) in table {
        check_typed(name, iname, valid, &value, &typed_input, &schema);
        let boxed: Box<dyn DynSchema> = Box::new(schema.clone());
        check_erased(name, iname, valid, &value, boxed.as_ref());
        let _ = &schema;
    }
}

#[test]
fn sc9_number_i64_parity() {
    let table: Vec<NumRowI64> = vec![
        ("plain/ok", number::<i64>(), "ok", Value::I64(11), 11, true),
        (
            "positive/zero",
            number::<i64>().positive(),
            "zero",
            Value::I64(0),
            0,
            false,
        ),
        (
            "min10/below",
            number::<i64>().min(10),
            "below",
            Value::I64(9),
            9,
            false,
        ),
        (
            "min10/max",
            number::<i64>().min(10),
            "max",
            Value::I64(i64::MAX),
            i64::MAX,
            true,
        ),
    ];
    for (name, schema, iname, value, typed_input, valid) in table {
        check_typed(name, iname, valid, &value, &typed_input, &schema);
        let boxed: Box<dyn DynSchema> = Box::new(schema.clone());
        check_erased(name, iname, valid, &value, boxed.as_ref());
        let _ = &schema;
    }
}

#[test]
fn sc9_bool_parity() {
    let table: Vec<(&str, BoolSchema, &'static str, Value, bool, bool)> = vec![
        (
            "equals_true/true",
            bool().equals(true),
            "true",
            Value::Bool(true),
            true,
            true,
        ),
        (
            "equals_true/false",
            bool().equals(true),
            "false",
            Value::Bool(false),
            false,
            false,
        ),
        (
            "equals_false/false",
            bool().equals(false),
            "false",
            Value::Bool(false),
            false,
            true,
        ),
        (
            "equals_false/true",
            bool().equals(false),
            "true",
            Value::Bool(true),
            true,
            false,
        ),
        ("plain/true", bool(), "true", Value::Bool(true), true, true),
    ];
    for (name, schema, iname, value, typed_input, valid) in table {
        check_typed(name, iname, valid, &value, &typed_input, &schema);
        let boxed: Box<dyn DynSchema> = Box::new(schema.clone());
        check_erased(name, iname, valid, &value, boxed.as_ref());
        let _ = &schema;
    }
}

#[test]
fn dv2_shape_introspection_through_erasure() {
    let s: Box<dyn DynSchema> = Box::new(string().min(3));
    let shape = s.shape();
    assert_eq!(shape.kind, formars_core::schema::ShapeKind::Str);
    assert_eq!(shape.constraints.len(), 1);
    assert_eq!(shape.constraints[0].code, IssueCode::Min);

    let n: Box<dyn DynSchema> = Box::new(number::<f64>().finite().int());
    assert_eq!(
        n.shape().kind,
        formars_core::schema::ShapeKind::Number { integer: false }
    );
    assert_eq!(n.shape().constraints.len(), 2);

    let b: Box<dyn DynSchema> = Box::new(bool().equals(true));
    assert_eq!(b.shape().kind, formars_core::schema::ShapeKind::Bool);
}

#[test]
fn dv3_metadata_accessible_through_erasure() {
    let s: Box<dyn DynSchema> = Box::new(
        string()
            .label("Email")
            .description("work address")
            .placeholder("you@x.com"),
    );
    let meta = s.metadata();
    assert_eq!(meta.label.as_deref(), Some("Email"));
    assert_eq!(meta.description.as_deref(), Some("work address"));
    assert_eq!(meta.placeholder.as_deref(), Some("you@x.com"));
}

#[test]
fn er4_fail_fast_single_issue_vs_accumulate() {
    let accumulate = string().min(10).email();
    let fast = string().min(10).email().fail_fast();
    let input = "ab";

    let slow_err = accumulate.parse(&input.to_string()).unwrap_err();
    assert_eq!(
        slow_err.issues.len(),
        2,
        "accumulate mode collects both builtin issues"
    );

    let fast_err = fast.parse(&input.to_string()).unwrap_err();
    assert_eq!(
        fast_err.issues.len(),
        1,
        "fail-fast yields EXACTLY ONE issue"
    );
    assert_eq!(
        fast_err.issues[0].code,
        IssueCode::Min,
        "first violated constraint wins"
    );
}

#[test]
fn er5_order_stability_across_repeated_parses() {
    let s = string().min(10).email();
    let expected: Vec<IssueCode> = vec![IssueCode::Min, IssueCode::Email];
    for _ in 0..50 {
        let got: Vec<IssueCode> = s
            .parse(&"ab".to_string())
            .unwrap_err()
            .issues
            .into_iter()
            .map(|i| i.code)
            .collect();
        assert_eq!(got, expected);
    }
}

#[test]
fn sc9_refine_and_failfast_parity() {
    let table: Vec<(&str, StringSchema, Value, String, bool)> = vec![
        (
            "refine_fail",
            string().min(2).refine(|s| s.contains('x')),
            Value::from("abc"),
            "abc".to_string(),
            false,
        ),
        (
            "refine_pass",
            string().min(2).refine(|s| s.contains('c')),
            Value::from("abc"),
            "abc".to_string(),
            true,
        ),
        (
            "ff_min_email",
            string().min(10).email().fail_fast(),
            Value::from("ab"),
            "ab".to_string(),
            false,
        ),
        (
            "two_refines",
            string().refine(|_| false).refine(|_| false),
            Value::from("q"),
            "q".to_string(),
            false,
        ),
    ];
    for (name, schema, value, typed_input, valid) in table {
        check_typed(name, "p4", valid, &value, &typed_input, &schema);
        // Clone preserves refinements (Arc-shared rules), so the erased view
        // is built from a clone instead of a hand-rebuilt schema.
        let boxed: Box<dyn DynSchema> = Box::new(schema.clone());
        check_erased(name, "p4", valid, &value, boxed.as_ref());
    }
}

#[test]
fn dv5_coerced_parity_through_erasure() {
    let schema = formars_core::coerce::coerced::<u32>();
    for (raw, valid) in [("42", true), ("abc", false)] {
        let value = Value::from(raw);
        check_typed("coerced", raw, valid, &value, &raw.to_string(), &schema);
        let boxed: Box<dyn DynSchema> = Box::new(formars_core::coerce::coerced::<u32>());
        check_erased("coerced", raw, valid, &value, boxed.as_ref());
    }
}

// ------------------------------------------------- DV-7/SC-9 object parity

fn obj(pairs: &[(&str, Value)]) -> formars_core::value::Object {
    let mut o = formars_core::value::Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    o
}

fn assert_object_parity(
    schema: &formars_core::types::ObjectSchema,
    input: &formars_core::value::Object,
) {
    use formars_core::schema::Schema;
    let value = Value::Object(input.clone());
    let typed = schema.parse(input).expect_err("expected rejection");
    let erased = schema.validate_value(&value);
    assert!(
        !typed.issues.is_empty(),
        "DV7: failing parse must carry issues"
    );
    assert_eq!(
        typed.issues, erased,
        "DV7 PARITY MISMATCH:\n  typed  : {:#?}\n  erased : {:#?}",
        typed.issues, erased
    );
}

#[test]
fn dv7_flat_object_failing_field_parity() {
    let schema = object().field("email", string().min(5));
    assert_object_parity(&schema, &obj(&[("email", Value::from("ab"))]));
}

#[test]
fn dv7_nested_object_of_object_parity() {
    let schema = object().field("outer", object().field("inner", string().nonempty()));
    assert_object_parity(
        &schema,
        &obj(&[("outer", Value::Object(obj(&[("inner", Value::from(""))])))]),
    );
}

#[test]
fn dv7_fail_fast_object_parity() {
    let schema = object()
        .fail_fast()
        .field("a", string().min(10))
        .field("b", string().min(10));
    assert_object_parity(
        &schema,
        &obj(&[("a", Value::from("x")), ("b", Value::from("y"))]),
    );
}

#[test]
fn dv7_absent_key_required_parity() {
    let schema = object().field("email", string().min(3));
    assert_object_parity(&schema, &formars_core::value::Object::new());
}

#[test]
fn dv7_null_present_parity() {
    let schema = object().field("age", formars_core::coerce::coerced::<u32>());
    assert_object_parity(&schema, &obj(&[("age", Value::Null)]));
}

#[test]
fn dv7_erased_non_object_yields_type_mismatch_at_root() {
    let boxed: Box<dyn DynSchema> = Box::new(object().field("x", string()));
    let issues = boxed.validate_value(&Value::String("nope".into()));
    assert_eq!(issues.len(), 1, "exactly one TypeMismatch");
    assert_eq!(issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(issues[0].path, FieldPath::ROOT);
}

#[test]
fn dv7_metadata_slot_unchanged_through_erasure() {
    let boxed: Box<dyn DynSchema> = Box::new(object().label("User").description("account fields"));
    let meta = boxed.metadata();
    assert_eq!(meta.label.as_deref(), Some("User"));
    assert_eq!(meta.description.as_deref(), Some("account fields"));
}
