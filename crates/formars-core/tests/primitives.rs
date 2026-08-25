//! SC-3/SC-5/SC-6 primitive scenarios, named per requirement ID.

use formars_core::prelude::*;
use formars_core::types::NumberSchema;

#[test]
fn sc3_trim_then_max_on_trimmed_input() {
    let s = string().trim().max(3);
    assert_eq!(s.parse(&" ab ".to_string()), Ok(" ab ".to_string()));
}

#[test]
fn sc2_chained_builders_usable_as_impl_schema_without_generics() {
    fn assert_impl_schema<S: Schema>(_: &S) {}
    let s = string().trim().min(2).max(5);
    assert_impl_schema(&s);
    assert_eq!(s.parse(&" abc ".to_string()), Ok(" abc ".to_string()));
}

#[test]
fn sc3_url_parse_accept_and_reject() {
    let schema = string().url();
    let accept = [
        "https://example.com",
        "http://example.com/path?query=1#frag",
        "https://sub.domain.org:8443/x",
        "ftp://files.example.org",
    ];
    let reject = [
        "",
        "example.com",
        "https://",
        "http:///path",
        "://example.com",
    ];
    for s in accept {
        assert!(schema.parse(&s.to_string()).is_ok(), "expected accept: {s}");
    }
    for s in reject {
        let err = schema.parse(&s.to_string()).unwrap_err();
        assert_eq!(
            err.first().unwrap().code,
            IssueCode::Url,
            "expected Url issue: {s}"
        );
    }
}

#[test]
fn sc3_uuid_parse_accept_and_reject() {
    let schema = string().uuid();
    let accept = [
        "123e4567-e89b-12d3-a456-426614174000",
        "00000000-0000-0000-0000-000000000000",
        "123E4567-E89B-12D3-A456-426614174000",
    ];
    let reject = [
        "",
        "123e4567e89b12d3a456426614174000",
        "123e4567-e89b-12d3-a456-42661417400g",
        "123e4567-e89b-12d3-a456",
        "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz",
    ];
    for s in accept {
        assert!(schema.parse(&s.to_string()).is_ok(), "expected accept: {s}");
    }
    for s in reject {
        let err = schema.parse(&s.to_string()).unwrap_err();
        assert_eq!(
            err.first().unwrap().code,
            IssueCode::Uuid,
            "expected Uuid issue: {s}"
        );
    }
}

#[test]
fn sc3_nonempty_rejects_empty() {
    let s = string().nonempty();
    let err = s.parse(&String::new()).unwrap_err();
    assert_eq!(err.first().unwrap().code, IssueCode::Empty);
}

#[test]
fn sc3_email_format_checks() {
    let ok = string().email().parse(&"user@example.com".to_string());
    assert!(ok.is_ok());
    let bad = string()
        .email()
        .parse(&"not-an-email".to_string())
        .unwrap_err();
    assert_eq!(bad.first().unwrap().code, IssueCode::Email);
}

#[test]
fn sc5_finite_rejects_nan() {
    let err = number::<f64>().finite().parse(&f64::NAN).unwrap_err();
    assert_eq!(err.first().unwrap().code, IssueCode::Finite);
}

#[test]
fn sc5_positive_zero_boundary() {
    let err = number::<f64>().positive().parse(&0.0).unwrap_err();
    assert_eq!(err.first().unwrap().code, IssueCode::Positive);
}

#[test]
fn sc5_int_check() {
    let err: FormaError = NumberSchema::<f64>::default()
        .int()
        .parse(&2.5)
        .unwrap_err();
    assert_eq!(err.first().unwrap().code, IssueCode::Int);
}

#[test]
fn sc6_equals_accept_matching_value() {
    assert_eq!(bool().equals(true).parse(&true), Ok(true));
}

#[test]
fn sc6_equals_rejects_mismatched_value_with_one_issue() {
    let err = bool().equals(false).parse(&true).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::BoolEquals);
}
