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
    assert_eq!(
        empty.to_string(),
        "``",
        "empty key renders the bare open/close backtick pair"
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
    let rendered = p.to_string();
    assert_eq!(rendered, p.to_string(), "display is deterministic");
    assert_eq!(
        rendered, "user.`a.b`",
        "mixed path: raw head, dot separator, quoted dotted tail"
    );
}

// --------------------------------------------- F5 control-char escaping pins

#[test]
fn f5_control_char_key_renders_escaped_inside_quotes() {
    // Exact byte sequence: backtick, '\', 'u', '{', '0', 'a', '}', backtick.
    assert_eq!(
        FieldPath::key("\n").to_string(),
        "`\\u{0a}`",
        "control chars never render raw; uniform lowercase \\u{{XX}} escapes"
    );
}

#[test]
fn f5_backslash_hole_real_newline_and_literal_lookalike_differ() {
    // key₁ carries a REAL U+000A; key₂ is the literal text a\u{0a}.b built
    // from backslash,u,{,0,a,},.,b. Both route to the quoted arm via '.'.
    let real_newline = FieldPath::key("a\u{0a}.b");
    let literal_lookalike = FieldPath::key("a\\u{0a}.b");

    assert_eq!(
        real_newline.to_string(),
        "`a\\u{0a}.b`",
        "real control renders ESCAPED (uniform \\u{{XX}})"
    );
    assert_eq!(
        literal_lookalike.to_string(),
        "`a\\\\u{0a}.b`",
        "literal backslash DOUBLES, so the lookalike stays distinct"
    );
    assert_ne!(
        real_newline.to_string(),
        literal_lookalike.to_string(),
        "the backslash hole is closed: structurally distinct keys never collide"
    );
}

/// Injectivity property over an adversarial corpus (spec Domain 4): every
/// ordered pair of DISTINCT keys renders differently — singly AND joined
/// below `user.`. Positive lower bound: all renderings collected into a set
/// whose length EQUALS the corpus length (not merely absence of duplicates).
const CORPUS: [&str; 36] = [
    "",
    "a",
    "email",
    "user",
    "A1",
    "x9", // empty + plain alnum
    ".",
    "[",
    "]",
    "`",  // single triggers
    "\\", // lone backslash
    "\n",
    "\t",
    "\0",
    "\r",
    "\u{7f}",
    "\u{85}",
    "\u{9f}",  // C0, DEL, C1
    "\\u{0a}", // literal \u-lookalike text
    "a.b",
    "a[0]",
    "a`b",
    "a\\b", // trigger mixtures
    "a\nb",
    ".\n",
    "`\n", // control + trigger mixes
    "café",
    "日本語", // unicode alphanumerics
    "user\nemail",
    "a\u{0}.b", // long mixtures
    "[0]",
    "x[0]", // index masquerade family
    " ",
    "--",
    "x.",
    "-x", // punctuation edge classes
];

#[test]
fn f5_rendering_is_injective_over_adversarial_corpus() {
    use std::collections::HashSet;

    assert!(CORPUS.len() >= 30, "corpus lower bound (spec Domain 4)");

    let singles: Vec<String> = CORPUS
        .iter()
        .map(|k| FieldPath::key(k).to_string())
        .collect();

    // Positive lower bound anchor: set cardinality equals corpus cardinality.
    let unique: HashSet<&String> = singles.iter().collect();
    assert_eq!(
        unique.len(),
        CORPUS.len(),
        "all {} corpus renderings are pairwise distinct (set-len anchor)",
        CORPUS.len()
    );

    // Every ORDERED distinct pair differs, singly and joined under `user.`.
    for i in 0..CORPUS.len() {
        for j in 0..CORPUS.len() {
            if i == j {
                continue;
            }
            assert_ne!(
                singles[i], singles[j],
                "keys {i:?} ({:?}) and {j:?} ({:?}) render identically",
                CORPUS[i], CORPUS[j]
            );
            let joined_i = FieldPath::key("user")
                .join(Segment::Key(CORPUS[i].into()))
                .to_string();
            let joined_j = FieldPath::key("user")
                .join(Segment::Key(CORPUS[j].into()))
                .to_string();
            assert_ne!(
                joined_i, joined_j,
                "joined forms of {:?} and {:?} render identically",
                CORPUS[i], CORPUS[j]
            );
        }
    }
}
