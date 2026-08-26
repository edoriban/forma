//! FA-IN-2 / FA-DOC-2 (ungated): the validate-only tier is a one-import
//! program under DEFAULT features, and the core prelude inventory is a CLOSED
//! list of exactly 17 names. Multi-path identity (FA-IN-7) and advanced-mirror
//! resolution (FA-IN-1) are pinned through `formars::formars_core` paths —
//! fully qualified, so this file's only toolkit import is the prelude glob.

use formars::prelude::*;

/// Local custom rule exercising the `Rule` extension point through the
/// builder seam exactly as the core doctest does. The rejection payload type
/// is reached via the whole-crate mirror (it is deliberately NOT part of the
/// 17-name prelude).
#[derive(Debug)]
struct StartsWith(char);

impl Rule<str> for StartsWith {
    fn name(&self) -> &'static str {
        "starts_with"
    }

    fn validate(&self, value: &str) -> Option<formars::formars_core::rule::RefineRejection> {
        if value.starts_with(self.0) {
            None
        } else {
            Some(formars::formars_core::rule::RefineRejection {
                code: Some(IssueCode::Refine),
                message: "must start with the expected char".into(),
                params: Vec::new(),
            })
        }
    }
}

fn signup_schema() -> ObjectSchema {
    object()
        .field("email", string().min(8).email())
        .field("age", coerced::<u32>())
}

fn input(email: &str, age: &str) -> formars::formars_core::value::Object {
    // `Object` travels through the advanced mirror path, keeping the import
    // surface of this file at exactly one line.
    let mut o = formars::formars_core::value::Object::new();
    o.insert("email", Value::from(email));
    o.insert("age", Value::from(age));
    o
}

#[test]
fn one_import_program_builds_parses_and_reports() {
    let schema = signup_schema();

    let parsed = schema
        .parse(&input("user@example.com", "36"))
        .expect("valid input must parse");
    assert_eq!(parsed.get("age"), Some(&Value::I64(36)));

    let Err(err) = string().min(8).parse(&"short".to_string()) else {
        panic!("too-short input must fail");
    };
    let first = err.first().expect("at least one issue on failure");
    assert_eq!(first.code, IssueCode::Min);
    assert_eq!(first.path, FieldPath::ROOT);
}

#[test]
fn invalid_input_accumulates_path_addressed_issues() {
    let Err(err) = signup_schema().parse(&input("user@example.com", "abc")) else {
        panic!("non-coercible age must fail");
    };
    let ages: Vec<_> = err
        .issues_for(&FieldPath::key("age"))
        .map(|i| i.code.clone())
        .collect();
    assert_eq!(ages, vec![IssueCode::Coerce]);
    assert_eq!(err.issues.len(), 1, "single violated constraint");
}

#[test]
fn custom_rule_composes_like_builtins() {
    let schema = string().min(2).rule(StartsWith('a'));
    assert!(schema.parse(&"apple".to_string()).is_ok());
    let Err(err) = schema.parse(&"pear".to_string()) else {
        panic!("rule rejection must fail");
    };
    assert_eq!(err.first().expect("issue").code, IssueCode::Refine);
}

#[test]
fn core_prelude_inventory_is_the_closed_17_name_list() {
    // Compilation IS the audit: every one of the 17 names resolves through
    // the single glob import; nothing else needs importing.
    fn accept<T>(_: T) {}
    fn bridges<T: FormBridge>(v: &T) -> Value {
        v.to_form_value()
    }
    fn parses<S: Schema>(schema: &S, input: &S::Input) -> bool {
        schema.parse(input).is_ok()
    }
    fn ruled<T: ?Sized + Rule<str>>(r: &T) -> &str {
        r.name()
    }
    fn companion_of<T>() -> Box<dyn DynSchema>
    where
        T: FormSchema,
        T::Schema: 'static,
    {
        Box::new(<T as FormSchema>::form_schema())
    }

    /// Nested carrier with exactly the `AsRef<ObjectSchema>` contract derive
    /// companions rely on (companions are `Clone`, mirroring this).
    #[derive(Debug, Clone)]
    struct Composed(ObjectSchema);
    impl AsRef<ObjectSchema> for Composed {
        fn as_ref(&self) -> &ObjectSchema {
            &self.0
        }
    }

    // Builder functions (coerced, bool, number, object, string).
    accept(coerced::<u32>());
    accept(string());
    accept(bool());
    accept(number::<i64>());
    let _: ObjectSchema = object();

    // Error vocabulary (FieldPath, FormaError, FormaIssue, IssueCode).
    let _: FieldPath = FieldPath::key("path");
    let _: FormaError = FormaError::default();
    let _: FormaIssue = FormaIssue {
        path: FieldPath::ROOT,
        code: IssueCode::Required,
        message: "missing".into(),
        params: Vec::new(),
    };
    let root_code = IssueCode::Min;
    assert_eq!(root_code, IssueCode::Min);

    // Trait positions (FormBridge, Schema, Rule, FormSchema, DynSchema).
    assert_eq!(bridges(&7u32), Value::I64(7));
    assert_eq!(bridges(&true), Value::Bool(true));
    assert_eq!(bridges(&String::from("x")), Value::from("x"));

    assert!(parses(&string(), &"anything".to_string()));

    assert_eq!(ruled(&StartsWith('a')), "starts_with");

    let erased = companion_of::<Dummy>();
    assert!(
        erased.validate_value(&Value::Bool(false)).is_empty(),
        "manual companion erases cleanly"
    );

    // Adapter position (Nested over an AsRef<ObjectSchema> carrier).
    let nested = Nested::new(Composed(object()));
    let composed = object().field("inner", nested);
    let mut inner = formars::formars_core::value::Object::new();
    inner.insert("qty", Value::from("1"));
    let mut outer = formars::formars_core::value::Object::new();
    outer.insert("inner", Value::Object(inner));
    assert!(
        composed.parse(&outer).is_ok(),
        "nested composition validates"
    );

    // Value closes the list.
    let _: Value = Value::Null;
}

/// Manual companion proving the TRAIT half of the inventory without the
/// derive feature: `FormSchema` + `Schema` + `DynSchema` cooperate over a
/// hand-written impl (the path pre-derive types take).
#[derive(Debug)]
struct Dummy;

#[derive(Debug)]
struct AlwaysOk;

impl Schema for AlwaysOk {
    type Input = Dummy;
    type Output = Dummy;

    fn parse(&self, _input: &Dummy) -> Result<Dummy, FormaError> {
        Ok(Dummy)
    }
}

impl DynSchema for AlwaysOk {
    fn validate_value(&self, _v: &Value) -> Vec<FormaIssue> {
        Vec::new()
    }

    fn shape(&self) -> &formars::formars_core::schema::ShapeNode {
        static SHAPE: formars::formars_core::schema::ShapeNode =
            formars::formars_core::schema::ShapeNode {
                kind: formars::formars_core::schema::ShapeKind::Bool,
                constraints: Vec::new(),
            };
        &SHAPE
    }

    fn metadata(&self) -> &formars::formars_core::schema::FieldMeta {
        static META: formars::formars_core::schema::FieldMeta =
            formars::formars_core::schema::FieldMeta {
                label: None,
                description: None,
                placeholder: None,
                extra: Vec::new(),
            };
        &META
    }
}

impl FormSchema for Dummy {
    type Schema = AlwaysOk;

    fn form_schema() -> AlwaysOk {
        AlwaysOk
    }
}

#[test]
fn form_schema_trait_quartet_works_without_derive() {
    let companion = <Dummy as FormSchema>::form_schema();
    assert!(
        companion.validate_value(&Value::Bool(true)).is_empty(),
        "erased companion accepts anything"
    );
    let out = companion.parse(&Dummy).expect("typed identity parse");
    let _: Dummy = out;
}

#[test]
fn multi_path_identity_and_mirror_resolution() {
    // Prelude `Value` and mirror-path `Value`: assignment compiles BOTH ways
    // (same DefId — one type, many paths).
    let via_prelude: Value = Value::from("same");
    let via_mirror: formars::formars_core::value::Value = via_prelude;
    let back: Value = via_mirror;
    assert_eq!(back, Value::from("same"));

    // Advanced-mirror resolution behaves identically to the prelude path.
    let mirrored = formars::formars_core::prelude::object()
        .field("age", formars::formars_core::prelude::coerced::<u32>());
    let local = object().field("age", coerced::<u32>());
    let sample = input("ignored", "7");
    assert_eq!(
        mirrored.parse(&sample).map(|o| o.get("age").cloned()),
        local.parse(&sample).map(|o| o.get("age").cloned())
    );
}
