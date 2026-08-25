//! FA-IN-5 probe: derive round-trip through the facade prelude alone. The
//! `#[form(..)]` helper attribute activates via the re-exported `FormSchema`
//! derive (serde precedent — no separate import), and the core `FormSchema`
//! TRAIT coexists with it in THIS SAME FILE because trait and macro occupy
//! different namespaces.

#![cfg(feature = "derive")]

use formars::prelude::*;

#[derive(formars::FormSchema)]
struct Signup {
    #[form(label = "Your name")]
    name: String,
    #[form(label = "Age")]
    age: u32,
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    // `Object` travels through the advanced mirror path, keeping this file's
    // toolkit imports at exactly ONE line (`use formars::prelude::*;`).
    let mut o = formars::formars_core::value::Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    Value::Object(o)
}

#[test]
fn derive_and_trait_coexist_and_typed_parse_round_trips() {
    // MACRO namespace: the derive above expanded and its helper attributes
    // resolved. TRAIT namespace: callable in the very same glob scope.
    let companion = <Signup as FormSchema>::form_schema();

    let out = companion
        .parse(&Signup {
            name: "ada".into(),
            age: 42,
        })
        .expect("typed parse succeeds");
    assert_eq!(out.name, "ada");
    assert_eq!(out.age, 42);
}

#[test]
fn erased_view_reports_coerce_at_age_path() {
    let erased: Box<dyn DynSchema> = Box::new(<Signup as FormSchema>::form_schema());
    let issues = erased.validate_value(&obj(&[
        ("name", Value::from("ada")),
        ("age", Value::from("not-a-number")),
    ]));
    assert_eq!(issues.len(), 1, "exactly one Coerce issue");
    assert_eq!(issues[0].code, IssueCode::Coerce);
    assert_eq!(issues[0].path.to_string(), "age");
}

#[test]
fn form_label_attr_reaches_field_meta() {
    let schema = <Signup as FormSchema>::form_schema();
    let meta = schema
        .field_meta("name")
        .expect("declared key must carry metadata");
    assert_eq!(meta.label.as_deref(), Some("Your name"));
}
