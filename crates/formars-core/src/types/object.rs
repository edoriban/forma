use std::sync::OnceLock;

use crate::error::{FieldPath, FormaError, FormaIssue, IssueCode, Segment};
use crate::schema::{
    DynSchema, FieldMeta, ObjectChild, ObjectFieldDesc, Schema, ShapeKind, ShapeNode,
};
use crate::value::{Object, Value};

/// One declared field: name, captured child metadata, and the boxed child
/// kernel — the single representation both views and `shape()` derive from.
#[derive(Debug)]
struct Field {
    name: Box<str>,
    meta: FieldMeta,
    child: Box<dyn ObjectChild>,
}

/// Object builder schema: an ordered field registry drives the typed parse,
/// the erased view and `shape()` from one walk kernel (single representation).
///
/// The ordered [`Object`] is core's struct-shaped currency (DV-1): on success
/// the output contains exactly the declared fields, in declaration order,
/// regardless of input insertion order; unknown input keys are dropped.
#[derive(Debug, Default)]
pub struct ObjectSchema {
    fields: Vec<Field>,
    meta: FieldMeta,
    fail_fast: bool,
    shape_cache: OnceLock<ShapeNode>,
}

impl ObjectSchema {
    /// Declares a field backed by any schema family; declaration order is
    /// preserved and governs walk order, issue order and output order.
    ///
    /// Re-declaring an already-declared name replaces the earlier child in
    /// place (last declaration wins, keeping the original position) —
    /// consistent with [`Object::insert`]'s last-write-wins semantics.
    #[must_use]
    pub fn field<C>(mut self, name: &str, child: C) -> Self
    where
        C: ObjectChild + 'static,
    {
        let field = Field {
            name: name.into(),
            meta: child.meta().clone(),
            child: Box::new(child),
        };
        match self.fields.iter_mut().find(|f| f.name.as_ref() == name) {
            Some(existing) => *existing = field,
            None => self.fields.push(field),
        }
        self
    }

    /// Stops at the FIRST violated constraint anywhere — including deep
    /// inside children — with exactly one issue (ER-4).
    #[must_use]
    pub fn fail_fast(mut self) -> Self {
        self.fail_fast = true;
        self
    }

    /// Sets the UI label metadata slot.
    #[must_use]
    pub fn label(mut self, label: &'static str) -> Self {
        self.meta.label = Some(label.into());
        self
    }

    /// Sets the UI description metadata slot.
    #[must_use]
    pub fn description(mut self, description: &'static str) -> Self {
        self.meta.description = Some(description.into());
        self
    }

    /// Sets the UI placeholder metadata slot.
    #[must_use]
    pub fn placeholder(mut self, placeholder: &'static str) -> Self {
        self.meta.placeholder = Some(placeholder.into());
        self
    }

    /// The declared field's own metadata slot, unchanged from its child.
    #[must_use]
    pub fn field_meta(&self, key: &str) -> Option<&FieldMeta> {
        self.fields
            .iter()
            .find(|f| f.name.as_ref() == key)
            .map(|f| &f.meta)
    }

    fn build_shape(&self) -> ShapeNode {
        ShapeNode {
            kind: ShapeKind::Object {
                fields: self
                    .fields
                    .iter()
                    .map(|f| ObjectFieldDesc {
                        key: f.name.clone(),
                        child: f.child.shape_node(),
                    })
                    .collect(),
            },
            constraints: Vec::new(),
        }
    }

    /// The shared walk kernel (D2): per declared field, in order — absent key
    /// yields one `Required` at the joined path; present values (including
    /// `Null`) go to the child with a joined path and effective fail-fast;
    /// successes reconstruct the output in declaration order.
    fn walk(
        &self,
        obj: &Object,
        path: &FieldPath,
        inherited_ff: bool,
    ) -> Result<Object, Vec<FormaIssue>> {
        let eff_ff = self.fail_fast || inherited_ff;
        let mut issues = Vec::new();
        let mut out = Object::new();
        for field in &self.fields {
            let joined = path.join(Segment::Key(field.name.clone()));
            match obj.get(&field.name) {
                None => {
                    issues.push(FormaIssue {
                        path: joined,
                        code: IssueCode::Required,
                        message: "required field is missing".into(),
                        params: Vec::new(),
                    });
                    if eff_ff {
                        return Err(issues);
                    }
                }
                Some(v) => match field.child.validate_at(v, &joined, eff_ff) {
                    Err(mut child_issues) => {
                        issues.append(&mut child_issues);
                        if eff_ff {
                            return Err(issues);
                        }
                    }
                    Ok(validated) => out.insert(&field.name, validated),
                },
            }
        }
        if issues.is_empty() {
            Ok(out)
        } else {
            Err(issues)
        }
    }
}

/// Creates a new empty [`ObjectSchema`].
#[must_use]
pub fn object() -> ObjectSchema {
    ObjectSchema::default()
}

impl Schema for ObjectSchema {
    type Input = Object;
    type Output = Object;

    fn parse(&self, input: &Self::Input) -> Result<Self::Output, FormaError> {
        self.walk(input, &FieldPath::ROOT, false)
            .map_err(|issues| FormaError { issues })
    }
}

impl DynSchema for ObjectSchema {
    fn validate_value(&self, v: &Value) -> Vec<FormaIssue> {
        match v {
            Value::Object(sub) => match self.walk(sub, &FieldPath::ROOT, false) {
                Ok(_) => Vec::new(),
                Err(issues) => issues,
            },
            _ => vec![FormaIssue {
                path: FieldPath::ROOT,
                code: IssueCode::TypeMismatch,
                message: "value is not an object".into(),
                params: Vec::new(),
            }],
        }
    }

    fn shape(&self) -> &ShapeNode {
        self.shape_cache.get_or_init(|| self.build_shape())
    }

    fn metadata(&self) -> &FieldMeta {
        &self.meta
    }
}

impl ObjectChild for ObjectSchema {
    fn validate_at(
        &self,
        v: &Value,
        path: &FieldPath,
        fail_fast: bool,
    ) -> Result<Value, Vec<FormaIssue>> {
        match v {
            Value::Object(sub) => self.walk(sub, path, fail_fast).map(Value::Object),
            _ => Err(vec![FormaIssue {
                path: path.clone(),
                code: IssueCode::TypeMismatch,
                message: "value is not an object".into(),
                params: Vec::new(),
            }]),
        }
    }

    fn shape_node(&self) -> ShapeNode {
        self.shape_cache.get_or_init(|| self.build_shape()).clone()
    }

    fn meta(&self) -> &FieldMeta {
        &self.meta
    }

    fn clone_boxed(&self) -> Box<dyn ObjectChild> {
        Box::new(self.clone())
    }
}

impl Clone for ObjectSchema {
    /// Deep clone: every child is duplicated through its
    /// [`ObjectChild::clone_boxed`] seam; the shape cache starts fresh (same
    /// primitive precedent as the string/number/bool/coerced builders).
    fn clone(&self) -> Self {
        Self {
            fields: self
                .fields
                .iter()
                .map(|f| Field {
                    name: f.name.clone(),
                    meta: f.meta.clone(),
                    child: f.child.clone_boxed(),
                })
                .collect(),
            meta: self.meta.clone(),
            fail_fast: self.fail_fast,
            shape_cache: OnceLock::new(),
        }
    }
}

impl crate::schema::sealed::Sealed for ObjectSchema {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coerce::coerced;
    use crate::schema::DynSchema;
    use crate::types::string;

    #[test]
    fn dv6_shape_derived_from_walked_registry() {
        let s = object()
            .field("name", string().min(1))
            .field("age", coerced::<u32>());
        let shape = s.shape();
        let ShapeKind::Object { fields } = &shape.kind else {
            panic!("expected Object kind");
        };
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_ref()).collect();
        assert_eq!(keys, vec!["name", "age"]);
        assert_eq!(fields[0].child.kind, ShapeKind::Str);
        assert!(matches!(
            fields[0].child.constraints[0].code,
            IssueCode::Min
        ));
        assert_eq!(fields[1].child.kind, ShapeKind::Coerced);
    }

    #[test]
    fn duplicate_field_declaration_replaces_in_place() {
        let s = object()
            .field("name", string().min(5))
            .field("age", coerced::<u32>())
            .field("name", string().min(1));
        let ShapeKind::Object { fields } = &s.shape().kind else {
            panic!("expected Object kind");
        };
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_ref()).collect();
        assert_eq!(keys, vec!["name", "age"], "position kept, no duplicate");
        // last declaration wins: min(1) accepts a 2-char name where min(5) would not
        let mut input = Object::new();
        input.insert("name", Value::from("ab"));
        input.insert("age", Value::from("42"));
        assert!(s.parse(&input).is_ok());
    }
}
