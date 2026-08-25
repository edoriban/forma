//! RF-1..RF-3 refinement semantics, named per requirement ID.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use formars_core::prelude::*;
use formars_core::rule::RefineRejection;

#[test]
fn rf1_refine_skipped_when_earlier_fails() {
    static INVOKED: AtomicBool = AtomicBool::new(false);
    let s = string().min(5).refine(|_| {
        INVOKED.store(true, Ordering::SeqCst);
        true
    });
    let err = s.parse(&"ab".to_string()).unwrap_err();
    assert_eq!(err.first().unwrap().code, IssueCode::Min);
    assert!(
        !INVOKED.load(Ordering::SeqCst),
        "refine closure must never be invoked"
    );
}

#[test]
fn rf1_refine_runs_after_constraints_pass() {
    let received = std::sync::Arc::new(Mutex::new(Vec::new()));
    let sink = received.clone();
    let s = string().min(2).refine(move |s| {
        sink.lock().unwrap().push(s.to_string());
        true
    });
    assert!(s.parse(&"abc".to_string()).is_ok());
    assert_eq!(
        *received.lock().unwrap(),
        vec!["abc".to_string()],
        "closure invoked exactly once with the value"
    );
}

#[test]
fn rf2_two_refines_accumulate_in_order() {
    let s = string().refine(|_| false).refine(|_| false);
    let err = s.parse(&"q".to_string()).unwrap_err();
    assert_eq!(
        err.issues.len(),
        2,
        "both failing refines yield distinct issues"
    );
    assert_eq!(err.issues[0].code, IssueCode::Refine);
    assert_eq!(err.issues[1].code, IssueCode::Refine);
}

#[derive(Debug)]
struct AlwaysRejects;

impl Rule<str> for AlwaysRejects {
    fn name(&self) -> &'static str {
        "always_rejects"
    }
    fn validate(&self, _value: &str) -> Option<RefineRejection> {
        Some(RefineRejection {
            code: None,
            message: "custom rule rejected".into(),
            params: Vec::new(),
        })
    }
}

#[test]
fn rf3_sync_signature_custom_rule_participates() {
    let s = string().rule(AlwaysRejects);
    let err = s.parse(&"anything".to_string()).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(
        err.issues[0].code,
        IssueCode::Refine,
        "None code maps to Refine"
    );
}
