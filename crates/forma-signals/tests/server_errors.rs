//! FSS-5: `apply_server_errors` routing, edit-clears-stale semantics.

use forma_core::error::{FieldPath, FormaError, FormaIssue, IssueCode};
use forma_core::prelude::*;
use forma_signals::{FormController, ValidateOn};
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
fn fss5_server_issue_lands_on_addressed_field() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("email"), Box::new(string()))
        .unwrap();
    let err = FormaError {
        issues: vec![issue(
            FieldPath::key("email"),
            IssueCode::Refine,
            "already taken",
        )],
    };
    c.apply_server_errors(&err);
    let h = c.field(&FieldPath::key("email")).unwrap();
    let visible = h.visible_errors().get();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].code, IssueCode::Refine);
    assert_eq!(visible[0].message, "already taken");
}

#[test]
fn fss5_unknown_path_issue_routed_to_form_level_no_panic() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("known"), Box::new(string()))
        .unwrap();
    let ghost = FieldPath::key("ghost");
    let err = FormaError {
        issues: vec![issue(ghost.clone(), IssueCode::Refine, "ghost rule")],
    };
    c.apply_server_errors(&err);
    let form_level: Vec<_> = c
        .form_errors()
        .get()
        .into_iter()
        .filter(|i| i.path == ghost)
        .collect();
    assert_eq!(form_level.len(), 1, "unmatched issue must reach form level");
}

#[test]
fn fss5_value_edit_clears_that_fields_stale_server_issues() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("email"), Box::new(string().min(8)))
        .unwrap();
    let path = FieldPath::key("email");
    c.field(&path)
        .unwrap()
        .value()
        .set(Value::from("taken@x.io"));
    let err = FormaError {
        issues: vec![issue(path.clone(), IssueCode::Refine, "already taken")],
    };
    c.apply_server_errors(&err);
    let h = c.field(&path).unwrap();
    assert!(!h.visible_errors().get().is_empty());
    h.value().set(Value::from("fresh@x.io"));
    assert!(
        !h.visible_errors()
            .get()
            .iter()
            .any(|i| i.message == "already taken"),
        "edit must clear stale server issues for that field"
    );
}

#[test]
fn fss5_reapply_replaces_not_appends_server_cells() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    let path = FieldPath::key("a");
    let first = FormaError {
        issues: vec![
            issue(path.clone(), IssueCode::Refine, "one"),
            issue(path.clone(), IssueCode::Refine, "two"),
        ],
    };
    c.apply_server_errors(&first);
    let second = FormaError {
        issues: vec![issue(path.clone(), IssueCode::Max, "only-this")],
    };
    c.apply_server_errors(&second);
    let h = c.field(&path).unwrap();
    let visible = h.visible_errors().get();
    assert_eq!(visible.len(), 1, "reapply must replace prior server issues");
    assert_eq!(visible[0].code, IssueCode::Max);
}
