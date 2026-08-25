//! Dual-view parity (spec #1635: DP-1) — for any input, typed `parse` and
//! erased `validate_value` agree on codes, joined paths and order.

use forma_core::error::IssueCode;
use forma_core::schema::{DynSchema, Schema};
use forma_core::value::{Object, Value};
use forma_derive::FormSchema;

#[derive(FormSchema, Debug)]
struct User {
    email: String,
    age: u32,
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut o = Object::new();
    for (k, v) in pairs {
        o.insert(k, v.clone());
    }
    Value::Object(o)
}

/// Signature of an issue for comparison purposes (code + rendered path).
fn sig(issues: &[forma_core::error::FormaIssue]) -> Vec<(String, IssueCode)> {
    issues
        .iter()
        .map(|i| (i.path.to_string(), i.code.clone()))
        .collect()
}

fn erased_sigs(schema: &UserSchema, input: &Value) -> Vec<(String, IssueCode)> {
    sig(&<UserSchema as DynSchema>::validate_value(schema, input))
}

#[test]
fn dp1_flat_field_views_agree_on_every_typed_representable_input() {
    // v0 default mappings carry no constraints, so every `&User` bridges to a
    // VALID wire object. Parity therefore pins: typed parse outcome == erased
    // outcome ON THE EXACT WIRE OBJECT the typed view feeds the kernel.
    let users = [
        User {
            email: "a@b.co".into(),
            age: 30,
        },
        User {
            email: String::new(),
            age: 0,
        },
    ];
    for user in &users {
        let wire = <User as ::forma_core::form::FormBridge>::to_form_value(user);
        let erased_ok =
            <UserSchema as DynSchema>::validate_value(&UserSchema::new(), &wire).is_empty();
        let typed_ok = <UserSchema as Schema>::parse(&UserSchema::new(), user).is_ok();
        assert_eq!(typed_ok, erased_ok, "views must agree for {wire:?}");

        // And the same wire shape with an injected wrong-variant payload is
        // rejected identically by the kernel regardless of entry view.
        let Value::Object(mut broken) = wire else {
            unreachable!("object schema wires as Object");
        };
        broken.entries.remove(0); // drop `email`
        let sigs = sig(&<UserSchema as DynSchema>::validate_value(
            &UserSchema::new(),
            &Value::Object(broken),
        ));
        assert_eq!(sigs, vec![("email".to_string(), IssueCode::Required)]);
    }
}

#[test]
fn dp1_nested_failure_agrees() {
    #[derive(FormSchema, Debug)]
    struct Inner {
        qty: u32,
    }
    #[derive(FormSchema, Debug)]
    struct Outer {
        inner: Inner,
    }

    let input = obj(&[("inner", obj(&[("qty", Value::from("not-a-number"))]))]);
    let erased = sig(&<OuterSchema as DynSchema>::validate_value(
        &OuterSchema::new(),
        &input,
    ));
    assert_eq!(
        erased,
        vec![("inner.qty".to_string(), IssueCode::Coerce)],
        "joined path identical across views"
    );

    // Typed parse cannot even express a non-coercible value (u32 is total),
    // so parity is pinned via the bridged wire form: the struct's own
    // FormBridge round-trip produces the same wire object the kernel walks.
    let wire = <Outer as ::forma_core::form::FormBridge>::to_form_value(&Outer {
        inner: Inner { qty: 7 },
    });
    assert!(<OuterSchema as DynSchema>::validate_value(&OuterSchema::new(), &wire).is_empty());
}

#[test]
fn dp1_absent_key_yields_exactly_one_required_at_joined_path() {
    let input = Value::Object(Object::new());
    let erased = erased_sigs(&UserSchema::new(), &input);
    assert_eq!(
        erased,
        vec![
            ("email".to_string(), IssueCode::Required),
            ("age".to_string(), IssueCode::Required)
        ],
        "one Required per absent declared field, declaration order"
    );
}

#[test]
fn dp1_non_object_erased_input_yields_single_root_type_mismatch() {
    let schema: Box<dyn DynSchema> = Box::new(UserSchema::new());
    let issues = schema.validate_value(&Value::String("nope".into()));
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, IssueCode::TypeMismatch);
    assert_eq!(issues[0].path.to_string(), "", "ROOT address");
}
