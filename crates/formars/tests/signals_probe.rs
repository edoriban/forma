//! FA-IN-3 probe: the headless controller tier is usable through the facade
//! prelude ALONE — no direct `formars_signals` / `reactive_graph` imports
//! anywhere in this file. Deliberately synchronous (design D5): register →
//! set (`Set`) → read (`Get`) → sync `validate()` → `apply_server_errors`
//! merge-back.

#![cfg(feature = "signals")]

use formars::prelude::*;

#[test]
fn headless_flow_set_get_validate_and_server_errors() {
    let mut c = FormController::new(ValidateOn::Blur);
    let email = c
        .register_with(
            FieldPath::key("email"),
            Box::new(string().min(8).email()),
            ValidateOn::Change,
        )
        .expect("fresh registration");

    // Edits flow through the handle's value signal (`Set`)...
    email.value().set(Value::from("user@example.com"));
    // ...and derived state reads back through `Get`.
    assert_eq!(email.value().get(), Value::from("user@example.com"));
    assert!(email.errors().get().is_empty());

    // Synchronous whole-form validation.
    assert!(c.validate().is_ok());

    // Server errors merge back onto their addressed fields.
    c.apply_server_errors(&FormaError {
        issues: vec![FormaIssue {
            path: FieldPath::key("ghost"),
            code: IssueCode::Refine,
            message: "unknown field".into(),
            params: Vec::new(),
        }],
    });
    assert!(
        c.form_errors()
            .get()
            .iter()
            .any(|i| i.path.to_string() == "ghost")
    );
}

#[test]
fn get_set_read_resolve_from_prelude_not_direct_import() {
    fn assert_get<T: Get>(_t: &T) {}
    fn assert_set<T: Set>(_t: &T) {}
    fn assert_read<T: Read>(_t: &T) {}

    let mut c = FormController::new(ValidateOn::Blur);
    let handle = c
        .register(FieldPath::key("f"), Box::new(string()))
        .expect("fresh registration");
    let signal = handle.value();
    assert_get(&signal);
    assert_set(&signal);
    assert_read(&signal);
    assert_read(&handle.errors());
}

#[test]
fn six_signal_names_resolve_through_prelude() {
    let mut c = FormController::new(ValidateOn::Blur);

    // Type positions pin each of the six signals-layer names.
    let _: ValidateOn = ValidateOn::Change;
    let _: SubmitError<std::convert::Infallible> = SubmitError::Validation(FormaError::default());
    let snapshot = FormSnapshot::default();
    assert!(snapshot.is_empty());

    let handle = c
        .register(FieldPath::key("f"), Box::new(string()))
        .expect("fresh registration");
    let _: &FieldHandle = &handle;

    let Err(dup) = c.register(FieldPath::key("f"), Box::new(string())) else {
        panic!("duplicate registration must be rejected");
    };
    assert!(matches!(dup, RegisterError::Duplicate { .. }));

    let _: &FormController = &c;
}

#[test]
fn multi_path_identity_via_mirrors() {
    // Prelude Value and BOTH mirror paths are one DefId: assignment compiles
    // across every path in both directions.
    let via_prelude: Value = Value::from("same");
    let via_signals_mirror: formars::formars_signals::Value = via_prelude;
    let via_core_mirror: formars::formars_core::value::Value = via_signals_mirror;
    let back: Value = via_core_mirror;
    assert_eq!(back, Value::from("same"));
}
