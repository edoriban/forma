//! FSVF-1/FSVF-2: whole-form `validate()` and reactive `form_errors` aggregate.

use formars_core::error::{FieldPath, FormaError, FormaIssue, IssueCode};
use formars_core::prelude::*;
use formars_signals::{FormController, ValidateOn};
use reactive_graph::traits::{Get, Set};

fn issue(path: FieldPath, code: IssueCode, msg: &'static str) -> FormaIssue {
    FormaIssue {
        path,
        code,
        message: msg.into(),
        params: Vec::new(),
    }
}

#[test]
fn fsvf1_clean_form_validates_ok() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("a"), Box::new(string().min(1).max(10)))
        .unwrap();
    c.field(&FieldPath::key("a"))
        .unwrap()
        .value()
        .set(Value::from("fine"));
    assert!(c.validate().is_ok());
}

#[test]
fn fsvf1_multi_field_failure_aggregates_all_violating_fields_only() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("a"), Box::new(string().min(5)))
        .unwrap();
    c.register(FieldPath::key("b"), Box::new(string().min(5)))
        .unwrap();
    c.register(FieldPath::key("c"), Box::new(string().min(1).max(3)))
        .unwrap();
    for name in ["a", "b"] {
        c.field(&FieldPath::key(name))
            .unwrap()
            .value()
            .set(Value::from("x"));
    }
    c.field(&FieldPath::key("c"))
        .unwrap()
        .value()
        .set(Value::from("ok"));
    let Err(err) = c.validate() else {
        panic!("expected validation failure");
    };
    let paths: Vec<_> = err.issues.iter().map(|i| i.path.to_string()).collect();
    assert!(paths.iter().all(|p| p == "a" || p == "b"));
    assert!(paths.contains(&"a".to_string()) && paths.contains(&"b".to_string()));
    assert!(!paths.contains(&"c".to_string()));
}

#[test]
fn fsvf2_correcting_field_empties_form_errors() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let path = FieldPath::key("email");
    c.mark_touched(&path);
    c.field(&path).unwrap().value().set(Value::from("ab"));
    assert!(!c.form_errors().get().is_empty());
    c.field(&path)
        .unwrap()
        .value()
        .set(Value::from("good@example.com"));
    assert!(
        c.form_errors().get().is_empty(),
        "correcting the field must empty the aggregate"
    );
}

#[test]
fn fsvf2_form_errors_equals_visible_errors_plus_unmatched_server() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("touched"), Box::new(string().min(8)))
        .unwrap();
    c.register(FieldPath::key("untouched"), Box::new(string().min(8)))
        .unwrap();
    let touched = FieldPath::key("touched");
    let untouched = FieldPath::key("untouched");
    c.field(&touched).unwrap().value().set(Value::from("bad"));
    c.field(&untouched).unwrap().value().set(Value::from("bad"));
    c.mark_touched(&touched);

    let server = FormaError {
        issues: vec![
            issue(touched.clone(), IssueCode::Refine, "server-a"),
            issue(FieldPath::key("ghost"), IssueCode::Refine, "ghost"),
        ],
    };
    c.apply_server_errors(&server);

    let expected: Vec<FormaIssue> = c
        .field(&touched)
        .unwrap()
        .visible_errors()
        .get()
        .into_iter()
        .chain(c.field(&untouched).unwrap().visible_errors().get())
        .collect();
    let got = c.form_errors().get();
    assert_eq!(
        got.len(),
        expected.len() + 1,
        "aggregate = visible errors + unmatched server (ghost)"
    );
    for i in &expected {
        assert!(got.contains(i));
    }
    assert!(
        got.iter()
            .any(|i| i.message == "ghost" && i.path == FieldPath::key("ghost"))
    );
}

#[test]
fn fsvf2_field_registered_after_first_read_joins_aggregate() {
    let mut c = FormController::new(ValidateOn::Submit);
    let form_errors = c.form_errors();
    assert!(
        form_errors.get().is_empty(),
        "empty registry aggregates nothing"
    );
    // Register AFTER the aggregate memo's first read: the registration epoch
    // must invalidate the memo so the new field's signals are subscribed.
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    c.field(&FieldPath::key("email"))
        .unwrap()
        .value()
        .set(Value::from("ab"));
    c.submitted().set(true); // Submit-mode display gate opens
    let issues = form_errors.get();
    assert!(
        !issues.is_empty(),
        "late-registered field's issues must reach the aggregate"
    );
    assert_eq!(issues[0].path, FieldPath::key("email"));
}
