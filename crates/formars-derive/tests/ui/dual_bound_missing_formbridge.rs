use formars_core::error::FormaError;
use formars_core::form::{FormSchema, Schema};
use formars_core::schema::{DynSchema, FieldMeta, ShapeNode};
use formars_core::value::Value;
use formars_derive::FormSchema;

// Hand-written companion WITHOUT a FormBridge impl for `Inner` (M2 pin).
pub struct InnerSchema {
    object: ::formars_core::types::ObjectSchema,
}

impl ::std::fmt::Debug for InnerSchema {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::std::fmt::Debug::fmt(&self.object, f)
    }
}

impl InnerSchema {
    pub fn new() -> Self {
        Self {
            object: ::formars_core::types::object()
                .field("q", ::formars_core::types::string()),
        }
    }
}

impl Schema for InnerSchema {
    type Input = Inner;
    type Output = Inner;

    fn parse(&self, input: &Inner) -> Result<Inner, FormaError> {
        Ok(Inner { q: input.q.clone() })
    }
}

impl DynSchema for InnerSchema {
    fn validate_value(&self, _v: &Value) -> Vec<formars_core::error::FormaIssue> {
        Vec::new()
    }

    fn shape(&self) -> &ShapeNode {
        unreachable!("not exercised by this pin")
    }

    fn metadata(&self) -> &FieldMeta {
        unreachable!("not exercised by this pin")
    }
}

#[derive(Debug)]
struct Inner {
    q: String,
}

impl FormSchema for Inner {
    type Schema = InnerSchema;

    fn form_schema() -> InnerSchema {
        InnerSchema::new()
    }
}

#[derive(FormSchema)]
struct Outer {
    inner: Inner,
}

fn main() {}
