//! F7: the prelude alone supports one-import usage (spec scenario set).

// The ONLY core import allowed in this file.
use formars_core::prelude::*;

#[derive(Debug)]
struct StartsWithVowel;

impl Rule<str> for StartsWithVowel {
    fn name(&self) -> &'static str {
        "starts_with_vowel"
    }
    fn validate(&self, value: &str) -> Option<RefineRejection> {
        if matches!(value.chars().next(), Some('a' | 'e' | 'i' | 'o' | 'u')) {
            None
        } else {
            Some(RefineRejection {
                code: Some(IssueCode::Refine),
                message: std::borrow::Cow::Borrowed("must start with a vowel"),
                params: Vec::new(),
            })
        }
    }
}

#[test]
fn f7_quickstart_compiles_from_prelude_alone() {
    let mut input = Object::new();
    input.insert("name", "ada".to_string().to_value());
    input.insert("age", "42".to_string().to_value());

    let schema = object()
        .field("name", string().min(2).rule(StartsWithVowel))
        .field("age", coerced::<u32>());

    let out = schema.parse(&input).expect("valid input parses");
    assert_eq!(out.get("age"), Some(&Value::I64(42)));

    // Shape-introspection users must be able to NAME the return type.
    let meta: &FieldMeta = schema.field_meta("name").expect("declared key found");
    assert!(meta.label.is_none());
}

#[test]
fn f7_rule_implementor_names_refine_rejection_from_prelude() {
    let schema = string().rule(StartsWithVowel);
    assert!(schema.parse(&"orange".to_string()).is_ok());
    let err = schema.parse(&"pear".to_string()).unwrap_err();
    assert_eq!(err.issues[0].code, IssueCode::Refine);
}
