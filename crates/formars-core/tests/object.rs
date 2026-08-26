//! OS-1..OS-3, ER-2, ER-4, ER-6 object-schema behavior (oracle: delta spec
//! `sdd/formars-core-object-schema/spec` #1623).

use formars_core::error::{FieldPath, Segment};
use formars_core::prelude::*;
use formars_core::schema::{DynSchema, Schema, ShapeKind as K};
use formars_core::value::{Object, ToValue, Value};

fn obj(pairs: &[(&str, Value)]) -> Object {
    let mut o = Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    o
}

fn paths(err: &FormaError) -> Vec<String> {
    err.issues.iter().map(|i| i.path.to_string()).collect()
}

// ---------------------------------------------------------------- OS-1/OS-2

#[test]
fn os2_successful_parse_reconstructs_in_declaration_order() {
    let schema = object()
        .field("name", string())
        .field("age", formars_core::coerce::coerced::<u32>());
    let input = obj(&[("age", Value::from("42")), ("name", Value::from("Ada"))]);
    let out = schema.parse(&input).expect("valid input must parse");
    let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_ref()).collect();
    assert_eq!(keys, vec!["name", "age"], "declaration order wins");
    let vals: Vec<Value> = out.iter().map(|(_, v)| v.clone()).collect();
    assert_eq!(vals[0], Value::String("Ada".into()));
    assert_eq!(
        vals[1],
        42u32.to_value(),
        "coerced result becomes Value::I64(42)"
    );
}

#[test]
fn os2_unknown_keys_excluded_from_output() {
    let schema = object()
        .field("name", string())
        .field("age", formars_core::coerce::coerced::<u32>());
    let input = obj(&[
        ("age", Value::from("42")),
        ("extra", Value::from("junk")),
        ("name", Value::from("Ada")),
    ]);
    let out = schema.parse(&input).expect("valid input must parse");
    let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_ref()).collect();
    assert_eq!(keys, vec!["name", "age"]);
    assert!(!keys.contains(&"extra"), "unknown key must not appear");
}

#[test]
fn os1_any_child_composition_validates() {
    let schema = object()
        .field("s", string().min(3))
        .field("n", formars_core::coerce::coerced::<u32>())
        .field("sub", object().field("b", formars_core::types::bool()));
    let input = obj(&[
        ("s", Value::from("abc")),
        ("n", Value::from("7")),
        ("sub", Value::Object(obj(&[("b", Value::Bool(true))]))),
    ]);
    assert!(
        schema.parse(&input).is_ok(),
        "string / coerced / nested object children all validate"
    );
}

#[test]
fn os1_metadata_slot_accessible_through_object() {
    let schema = object().field("name", string().label("Full name"));
    let meta = schema.field_meta("name").expect("declared key found");
    assert_eq!(meta.label.as_deref(), Some("Full name"));
    assert!(schema.field_meta("absent").is_none());
}

// --------------------------------------------------------------------- ER-6

#[test]
fn er6_absent_key_yields_exactly_one_required_at_joined_path() {
    let schema = object().field("email", string().min(3));
    let err = schema.parse(&Object::new()).unwrap_err();
    assert_eq!(err.issues.len(), 1, "exactly one issue for absent key");
    let first = err.first().unwrap();
    assert_eq!(first.code, IssueCode::Required);
    assert_eq!(first.path, FieldPath::key("email"));
    assert_eq!(first.path.to_string(), "email");
}

#[test]
fn er6_null_is_present_not_missing_child_issue_surfaces_at_field_path() {
    let schema = object().field("age", formars_core::coerce::coerced::<u32>());
    let input = obj(&[("age", Value::Null)]);
    let err = schema.parse(&input).unwrap_err();
    assert!(
        !err.issues.iter().any(|i| i.code == IssueCode::Required),
        "Required must NOT fire for a present Null"
    );
    let first = err.first().unwrap();
    assert_eq!(first.path, FieldPath::key("age"));
}

#[test]
fn er6_null_handed_to_coerced_child_reports_type_mismatch() {
    // Pin the exact child outcome: the coerced bridge is string-only, so
    // Null is a TypeMismatch at the field path (never Required).
    let schema = object().field("age", formars_core::coerce::coerced::<u32>());
    let input = obj(&[("age", Value::Null)]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(err.issues[0].path.to_string(), "age");
}

#[test]
fn er6_wrong_typed_present_value_reports_child_codes_only() {
    let schema = object().field("name", string().min(3));
    let input = obj(&[("name", Value::I64(7))]);
    let err = schema.parse(&input).unwrap_err();
    assert!(
        !err.issues.iter().any(|i| i.code == IssueCode::Required),
        "wrong-typed present value never yields Required"
    );
    assert!(
        err.issues.iter().all(|i| i.code == IssueCode::TypeMismatch),
        "child's own codes only, got {:?}",
        err.issues
            .iter()
            .map(|i| i.code.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn er6_unknown_keys_ignored_v0_policy() {
    let schema = object().field("a", string());
    let input = obj(&[("a", Value::from("ok")), ("b", Value::F64(f64::NAN))]);
    assert!(
        schema.parse(&input).is_ok(),
        "unknown keys produce zero issues in v0"
    );
}

// --------------------------------------------------------------------- ER-2

#[test]
fn er2_child_issue_carries_joined_path_and_is_retrievable() {
    let schema = object().field("user", object().field("email", string().email()));
    let input = obj(&[(
        "user",
        Value::Object(obj(&[("email", Value::from("not-an-email"))])),
    )]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].path.to_string(), "user.email");

    let joined = FieldPath::key("user").join(Segment::Key("email".into()));
    let got: Vec<_> = err.issues_for(&joined).collect();
    assert_eq!(got.len(), 1, "issues_for retrieves the joined path");
    assert_eq!(got[0].code, IssueCode::Email);
}

#[test]
fn er2_two_level_nesting_renders_dot_joined() {
    let schema = object().field(
        "outer",
        object().field("inner", object().field("field", string().nonempty())),
    );
    let input = obj(&[(
        "outer",
        Value::Object(obj(&[(
            "inner",
            Value::Object(obj(&[("field", Value::from(""))])),
        )])),
    )]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(paths(&err), vec!["outer.inner.field"]);
}

#[test]
fn er2_top_level_primitives_still_report_root() {
    let err = string().min(3).parse(&"ab".to_string()).unwrap_err();
    assert_eq!(err.issues[0].path, FieldPath::ROOT);
    assert_eq!(err.issues[0].path.to_string(), "");
}

// --------------------------------------------------------------------- ER-4

#[test]
fn er4_cross_field_fail_fast_returns_exactly_one_issue_at_first_field() {
    let schema = object()
        .fail_fast()
        .field("a", string().min(10))
        .field("b", string().min(10));
    let input = obj(&[("a", Value::from("x")), ("b", Value::from("y"))]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(err.issues.len(), 1, "first-issue-anywhere-stops");
    assert_eq!(err.issues[0].path.to_string(), "a");
}

#[test]
fn er4_fail_fast_descends_into_non_fail_fast_child() {
    // inner.x fails two constraints but the inner string schema has no
    // fail_fast of its own — the object's flag must still stop everything.
    let schema = object()
        .fail_fast()
        .field("ok", string())
        .field("inner", object().field("x", string().min(10).email()));
    let input = obj(&[
        ("ok", Value::from("fine")),
        ("inner", Value::Object(obj(&[("x", Value::from("ab"))]))),
    ]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(
        err.issues.len(),
        1,
        "ONE issue despite child accumulating 2"
    );
    assert_eq!(err.issues[0].path.to_string(), "inner.x");
    assert_eq!(err.issues[0].code, IssueCode::Min);
}

#[test]
fn er4_without_fail_fast_fields_accumulate_in_declaration_order() {
    let schema = object()
        .field("z", string().min(10))
        .field("a", string().min(10));
    let input = obj(&[("z", Value::from("x")), ("a", Value::from("y"))]);
    for _ in 0..50 {
        let err = schema.parse(&input).unwrap_err();
        assert_eq!(
            paths(&err),
            vec!["z", "a"],
            "declaration order, not alphabetical"
        );
    }
}

#[test]
fn er4_absent_key_under_fail_fast_stops_entirely() {
    let schema = object()
        .fail_fast()
        .field("a", string())
        .field("b", string());
    let input = obj(&[("b", Value::from("present"))]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::Required);
    assert_eq!(err.issues[0].path.to_string(), "a");
}

// --------------------------------------------------------------------- DV-6

#[test]
fn dv6_shape_reports_object_with_declared_fields_in_order() {
    let boxed: Box<dyn DynSchema> = Box::new(
        object()
            .field("name", string())
            .field("age", formars_core::coerce::coerced::<u32>()),
    );
    let shape = boxed.shape();
    match &shape.kind {
        K::Object { fields } => {
            let keys: Vec<&str> = fields.iter().map(|f| f.key.as_ref()).collect();
            assert_eq!(keys, vec!["name", "age"]);
            assert!(matches!(fields[0].child.kind, K::Str));
            assert!(matches!(fields[1].child.kind, K::Coerced));
        }
        other => panic!("expected Object shape kind, got {other:?}"),
    }
}

// --------------------------------------------------------------------- OS-3

#[test]
fn os3_three_level_nesting_succeeds_preserving_all_levels() {
    let schema = object().field(
        "a",
        object().field("b", object().field("c", formars_core::types::bool())),
    );
    let input = obj(&[(
        "a",
        Value::Object(obj(&[(
            "b",
            Value::Object(obj(&[("c", Value::Bool(true))])),
        )])),
    )]);
    let out = schema.parse(&input).expect("three-level valid tree parses");

    let level_a_keys: Vec<&str> = out.iter().map(|(k, _)| k.as_ref()).collect();
    assert_eq!(level_a_keys, vec!["a"]);

    let Some(Value::Object(b)) = out.get("a") else {
        panic!("nested object expected at 'a'");
    };
    let b_keys: Vec<&str> = b.iter().map(|(k, _)| k.as_ref()).collect();
    assert_eq!(b_keys, vec!["b"]);
    let Some(Value::Object(c)) = b.get("b") else {
        panic!("nested object expected at 'a.b'");
    };
    assert_eq!(c.get("c"), Some(&Value::Bool(true)));
}

#[test]
fn os3_nested_absent_key_reports_required_at_depth() {
    let schema = object().field(
        "outer",
        object().field("qty", formars_core::coerce::coerced::<u32>()),
    );
    // inner provides an object WITHOUT qty.
    let input = obj(&[("outer", Value::Object(Object::new()))]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::Required);
    assert_eq!(err.issues[0].path.to_string(), "outer.qty");
}

// ------------------------------------------------------------------ T1 (ER-6)

#[test]
fn t1_scalar_into_declared_object_field_yields_one_type_mismatch_at_joined_path() {
    let schema = object().field("child", object().field("x", string()));
    let input = obj(&[("child", Value::I64(7))]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(
        err.issues.len(),
        1,
        "exactly one issue for a scalar into a declared-object field"
    );
    assert_eq!(err.issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(err.issues[0].path.to_string(), "child");
    assert_ne!(err.issues[0].path, FieldPath::ROOT);
}

#[test]
fn t1_scalar_into_declared_object_field_under_parent_fail_fast_stops_at_first() {
    let schema = object()
        .fail_fast()
        .field("first", string().min(10))
        .field("child", object().field("x", string()));
    let input = obj(&[
        ("first", Value::from("too-short")),
        ("child", Value::I64(7)),
    ]);
    let err = schema.parse(&input).unwrap_err();
    assert_eq!(err.issues.len(), 1, "fail-fast stops before reaching child");
    assert_eq!(err.issues[0].path.to_string(), "first");

    // Same schema, only the child violated: exactly ONE TypeMismatch, no
    // accumulation beyond it.
    let input2 = obj(&[
        ("first", Value::from("long-enough-name")),
        ("child", Value::I64(7)),
    ]);
    let err2 = schema.parse(&input2).unwrap_err();
    assert_eq!(err2.issues.len(), 1);
    assert_eq!(err2.issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(err2.issues[0].path.to_string(), "child");
}

// ------------------------------------------------------------- F1 clone contract

fn rich_schema() -> formars_core::types::ObjectSchema {
    object()
        .field("sub", object().field("email", string().email()))
        .field("name", string().min(3).refine(|s: &str| s.contains('a')))
        .field("age", formars_core::coerce::coerced::<u32>())
}

fn sample_inputs() -> Vec<Object> {
    vec![
        obj(&[
            (
                "sub",
                Value::Object(obj(&[("email", Value::from("a@b.c"))])),
            ),
            ("name", Value::from("ada")),
            ("age", Value::from("42")),
        ]),
        Object::new(),
        obj(&[
            ("sub", Value::Object(obj(&[("email", Value::from("nope"))]))),
            ("name", Value::I64(7)),
            ("age", Value::Null),
        ]),
    ]
}

#[test]
fn f1_clone_validates_identically_to_source() {
    let original = rich_schema();
    let clone = original.clone();
    for input in sample_inputs() {
        let a = original.parse(&input);
        let b = clone.parse(&input);
        match (a, b) {
            (Ok(pa), Ok(pb)) => {
                assert_eq!(
                    pa.iter().map(|(k, _)| k.as_ref()).collect::<Vec<_>>(),
                    pb.iter().map(|(k, _)| k.as_ref()).collect::<Vec<_>>(),
                );
                assert_eq!(pa, pb, "Ok payloads must be equal");
            }
            (Err(ea), Err(eb)) => {
                assert_eq!(
                    ea.issues, eb.issues,
                    "Err issue vectors equal INCLUDING ORDER"
                );
            }
            (a, b) => panic!("original and clone disagree: {a:?} vs {b:?}"),
        }
    }
}

#[test]
fn f1_builder_reuse_isolation_between_clones() {
    fn declared_keys(s: &formars_core::types::ObjectSchema) -> Vec<String> {
        match &s.shape().kind {
            K::Object { fields } => fields.iter().map(|f| f.key.to_string()).collect::<Vec<_>>(),
            other => panic!("expected Object kind, got {other:?}"),
        }
    }

    let base = object().field("name", string());
    let variant_a = base.clone().field("only_a", string());
    let variant_b = base.clone().field("only_b", string());

    assert_eq!(declared_keys(&variant_a), vec!["name", "only_a"]);
    assert_eq!(declared_keys(&variant_b), vec!["name", "only_b"]);

    // Pre-clone schema untouched by post-clone builder calls.
    let base_keys = {
        let boxed: Box<dyn DynSchema> = Box::new(base);
        let keys: Vec<String> = match &boxed.shape().kind {
            K::Object { fields } => fields.iter().map(|f| f.key.to_string()).collect(),
            other => panic!("expected Object kind, got {other:?}"),
        };
        keys
    };
    assert_eq!(base_keys, vec!["name"]);
}

#[test]
fn f1_shape_cache_correct_across_clone() {
    let original = rich_schema();
    let original_shape = original.shape().clone(); // populate the OnceLock FIRST
    let clone = original.clone();
    assert_eq!(
        &original_shape,
        clone.shape(),
        "fresh cache re-derives an identical tree"
    );
}
