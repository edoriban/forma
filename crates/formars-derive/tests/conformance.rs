//! Happy-path conformance for `#[derive(FormSchema)]` (spec #1635:
//! EX-2/EX-3/EX-6 happy paths, EX-4 coercion scenarios).
//!
//! Tests assert the OBSERVABLE CONTRACT through the public formars-core API —
//! never expansion tokens.

use formars_core::error::IssueCode;
use formars_core::prelude::*;
use formars_core::schema::{DynSchema, Schema, ShapeKind};
use formars_core::value::{Object, Value};
use formars_derive::FormSchema;

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut o = Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    Value::Object(o)
}

/// Signature of an issue (code + rendered path) for order comparisons.
fn sig_of(issues: &[formars_core::error::FormaIssue]) -> Vec<formars_core::error::FormaIssue> {
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

// Dotted renames stay LEGAL; the ambiguity caveat lives in docs, not grammar.
#[derive(FormSchema)]
struct DottedRenamer {
    #[form(rename = "a.b")]
    email: String,
}

#[test]
fn at2_dotted_rename_still_compiles_and_uses_the_literal_wire_key() {
    let erased: Box<dyn DynSchema> = Box::new(DottedRenamerSchema::new());
    let input = obj(&[("a.b", Value::from("a@b.co"))]);
    assert!(
        erased.validate_value(&input).is_empty(),
        "the literal dotted string is the wire key"
    );
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

// ------------------------------------------------- AT-4 metadata + AT-1 override (C.5)

// Dup-wire-key exemption (spec Domain 1): skipped fields contribute no wire
// key, so a skipped field whose IDENT equals a wired key must NOT trip the
// dup-key rejection. (`rename = "..", skip` cannot coexist in the v0
// grammar, so the ident collision is the only expressible variant.)
#[derive(FormSchema)]
struct SkippedKeyExempt {
    wired: String,
    #[form(skip)]
    wired_and_also_secret: u32,
}

#[test]
fn skipped_field_ident_equal_to_wired_key_compiles() {
    let input = SkippedKeyExempt {
        wired: "x".into(),
        wired_and_also_secret: 7,
    };
    let out = <SkippedKeyExemptSchema as Schema>::parse(&SkippedKeyExemptSchema::new(), &input)
        .expect("skipped fields are exempt from wire-key collisions");
    assert_eq!(out.wired, "x");
    assert_eq!(out.wired_and_also_secret, 7);
}

// Docs-vs-behavior conformance (spec Domain 1): a fully-qualified primitive
// path with a `schema` override COMPILES — the override wins and the
// mapping layer (incl. the qualified-primitive diagnostic) is never consulted.
#[derive(FormSchema)]
struct QualifiedOverride {
    #[form(schema = string())]
    s: ::std::string::String,
}

#[test]
fn at1_schema_override_suppresses_qualified_primitive_diagnostic() {
    let erased: Box<dyn DynSchema> = Box::new(QualifiedOverrideSchema::new());
    let input = obj(&[("s", Value::from("override applied"))]);
    assert!(
        erased.validate_value(&input).is_empty(),
        "`#[form(schema = ..)]` must win over the qualified-primitive guard"
    );
    let bad = obj(&[("s", Value::Bool(true))]);
    let issues = erased.validate_value(&bad);
    assert_eq!(issues.len(), 1, "string() override enforced");
    assert_eq!(issues[0].code, IssueCode::TypeMismatch);
}

// Docs-vs-behavior conformance (spec Domain 1): type ALIASES are not mapped
// by name. `Count` derives the traits; its alias composes as a nested child —
// proof that mapping keys on single-segment NAMES, never on what the alias
// points at (the aliased `u32` matrix member is irrelevant here).
#[derive(FormSchema)]
struct Count {
    n: u32,
}

/// Alias of a derived struct: final segment `Counter` is unknown to the
/// mapping table, so the field falls through to nested composition.
type Counter = Count;

#[derive(FormSchema)]
struct AliasFallThrough {
    counter: Counter,
}

#[test]
fn alias_composes_as_nested_child_not_by_name() {
    let erased: Box<dyn DynSchema> = Box::new(AliasFallThroughSchema::new());
    // Nested child: the wire value is an OBJECT (composed child), not the
    // coerced form-string a name/numeric mapping would demand.
    let input = obj(&[("counter", obj(&[("n", Value::from("3"))]))]);
    assert!(
        erased.validate_value(&input).is_empty(),
        "alias must fall through to nested composition"
    );
    // Had the alias been resolved by name/bounds to the aliased primitive,
    // a bare u32 wire value would have been accepted — it is not.
    let as_scalar = obj(&[("counter", Value::from("3"))]);
    assert_eq!(
        erased.validate_value(&as_scalar).len(),
        1,
        "alias did NOT resolve to the numeric coercion mapping"
    );
}

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

// ------------------------------------------------------------------ T2 floats

#[derive(FormSchema)]
struct Meters {
    m: f32,
    exact: f64,
}

#[test]
fn t2_float_strings_coerce_in_and_reconstruct_exactly() {
    let erased: Box<dyn DynSchema> = Box::new(MetersSchema::new());
    let input = obj(&[("m", Value::from("1.5")), ("exact", Value::from("2.25"))]);
    assert!(
        erased.validate_value(&input).is_empty(),
        "ToString path coerces"
    );

    // typed parse reconstructs the exact f32/f64 values
    let parsed = <MetersSchema as Schema>::parse(
        &MetersSchema::new(),
        &Meters {
            m: 1.5,
            exact: 2.25,
        },
    )
    .expect("typed parse succeeds");
    assert_eq!(parsed.m.to_bits(), 1.5f32.to_bits());
    assert_eq!(parsed.exact.to_bits(), 2.25f64.to_bits());
}

#[test]
fn t2_non_coercible_string_yields_coerce_at_joined_path() {
    let erased: Box<dyn DynSchema> = Box::new(MetersSchema::new());
    let bad = obj(&[("m", Value::from("nope")), ("exact", Value::from("2.25"))]);
    let issues = erased.validate_value(&bad);
    assert_eq!(issues.len(), 1, "exactly one Coerce issue");
    assert_eq!(issues[0].code, IssueCode::Coerce);
    assert_eq!(issues[0].path.to_string(), "m");
}

#[test]
fn t2_wire_asymmetry_strings_in_f64_out() {
    // INBOUND: numeric fields travel as STRINGS (HTML-form currency — what
    // `coerced::<T>()` consumes), even for f32/f64 fields.
    let meters = Meters {
        m: 0.1f32,
        exact: f64::from_bits(0x3FD3_3333_3333_3333),
    };
    let wire = <Meters as ::formars_core::form::FormBridge>::to_form_value(&meters);
    let Value::Object(o) = &wire else {
        panic!("expected object wire form");
    };
    assert_eq!(o.get("m"), Some(&Value::String("0.1".into())));
    assert!(matches!(o.get("exact"), Some(Value::String(_))));

    // OUTBOUND: validated output carries I64/F64. `f32` recovers the widened
    // bits EXACTLY through from_validated; f64 is lossless.
    let mut validated = Object::new();
    validated.insert("m", Value::F64(f64::from(0.1f32)));
    validated.insert("exact", Value::F64(f64::from_bits(0x3FD3_3333_3333_3333)));

    let back_m =
        <f32 as ::formars_core::form::FormBridge>::from_validated(validated.get("m").expect("key"))
            .expect("validated output reconstructs");
    assert_eq!(back_m.to_bits(), 0.1f32.to_bits(), "bit-exact recovery");

    let back_e = <f64 as ::formars_core::form::FormBridge>::from_validated(
        validated.get("exact").expect("key"),
    )
    .expect("validated output reconstructs");
    assert_eq!(
        back_e.to_bits(),
        f64::from_bits(0x3FD3_3333_3333_3333).to_bits()
    );

    // And the full derived round-trip agrees: typed parse of the struct whose
    // bridging inserts are the string form reconstructs the same values.
    let parsed = <MetersSchema as Schema>::parse(&MetersSchema::new(), &meters).expect("parses");
    assert_eq!(parsed.m.to_bits(), meters.m.to_bits());
    assert_eq!(parsed.exact.to_bits(), meters.exact.to_bits());
}

// --------------------------------------------------------------- T3 raw idents

#[derive(FormSchema)]
struct Raw {
    r#type: String,
}

#[test]
fn t3_raw_identifier_field_end_to_end() {
    let erased: Box<dyn DynSchema> = Box::new(RawSchema::new());

    // wire key is the raw identifier's source text WITHOUT the r# prefix
    let input = obj(&[("type", Value::from("wifi"))]);
    assert!(erased.validate_value(&input).is_empty());

    // shape lists key `type`
    let ShapeKind::Object { fields } = &erased.shape().kind else {
        panic!("expected Object kind");
    };
    assert_eq!(fields[0].key.as_ref(), "type");

    // typed parse reconstructs r#type
    let parsed = <RawSchema as Schema>::parse(
        &RawSchema::new(),
        &Raw {
            r#type: "wifi".into(),
        },
    )
    .expect("typed parse succeeds");
    assert_eq!(parsed.r#type, "wifi");
}

// ------------------------------------------------------------ T4 visibility

/// T4 positive: `pub` struct yields a `pub` companion.
#[derive(FormSchema)]
pub struct PubStruct {
    name: String,
}

#[derive(FormSchema)]
pub(crate) struct CrateStruct {
    name: String,
}

#[derive(FormSchema)]
struct PrivateStruct {
    name: String,
}

mod t4_cross_module_check {
    use super::{CrateStructSchema, PubStructSchema};

    #[allow(
        dead_code,
        reason = "visibility-tightness compile probe; never invoked"
    )]
    fn companions_visible_at_matching_tightness() {
        // pub companion usable anywhere
        let _pub_ok: PubStructSchema = PubStructSchema::new();
        // pub(crate) companion visible crate-wide
        let _crate_ok: CrateStructSchema = CrateStructSchema::new();
        // NOTE: PrivateStructSchema is deliberately NOT referenced here — it
        // is private to this test file's root module; leaking it through a
        // pub signature is pinned by tests/ui/companion_visibility_leak.rs.
    }
}

#[test]
fn t4_companions_compile_at_struct_visibility() {
    // same-module exposure at matching-tightness signatures
    fn expose_pub(_: &PubStructSchema) {}
    fn expose_crate(_: &CrateStructSchema) {}
    fn expose_private(_: &PrivateStructSchema) {}
    expose_pub(&PubStructSchema::new());
    expose_crate(&CrateStructSchema::new());
    expose_private(&PrivateStructSchema::new());
}
