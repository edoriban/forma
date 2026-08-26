//! ER-1..ER-5 error-model behavior, named per requirement ID.

use formars_core::prelude::*;
use formars_core::schema::DynSchema;

#[test]
fn er1_multi_constraint_accumulation() {
    let err = string()
        .min(10)
        .email()
        .parse(&"ab".to_string())
        .unwrap_err();
    let codes: Vec<IssueCode> = err.issues.iter().map(|i| i.code.clone()).collect();
    assert_eq!(
        codes,
        vec![IssueCode::Min, IssueCode::Email],
        "BOTH issues collected"
    );
}

#[test]
fn er1_err_issues_nonempty() {
    let schemas: Vec<Box<dyn DynSchema>> = vec![
        Box::new(string().min(3)),
        Box::new(string().nonempty()),
        Box::new(number::<f64>().positive()),
        Box::new(bool().equals(true)),
        Box::new(formars_core::coerce::coerced::<u32>()),
    ];
    for boxed in schemas {
        let issues = boxed.validate_value(&Value::Null);
        assert!(
            !issues.is_empty(),
            "failing erased validation always yields >= 1 issue"
        );
    }
}

#[test]
fn er2_issues_for_path_lookup() {
    let email_path = FieldPath::key("email");
    let make_issue = |path: FieldPath, code: IssueCode| FormaIssue {
        path,
        code,
        message: "m".into(),
        params: Vec::new(),
    };
    let e = FormaError {
        issues: vec![
            make_issue(email_path.clone(), IssueCode::Email),
            make_issue(FieldPath::key("age"), IssueCode::Min),
            make_issue(email_path.clone(), IssueCode::Max),
        ],
    };
    let got: Vec<IssueCode> = e.issues_for(&email_path).map(|i| i.code.clone()).collect();
    assert_eq!(got, vec![IssueCode::Email, IssueCode::Max]);
}

#[test]
fn er2_first_returns_earliest() {
    let err = string()
        .min(10)
        .email()
        .parse(&"ab".to_string())
        .unwrap_err();
    assert_eq!(
        err.first().unwrap().code,
        IssueCode::Min,
        "first() is the earliest-collected issue"
    );
}

#[test]
fn er3_code_based_matching_coerce() {
    let err = formars_core::coerce::coerced::<u32>()
        .parse(&"x".to_string())
        .unwrap_err();
    assert!(
        matches!(err.first().unwrap().code, IssueCode::Coerce),
        "code equals the coercion code"
    );
}

#[test]
fn er4_fail_fast_single_issue() {
    let s = string().min(10).email().fail_fast();
    let err = s.parse(&"ab".to_string()).unwrap_err();
    assert_eq!(err.issues.len(), 1);
    assert_eq!(err.issues[0].code, IssueCode::Min);
}

#[test]
fn er4_accumulate_off_by_default() {
    let s = string().min(10).email();
    let err = s.parse(&"ab".to_string()).unwrap_err();
    assert_eq!(err.issues.len(), 2);
}

#[test]
fn er5_order_stability_loop() {
    let s = string().min(10).email().refine(|_| false);
    let expected = [IssueCode::Min, IssueCode::Email];
    for _ in 0..100 {
        let codes: Vec<IssueCode> = s
            .parse(&"ab".to_string())
            .unwrap_err()
            .issues
            .into_iter()
            .map(|i| i.code)
            .collect();
        assert_eq!(&codes[..2], &expected, "declaration order is deterministic");
        assert_eq!(codes.len(), 2, "refines skipped when builtins fail (RF-1)");
    }
}

// ------------------------------------------------------- F5 Display quoting

use formars_core::error::{FieldPath, Segment};

#[test]
fn f5_normal_paths_render_byte_identically() {
    assert_eq!(FieldPath::key("email").to_string(), "email");
    let nested = FieldPath::key("user").join(Segment::Key("email".into()));
    assert_eq!(nested.to_string(), "user.email");
    let indexed = FieldPath::key("items")
        .join(Segment::Index(2))
        .join(Segment::Key("qty".into()));
    assert_eq!(indexed.to_string(), "items[2].qty");
}

#[test]
fn f5_dotted_key_disambiguated_from_nesting() {
    let flat = FieldPath::key("user").join(Segment::Key("a.b".into()));
    let deep = FieldPath::key("user")
        .join(Segment::Key("a".into()))
        .join(Segment::Key("b".into()));
    assert_ne!(
        flat.to_string(),
        deep.to_string(),
        "structurally different paths must never render identically"
    );
    assert_ne!(
        flat.to_string(),
        "user.a.b",
        "flat key must not equal naive concatenation"
    );
    assert_eq!(deep.to_string(), "user.a.b");
}

#[test]
fn f5_bracket_bearing_key_cannot_masquerade_as_index() {
    let tricky = FieldPath::key("x[0]");
    let real_index = FieldPath::key("x").join(Segment::Index(0));
    assert_eq!(real_index.to_string(), "x[0]", "real index arm untouched");
    assert_ne!(
        tricky.to_string(),
        real_index.to_string(),
        "a key containing [0] must not render like an index segment"
    );
}

#[test]
fn f5_empty_key_renders_non_empty_unlike_root() {
    assert_eq!(
        FieldPath::ROOT.to_string(),
        "",
        "ROOT still renders as the empty string"
    );
    let empty = FieldPath::key("");
    assert_ne!(
        empty.to_string(),
        "",
        "an empty KEY never renders as empty output"
    );
}

#[test]
fn f5_backtick_keys_quoted_with_doubling() {
    // Key("a`") -> `a`` (open, a, doubled backtick, close)
    assert_eq!(FieldPath::key("a`").to_string(), "`a```");
    // Key("`") -> four backticks: open + doubled content + close
    assert_eq!(FieldPath::key("`").to_string(), "````");
}

#[test]
fn f5_display_is_deterministic() {
    let p = FieldPath::key("user").join(Segment::Key("a.b".into()));
    assert_eq!(p.to_string(), p.to_string());
}
