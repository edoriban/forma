//! Adapter-parity pins (NE-2): a `Nested`-backed field slot must behave
//! IDENTICALLY to a direct `ObjectSchema` child — same joined paths (ER-2),
//! same single innermost issue under fail-fast descent (ER-4), same `shape()`
//! node (DV-6), same `field_meta()` (OS-1). Every assertion compares the
//! `Nested`-backed composition against the hand-built equivalent byte-equal.

use formars_core::error::IssueCode;
use formars_core::prelude::*;
use formars_core::schema::{DynSchema, ShapeKind};
use formars_core::types::ObjectSchema;
use formars_core::value::{Object, Value};

/// Stand-in for a derive companion: owns a composed `ObjectSchema` and lends
/// it via `AsRef` — exactly the contract the generated code relies on.
#[derive(Debug)]
struct Composed(ObjectSchema);

impl AsRef<ObjectSchema> for Composed {
    fn as_ref(&self) -> &ObjectSchema {
        &self.0
    }
}

fn leaf() -> ObjectSchema {
    object().field("qty", coerced::<u32>())
}

fn mid() -> ObjectSchema {
    object().field("leaf", leaf())
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut o = Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    Value::Object(o)
}

#[test]
fn er2_nested_coercion_failure_matches_direct_child() {
    let input = obj(&[("inner", obj(&[("qty", Value::String("abc".into()))]))]);
    let direct = object().field("inner", leaf());
    let adapted = object().field("inner", Nested::new(Composed(leaf())));
    let issues = direct.validate_value(&input);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::Coerce);
    assert_eq!(issues[0].path.to_string(), "inner.qty");
    assert_eq!(
        direct.validate_value(&input),
        adapted.validate_value(&input)
    );
}

#[test]
fn nested_absent_key_required_matches_direct_child() {
    let input = Value::Object(Object::new());
    let direct = object().field("inner", leaf());
    let adapted = object().field("inner", Nested::new(Composed(leaf())));
    let issues = direct.validate_value(&input);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::Required);
    assert_eq!(issues[0].path.to_string(), "inner");
    assert_eq!(
        direct.validate_value(&input),
        adapted.validate_value(&input)
    );
}

#[test]
fn er2_two_level_nesting_matches_direct_child() {
    let input = obj(&[(
        "mid",
        obj(&[("leaf", obj(&[("qty", Value::String("abc".into()))]))]),
    )]);
    let direct = object().field("mid", mid());
    let adapted = object().field("mid", Nested::new(Composed(mid())));
    let issues = direct.validate_value(&input);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::Coerce);
    assert_eq!(issues[0].path.to_string(), "mid.leaf.qty");
    assert_eq!(
        direct.validate_value(&input),
        adapted.validate_value(&input)
    );
}

#[test]
fn dv6_nested_shape_node_matches_direct_child() {
    let direct = object().field("mid", mid());
    let adapted = object().field("mid", Nested::new(Composed(mid())));
    assert_eq!(direct.shape(), adapted.shape());

    let ShapeKind::Object { fields } = &adapted.shape().kind else {
        panic!("expected Object kind");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].key.as_ref(), "mid");
    let ShapeKind::Object { fields: inner } = &fields[0].child.kind else {
        panic!("expected nested Object kind");
    };
    let keys: Vec<&str> = inner.iter().map(|f| f.key.as_ref()).collect();
    assert_eq!(keys, vec!["leaf"]);
}

#[test]
fn os1_nested_field_meta_matches_direct_child() {
    let direct = object().field("mid", mid().label("Mid"));
    let adapted = object().field("mid", Nested::new(Composed(mid().label("Mid"))));
    assert_eq!(direct.field_meta("mid"), adapted.field_meta("mid"));
    assert_eq!(
        direct.field_meta("mid").and_then(|m| m.label.clone()),
        Some("Mid".into())
    );
}

#[test]
fn er4_fail_fast_yields_single_innermost_issue_through_nested() {
    // Deep failure declared BEFORE the shallow one: fail-fast must stop at the
    // innermost violated constraint (`deep.qty`) with EXACTLY ONE issue.
    let input = obj(&[
        ("deep", obj(&[("qty", Value::String("abc".into()))])),
        ("early", Value::String("x".into())),
    ]);
    let direct = object()
        .fail_fast()
        .field("deep", leaf())
        .field("early", string().min(10));
    let adapted = object()
        .fail_fast()
        .field("deep", Nested::new(Composed(leaf())))
        .field("early", string().min(10));
    for s in [&direct, &adapted] {
        let issues = s.validate_value(&input);
        assert_eq!(issues.len(), 1, "fail-fast must yield exactly one issue");
        assert_eq!(issues[0].code, IssueCode::Coerce);
        assert_eq!(issues[0].path.to_string(), "deep.qty");
    }
    assert_eq!(
        direct.validate_value(&input),
        adapted.validate_value(&input)
    );
}
