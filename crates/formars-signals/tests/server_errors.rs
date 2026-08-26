//! FSS-5: `apply_server_errors` routing, edit-clears-stale semantics.

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
fn fsa4_server_baseline_follows_the_registered_preset() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register_initial(
        FieldPath::key("email"),
        Box::new(string()),
        Value::from("P"),
    )
    .unwrap();
    let path = FieldPath::key("email");
    let err = FormaError {
        issues: vec![issue(path.clone(), IssueCode::Refine, "taken")],
    };
    c.apply_server_errors(&err);
    let h = c.field(&path).unwrap();
    assert!(
        !h.visible_errors().get().is_empty(),
        "server issue visible while value sits on the preset baseline"
    );
    h.value().set(Value::from("away"));
    assert!(
        !h.visible_errors()
            .get()
            .iter()
            .any(|i| i.message == "taken"),
        "edit away from the preset hides the server issue"
    );
    h.value().set(Value::from("P"));
    assert!(
        h.visible_errors()
            .get()
            .iter()
            .any(|i| i.message == "taken"),
        "returning to the preset makes the server issue visible again"
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

/// Reset kills form-level (unmatched) server issues — they must not survive
/// as ghosts after the fields are restored — and a subsequent apply routes
/// normally again (spec Domain 3 regression pin).
#[test]
fn fss5_reset_kills_ghost_form_level_issue_then_apply_works_again() {
    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("known"), Box::new(string()))
        .unwrap();
    let ghost = FieldPath::key("ghost");
    let err = FormaError {
        issues: vec![issue(ghost.clone(), IssueCode::Refine, "ghost rule")],
    };
    c.apply_server_errors(&err);
    assert!(
        c.form_errors().get().iter().any(|i| i.path == ghost),
        "ghost issue must be visible at form level before reset"
    );
    c.reset();
    assert!(
        !c.form_errors().get().iter().any(|i| i.path == ghost),
        "reset must clear form-level (unmatched) server issues"
    );
    let second = FormaError {
        issues: vec![issue(
            FieldPath::key("known"),
            IssueCode::Refine,
            "fresh server verdict",
        )],
    };
    c.apply_server_errors(&second);
    let known = c.field(&FieldPath::key("known")).unwrap();
    assert!(
        known
            .visible_errors()
            .get()
            .iter()
            .any(|i| i.message == "fresh server verdict"),
        "apply after reset must work normally"
    );
}

// ------------------------------------------------- F3 threaded stress pin
//
// Honest framing: atomicity of the capture inside `apply_server_errors` is
// established by construction (ONE lock acquisition); this test pins the
// OBSERVABLE consequences probabilistically — a microsecond tear window is
// not deterministically catchable. It asserts the sequential contract still
// holds verbatim after sustained interleaving from concurrent editors.

use std::sync::Arc;

fn mixed_error(tag: &str) -> FormaError {
    // Owned `Cow` messages (`format!(...).into()`): no `'static` leaks, and
    // assertion semantics are unchanged (`Cow` compares by value).
    let owned_issue = |path: FieldPath, code: IssueCode, msg: String| FormaIssue {
        path,
        code,
        message: msg.into(),
        params: Vec::new(),
    };
    FormaError {
        issues: vec![
            owned_issue(
                FieldPath::key("a"),
                IssueCode::Refine,
                format!("server-a-{tag}"),
            ),
            owned_issue(
                FieldPath::key("b"),
                IssueCode::Max,
                format!("server-b-{tag}"),
            ),
            owned_issue(
                FieldPath::key("unknown"),
                IssueCode::Refine,
                format!("ghost-{tag}"),
            ),
            owned_issue(FieldPath::ROOT, IssueCode::Required, format!("root-{tag}")),
        ],
    }
}

#[test]
fn f3_stress_threads_alternate_edits_and_applies_quiescent_contract_holds() {
    const THREADS: usize = 4;
    const ITERATIONS: usize = 2000;

    let mut c = FormController::new(ValidateOn::Blur);
    c.register(FieldPath::key("a"), Box::new(string())).unwrap();
    c.register(FieldPath::key("b"), Box::new(string())).unwrap();
    let controller = Arc::new(c);

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let controller = controller.clone();
            std::thread::spawn(move || {
                let ha = controller.field(&FieldPath::key("a")).unwrap();
                for i in 0..ITERATIONS {
                    // alternating edit and apply
                    ha.value().set(Value::from(format!("t{t}-i{i}")));
                    controller.apply_server_errors(&mixed_error(&format!("t{t}-{i}")));
                }
            })
        })
        .collect();
    for h in handles {
        h.join().expect("storm thread: no panic/mutex-poison");
    }

    // Quiescent state: ONE final apply, then the sequential contract holds.
    let final_err = mixed_error("final");
    controller.apply_server_errors(&final_err);

    let ha = controller.field(&FieldPath::key("a")).unwrap();
    let hb = controller.field(&FieldPath::key("b")).unwrap();

    // Each field's visible server cells equal the final error's known-path
    // partition exactly (baseline anchored to the captured value, unedited).
    let va = ha.visible_errors().get();
    assert_eq!(va.len(), 1);
    assert_eq!(va[0].message, "server-a-final");

    let vb = hb.visible_errors().get();
    assert_eq!(vb.len(), 1);
    assert_eq!(vb[0].message, "server-b-final");

    // unmatched equals expected unknown + ROOT partition of the final error.
    // form_errors() is the AGGREGATE: visible per-field server cells plus the
    // unmatched collection — so the full partition must appear exactly once.
    let form_level = controller.form_errors().get();
    let count_by_message = |m: &str| form_level.iter().filter(|i| i.message == m).count();
    assert_eq!(
        count_by_message("server-a-final"),
        1,
        "field a's known-path issue present once"
    );
    assert_eq!(
        count_by_message("server-b-final"),
        1,
        "field b's known-path issue present once"
    );
    assert_eq!(
        form_level
            .iter()
            .filter(|i| i.path == FieldPath::key("unknown"))
            .count(),
        1,
        "exactly one unknown-path issue (unmatched)"
    );
    assert_eq!(
        form_level
            .iter()
            .filter(|i| i.path == FieldPath::ROOT)
            .count(),
        1,
        "exactly one ROOT issue (unmatched)"
    );
    assert_eq!(
        form_level.len(),
        4,
        "nothing beyond the final apply's four issues"
    );

    // Editing any field hides exactly THAT field's server issues.
    ha.value().set(Value::from("edited-after-storm"));
    assert!(
        ha.visible_errors().get().is_empty(),
        "edit must hide field a's stale server issues"
    );
    assert_eq!(
        hb.visible_errors().get()[0].message,
        "server-b-final",
        "field b's server issues unaffected by a's edit"
    );
}
