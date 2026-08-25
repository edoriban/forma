//! Happy-path conformance for `#[derive(FormSchema)]` (spec #1635:
//! EX-2/EX-3/EX-6 happy paths, EX-4 coercion scenarios).
//!
//! Tests assert the OBSERVABLE CONTRACT through the public forma-core API —
//! never expansion tokens.

use forma_core::error::IssueCode;
use forma_core::prelude::*;
use forma_core::schema::{DynSchema, Schema, ShapeKind};
use forma_core::value::{Object, Value};
use forma_derive::FormSchema;

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut o = Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    Value::Object(o)
}

/// Signature of an issue (code + rendered path) for order comparisons.
fn sig_of(issues: &[forma_core::error::FormaIssue]) -> Vec<forma_core::error::FormaIssue> {
    issues.to_vec()
}

#[derive(FormSchema)]
struct Signup {
    z: String,
    a: bool,
    m: u32,
}

// ------------------------------------------------------------- EX-2 / EX-3

#[test]
fn ex2_companion_exists_and_typed_parse_round_trips() {
    let input = Signup {
        z: "ada".into(),
        a: true,
        m: 42,
    };
    let out = <SignupSchema as Schema>::parse(&SignupSchema::new(), &input)
        .expect("valid input must parse");
    assert_eq!(out.z, "ada");
    assert!(out.a);
    assert_eq!(out.m, 42);
}

#[test]
fn ex3_erased_view_accepts_valid_object() {
    let erased: Box<dyn DynSchema> = Box::new(SignupSchema::new());
    let input = obj(&[
        ("z", Value::from("ada")),
        ("a", Value::Bool(true)),
        ("m", Value::from("42")),
    ]);
    assert!(
        erased.validate_value(&input).is_empty(),
        "valid Value::Object must validate cleanly"
    );
}

#[test]
fn ex2_shape_iterates_in_declaration_order_not_alphabetical() {
    let erased: Box<dyn DynSchema> = Box::new(SignupSchema::new());
    let shape = erased.shape();
    let ShapeKind::Object { fields } = &shape.kind else {
        panic!("expected Object kind");
    };
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_ref()).collect();
    assert_eq!(keys, vec!["z", "a", "m"], "declaration order wins");
}

#[test]
fn ex6_unknown_erased_keys_dropped_silently() {
    let erased: Box<dyn DynSchema> = Box::new(SignupSchema::new());
    let mut o = Object::new();
    o.insert("z", Value::from("ada"));
    o.insert("extra_junk", Value::from("ignored"));
    o.insert("a", Value::Bool(false));
    o.insert("m", Value::from("7"));
    assert!(erased.validate_value(&Value::Object(o)).is_empty());
}

#[test]
fn ex6_reconstruction_fills_fields_from_validated_values() {
    let input = Signup {
        z: "x".into(),
        a: false,
        m: 7,
    };
    let out = <SignupSchema as Schema>::parse(&SignupSchema::new(), &input)
        .expect("valid input must parse");
    assert_eq!(out.m, 7, "reconstructed from validated output");
    assert_eq!(out.z, "x");
    assert!(!out.a);
}

// ------------------------------------------------------------------- EX-4

#[derive(FormSchema)]
struct Qty {
    amount: u32,
}

#[test]
fn ex4_form_string_coerces_into_numeric_field() {
    let erased: Box<dyn DynSchema> = Box::new(QtySchema::new());
    let input = obj(&[("amount", Value::from("42"))]);
    let issues = erased.validate_value(&input);
    assert!(issues.is_empty(), "HTML-form currency strings coerce");
}

#[test]
fn ex4_coercion_failure_yields_single_coerce_issue() {
    let erased: Box<dyn DynSchema> = Box::new(QtySchema::new());
    let input = obj(&[("amount", Value::from("abc"))]);
    let issues = erased.validate_value(&input);
    assert_eq!(issues.len(), 1, "exactly one Coerce issue");
    assert_eq!(issues[0].code, IssueCode::Coerce);
    assert_eq!(issues[0].path.to_string(), "amount");
}

// ------------------------------------------------------- AT-2 rename (C.3)

#[derive(FormSchema)]
struct Renamer {
    #[form(rename = "user_email")]
    email: String,
}

#[test]
fn at2_rename_round_trips_through_wire_key_everywhere() {
    let schema = RenamerSchema::new();

    // erased lookup through the wire key
    let erased: Box<dyn DynSchema> = Box::new(RenamerSchema::new());
    let input = obj(&[("user_email", Value::from("a@b.co"))]);
    assert!(erased.validate_value(&input).is_empty());
    let rust_id_input = obj(&[("email", Value::from("a@b.co"))]);
    let issues = erased.validate_value(&rust_id_input);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::Required, "wire key governs");

    // typed reconstruction
    let out = <RenamerSchema as Schema>::parse(
        &schema,
        &Renamer {
            email: "a@b.co".into(),
        },
    )
    .expect("typed parse succeeds");
    assert_eq!(out.email, "a@b.co", "Rust identifier untouched");

    // shape descriptor
    let erased2: Box<dyn DynSchema> = Box::new(RenamerSchema::new());
    let ShapeKind::Object { fields } = &erased2.shape().kind else {
        panic!("expected Object kind");
    };
    assert_eq!(fields[0].key.as_ref(), "user_email");

    // field_meta lookup
    assert!(schema.field_meta("user_email").is_some());
    assert!(schema.field_meta("email").is_none());
}

// ---------------------------------------------------------- AT-3 skip (C.3)

#[derive(FormSchema)]
struct Rec {
    a: String,
    #[form(skip)]
    secret: u32,
}

#[test]
fn at3_skipped_field_absent_from_shape_and_bridging() {
    let erased: Box<dyn DynSchema> = Box::new(RecSchema::new());

    // absent from shape()
    let shape = erased.shape();
    let ShapeKind::Object { fields } = &shape.kind else {
        panic!("expected Object kind");
    };
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_ref()).collect();
    assert_eq!(keys, vec!["a"], "skipped field must not appear in shape()");

    // no Required when the erased input lacks the key
    let input = obj(&[("a", Value::from("x"))]);
    assert!(erased.validate_value(&input).is_empty());
}

#[test]
fn ex6_skipped_field_passes_through_untouched_in_typed_parse() {
    let input = Rec {
        a: "x".into(),
        secret: 7,
    };
    let out = <RecSchema as Schema>::parse(&RecSchema::new(), &input).expect("parse must succeed");
    assert_eq!(out.secret, 7, "verbatim passthrough, never defaulted");
    assert_eq!(out.a, "x");
}

/// Documented v0 asymmetry (verify finding W2): a derived struct with skipped
/// fields declines erased `from_validated`, so using it as a NESTED CHILD
/// compiles but fails typed parse at runtime — the parent's reconstruction
/// cannot rebuild the child from validated output alone.
#[derive(FormSchema)]
struct SkippedChild {
    a: String,
    #[form(skip)]
    secret: String,
}

#[derive(FormSchema)]
struct ParentOfSkipped {
    child: SkippedChild,
}

#[test]
fn ex6_skipped_field_struct_as_nested_child_fails_typed_parse_with_type_mismatch() {
    let input = ParentOfSkipped {
        child: SkippedChild {
            a: "x".into(),
            secret: "s".into(),
        },
    };
    let Err(err) = <ParentOfSkippedSchema as Schema>::parse(&ParentOfSkippedSchema::new(), &input)
    else {
        panic!("skipped-field child must decline erased bridging in v0");
    };
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(err.issues[0].path.to_string(), "child");
    assert_eq!(
        err.issues[0].message,
        "validated output is missing this field"
    );
}

// ------------------------------------------- AT-4 metadata + AT-1 override (C.5)

#[derive(FormSchema)]
struct MetaFields {
    #[form(
        label = "Full name",
        description = "Your legal name",
        placeholder = "Ada Lovelace"
    )]
    name: String,
    #[form(schema = string().trim().min(4).label("Email"))]
    email: String,
    #[form(schema = coerced::<u32>())]
    qty: u32,
}

#[test]
fn at4_metadata_reachable_via_metadata_and_field_meta() {
    let erased: Box<dyn DynSchema> = Box::new(MetaFieldsSchema::new());

    let meta = erased.metadata();
    // object-level metadata is empty; per-field slots carry the labels
    assert!(meta.label.is_none());

    let schema = MetaFieldsSchema::new();
    let fm = schema.field_meta("name").expect("key present");
    assert_eq!(fm.label.as_deref(), Some("Full name"));
    assert_eq!(fm.description.as_deref(), Some("Your legal name"));
    assert_eq!(fm.placeholder.as_deref(), Some("Ada Lovelace"));
}

#[test]
fn at4_metadata_works_on_renamed_fields() {
    #[derive(FormSchema)]
    struct RenamedMeta {
        #[form(rename = "user_name", label = "Username")]
        name: String,
    }
    let schema = RenamedMetaSchema::new();
    let fm = schema.field_meta("user_name").expect("renamed key present");
    assert_eq!(fm.label.as_deref(), Some("Username"));
}

#[test]
fn at1_override_applies_constraints_through_erased_view() {
    let erased: Box<dyn DynSchema> = Box::new(MetaFieldsSchema::new());
    let mut o = Object::new();
    o.insert("name", Value::from("ada"));
    o.insert("email", Value::from("  a@b.co  "));
    o.insert("qty", Value::from("3"));
    let issues = erased.validate_value(&Value::Object(o));
    assert!(
        issues.is_empty(),
        "trimmed override accepts padded email: {issues:?}"
    );

    // untrimmed too-short local part violates min(4) AFTER trim
    let mut o = Object::new();
    o.insert("name", Value::from("ada"));
    o.insert("email", Value::from(" ab  "));
    o.insert("qty", Value::from("3"));
    let issues = erased.validate_value(&Value::Object(o));
    assert_eq!(issues.len(), 1, "override constraint enforced");

    // the override's label reaches field_meta
    let schema2 = MetaFieldsSchema::new();
    let fm = schema2.field_meta("email").expect("present");
    assert_eq!(fm.label.as_deref(), Some("Email"));
}

// ------------------------------------------------- DP-3 determinism (C.8)

#[derive(FormSchema)]
struct Multi {
    aa: String,
    bb: u32,
    cc: String,
    dd: bool,
}

fn failing_input() -> Value {
    // several fields violated: wrong variant for strings, non-coercible number
    let mut o = Object::new();
    o.insert("aa", Value::I64(1));
    o.insert("bb", Value::from("nope"));
    o.insert("cc", Value::Bool(true));
    o.insert("dd", Value::Bool(false));
    Value::Object(o)
}

#[test]
fn dp3_issue_order_is_declaration_order_across_repetitions() {
    let schema: Box<dyn DynSchema> = Box::new(MultiSchema::new());
    let expected = sig_of(&schema.validate_value(&failing_input()));
    assert_eq!(expected.len(), 3, "three violations collected");
    for _ in 0..10 {
        assert_eq!(sig_of(&schema.validate_value(&failing_input())), expected);
    }
    // multiple compilations of the same fixture agree
    let again: Box<dyn DynSchema> = Box::new(MultiSchema::new());
    assert_eq!(sig_of(&again.validate_value(&failing_input())), expected);
    // paths follow declaration order aa < bb < cc (dd valid)
    let paths: Vec<String> = expected.iter().map(|i| i.path.to_string()).collect();
    assert_eq!(paths, vec!["aa", "bb", "cc"]);
}
