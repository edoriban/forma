//! Nesting conformance (spec #1635: NE-2/NE-3) — arbitrary-depth derived
//! composition with dot-joined paths, inherited fail-fast, and free mixing
//! with hand-built `object()` fields.

use formars_core::error::IssueCode;
use formars_core::prelude::*;
use formars_core::schema::{DynSchema, Schema, ShapeKind};
use formars_core::value::{Object, Value};
use formars_derive::FormSchema;

#[derive(FormSchema)]
struct C {
    ok_flag: bool,
}

#[derive(FormSchema)]
struct B {
    c: C,
}

#[derive(FormSchema)]
struct A {
    z_last: String,
    b: B,
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut o = Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    Value::Object(o)
}

#[test]
fn ne3_three_level_valid_input_succeeds_end_to_end() {
    let erased: Box<dyn DynSchema> = Box::new(ASchema::new());
    let input = obj(&[
        ("z_last", Value::from("ok")),
        ("b", obj(&[("c", obj(&[("ok_flag", Value::Bool(true))]))])),
    ]);
    assert!(erased.validate_value(&input).is_empty());

    let typed = A {
        z_last: "ok".into(),
        b: B {
            c: C { ok_flag: true },
        },
    };
    let out = <ASchema as Schema>::parse(&ASchema::new(), &typed).expect("typed parse succeeds");
    assert!(out.b.c.ok_flag);
}

#[test]
fn ne3_failing_leaf_reports_dot_joined_path() {
    let erased: Box<dyn DynSchema> = Box::new(ASchema::new());
    let input = obj(&[
        ("z_last", Value::from("ok")),
        (
            "b",
            obj(&[("c", obj(&[("ok_flag", Value::from("not-a-bool"))]))]),
        ),
    ]);
    let issues = erased.validate_value(&input);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(issues[0].path.to_string(), "b.c.ok_flag");
}

#[test]
fn ne3_declaration_order_preserved_at_every_level() {
    let erased: Box<dyn DynSchema> = Box::new(ASchema::new());
    let shape = erased.shape();
    let ShapeKind::Object { fields } = &shape.kind else {
        panic!("expected Object kind");
    };
    // Level 1: declaration order (`z_last` before `b`, not alphabetical).
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_ref()).collect();
    assert_eq!(keys, vec!["z_last", "b"]);
    // Level 2.
    let ShapeKind::Object { fields: level2 } = &fields[1].child.kind else {
        panic!("expected nested Object kind");
    };
    assert_eq!(
        level2.iter().map(|f| f.key.as_ref()).collect::<Vec<_>>(),
        vec!["c"]
    );
    // Level 3.
    let ShapeKind::Object { fields: level3 } = &level2[0].child.kind else {
        panic!("expected nested Object kind");
    };
    assert_eq!(
        level3.iter().map(|f| f.key.as_ref()).collect::<Vec<_>>(),
        vec!["ok_flag"]
    );
}

/// ER-4 descent: `.fail_fast()` applied at OUTER composition (hand-built
/// object wrapping a derived child) stops at the FIRST violation anywhere —
/// exactly ONE issue, raised by the innermost violated constraint, even when
/// a shallow sibling also violates.
#[test]
fn er4_fail_fast_at_outer_composition_yields_single_innermost_issue() {
    #[derive(FormSchema)]
    struct Deep {
        flag: bool,
    }
    #[derive(FormSchema)]
    struct Mid {
        deep: Deep,
    }

    let outer = object()
        .fail_fast()
        .field("mid", Nested::new(MidSchema::new()))
        .field("shallow", string().min(10));

    let input = obj(&[
        ("mid", obj(&[("deep", obj(&[("flag", Value::from("x"))]))])),
        ("shallow", Value::from("short")),
    ]);
    let issues = outer.validate_value(&input);
    assert_eq!(issues.len(), 1, "fail-fast must yield exactly one issue");
    assert_eq!(issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(issues[0].path.to_string(), "mid.deep.flag");
}

/// NE-3: derived children mix freely with hand-built `object()` fields.
#[test]
fn ne3_derived_children_mix_freely_with_hand_built_fields() {
    #[derive(FormSchema)]
    struct Profile {
        nickname: String,
    }

    let mixed = object()
        .field("email", string().min(3))
        .field(
            "profile",
            Nested::new(<Profile as FormSchema>::form_schema()),
        )
        .field("age", coerced::<u32>());

    let good = obj(&[
        ("email", Value::from("a@b.co")),
        ("profile", obj(&[("nickname", Value::from("ada"))])),
        ("age", Value::from("30")),
    ]);
    assert!(mixed.validate_value(&good).is_empty());

    let bad = obj(&[
        ("email", Value::from("a@b.co")),
        ("profile", obj(&[("nickname", Value::Bool(true))])),
        ("age", Value::from("30")),
    ]);
    let issues = mixed.validate_value(&bad);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].path.to_string(), "profile.nickname");
}
