//! Codegen per design Decision 4: a companion `XSchema` owning THE one
//! composed `::formars_core::ObjectSchema`, with both views (`Schema`,
//! `DynSchema`) delegating to it (SC-9), plus the nesting link
//! (`FormSchema for X`) and the value bridge (`FormBridge for X`).
//!
//! Every generated path is absolute `::formars_core::...` (CR-4). Per-field
//! generated tokens reuse the field's original [`proc_macro2::Span`] so
//! trait-bound errors land on the offending field identifier, never inside a
//! wall of expansion noise.
//!
//! # Generated contract: the nested-field DUAL BOUND
//!
//! A field whose type composes as a nested child MUST implement BOTH traits:
//! - [`::formars_core::form::FormSchema`] — supplies the composed child schema
//!   (`Nested::new(<T as FormSchema>::form_schema())`);
//! - [`::formars_core::form::FormBridge`] — supplies the typed-parse bridging
//!   (`to_form_value` on the way in, `from_validated` on reconstruction).
//!
//! A hand-written `FormSchema` impl WITHOUT `FormBridge` therefore fails
//! compilation; the derive emits both impls together for derived structs.
//! This behavioral pin lands as trybuild UI case 13 (validation finding M2).

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote_spanned};
use syn::ext::IdentExt as _;
use syn::spanned::Spanned as _;
use syn::{Data, DeriveInput, Fields, Ident, Type};

use crate::attrs::FieldAttrs;

/// One planned field: Rust identifier (span-carrying), wire key, type, and
/// the validated attribute payload.
struct FieldPlan {
    ident: Ident,
    key: String,
    ty: Type,
    attrs: FieldAttrs,
}

/// Validates the derive target shape (EX-1): braced structs with named
/// fields only — no enums, unions, tuple/unit structs, generics or lifetimes.
fn check_item_shape(input: &DeriveInput) -> Result<(), syn::Error> {
    use syn::spanned::Spanned;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "`#[derive(FormSchema)]` does not support generic parameters in v0; \
             only plain named-field structs are supported",
        ));
    }
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(_) => Ok(()),
            Fields::Unnamed(fields) => Err(syn::Error::new(
                fields.span(),
                "`#[derive(FormSchema)]` does not support tuple structs in v0; \
                 only plain named-field structs are supported",
            )),
            Fields::Unit => Err(syn::Error::new(
                input.ident.span(),
                "`#[derive(FormSchema)]` does not support unit structs in v0; \
                 only plain named-field structs are supported",
            )),
        },
        Data::Enum(data) => Err(syn::Error::new(
            data.enum_token.span(),
            "`#[derive(FormSchema)]` does not support enums in v0; \
             only plain named-field structs are supported",
        )),
        Data::Union(data) => Err(syn::Error::new(
            data.union_token.span(),
            "`#[derive(FormSchema)]` does not support unions in v0; \
             only plain named-field structs are supported",
        )),
    }
}

/// Rejects ANY container-level `#[form(..)]` as unrecognized (AT-5): v0 has
/// no container attributes, and they must not be silently ignored.
fn reject_container_attrs(input: &DeriveInput) -> Result<(), syn::Error> {
    for attr in &input.attrs {
        if attr.path().is_ident("form") {
            return Err(syn::Error::new(
                attr.span(),
                "container-level `#[form(..)]` attributes do not exist in v0; \
                 `#[form]` is recognized only on fields of a struct deriving `FormSchema`",
            ));
        }
    }
    Ok(())
}

impl FieldPlan {
    fn new(field: &syn::Field) -> Result<Option<Self>, syn::Error> {
        let Some(ident) = field.ident.clone() else {
            return Ok(None);
        };
        let attrs = FieldAttrs::parse(&field.attrs)?;
        // `unraw`: a raw identifier `r#type` names the wire key `type`
        // (the r# prefix is Rust syntax, never part of the object key).
        let key = attrs
            .rename
            .as_ref()
            .map_or_else(|| ident.unraw().to_string(), syn::LitStr::value);
        Ok(Some(Self {
            ident,
            key,
            ty: field.ty.clone(),
            attrs,
        }))
    }

    /// True when this plan composes a nested derived child (no `schema`
    /// override and a non-primitive type).
    fn is_nested(&self) -> bool {
        self.attrs.schema.is_none() && !is_known_primitive(&self.ty)
    }
}

/// The EX-4 primitive set recognized by name mapping (`String`, `bool`, and
/// the coerced numeric matrix). Everything else composes as a nested child.
/// Single source of truth for every name-based mapping decision.
const KNOWN_PRIMITIVES: &[&str] = &[
    "String", "bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "f32", "f64",
];

/// The coerced numeric subset of [`KNOWN_PRIMITIVES`] whose wire
/// representation is the form string `coerced::<T>()` consumes.
const NUMERIC_MATRIX: &[&str] = &["i8", "i16", "i32", "i64", "u8", "u16", "u32", "f32", "f64"];

fn is_known_primitive(ty: &Type) -> bool {
    single_segment_ident(ty).is_some_and(|segment| KNOWN_PRIMITIVES.contains(&segment.as_str()))
}

/// Default field mapping (EX-4): `String → string()`, `bool → bool()`,
/// integers/floats of the supported matrix → `coerced::<T>()`; anything else
/// composes as a nested child via `FormSchema`. The expression is spanned at
/// the field identifier so downstream bound errors point at the field.
///
/// NO syntactic whitelist guessing beyond this matrix: unsupported types fail
/// bound-check on the generated `FormBridge` requirement, whose
/// `on_unimplemented` help names the `#[form(schema = ..)]` escape hatch.
fn default_child(plan: &FieldPlan) -> TokenStream {
    let span = plan.ident.span();
    let ty = &plan.ty;
    let segment = single_segment_ident(ty);
    if segment.as_deref() == Some("String") {
        return quote_spanned! { span=> ::formars_core::types::string() };
    }
    if segment.as_deref() == Some("bool") {
        return quote_spanned! { span=> ::formars_core::types::bool() };
    }
    if segment.is_some_and(|name| NUMERIC_MATRIX.contains(&name.as_str())) {
        return quote_spanned! { span=> ::formars_core::coerce::coerced::<#ty>() };
    }
    quote_spanned! { span=>
        ::formars_core::schema::Nested::new(<#ty as ::formars_core::form::FormSchema>::form_schema())
    }
}

/// The field's full child expression: `schema` override verbatim (AT-1),
/// otherwise the EX-4 default mapping — then any metadata builder calls
/// appended to the chain (AT-4).
fn child_expr(plan: &FieldPlan) -> TokenStream {
    let span = plan.ident.span();
    let mut child = match &plan.attrs.schema {
        Some(expr) => quote_spanned! { span=> #expr },
        None => default_child(plan),
    };
    if let Some(label) = &plan.attrs.label {
        quote_spanned! { span=> #child.label(#label) }.clone_into(&mut child);
    }
    if let Some(description) = &plan.attrs.description {
        quote_spanned! { span=> #child.description(#description) }.clone_into(&mut child);
    }
    if let Some(placeholder) = &plan.attrs.placeholder {
        quote_spanned! { span=> #child.placeholder(#placeholder) }.clone_into(&mut child);
    }
    child
}

fn single_segment_ident(ty: &Type) -> Option<String> {
    if let Type::Path(path) = ty
        && path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].arguments.is_empty()
    {
        return Some(path.path.segments[0].ident.to_string());
    }
    None
}

/// The final path segment's identifier when it carries no generic arguments,
/// regardless of how many segments precede it.
fn final_segment_ident(ty: &Type) -> Option<String> {
    if let Type::Path(path) = ty
        && path.qself.is_none()
        && let Some(segment) = path.path.segments.last()
        && segment.arguments.is_empty()
    {
        return Some(segment.ident.to_string());
    }
    None
}

/// Qualified-primitive guard: a multi-segment path whose FINAL segment is a
/// known primitive (`::std::string::String`, `::core::primitive::u32`) is NOT
/// mapped by name — name mapping keys on single-segment paths only. Emit the
/// targeted diagnostic instead of letting the field fall through to nested
/// composition and die as E0277 bound noise suggesting `FormSchema` on the
/// primitive. Schema overrides win (mapping layer never consulted); bare
/// single-segment names take the normal mapping path untouched.
fn reject_qualified_primitives(plans: &[FieldPlan]) -> Result<(), syn::Error> {
    for plan in plans {
        if plan.attrs.schema.is_some() || single_segment_ident(&plan.ty).is_some() {
            continue;
        }
        if let Some(primitive) = final_segment_ident(&plan.ty)
            && KNOWN_PRIMITIVES.contains(&primitive.as_str())
        {
            return Err(syn::Error::new(
                plan.ident.span(),
                format!(
                    "field `{}`: primitive `{primitive}` written as a multi-segment path \
                     is not mapped by name; use the bare type name (`{primitive}`) or \
                     `#[form(schema = ..)]`",
                    plan.ident
                ),
            ));
        }
    }
    Ok(())
}

/// True for the EX-4 numeric matrix (`i8..i64`, `u8..u32`, `f32`, `f64`),
/// whose wire representation is the form-string the `coerced::<T>()` child
/// consumes.
fn is_numeric_matrix(ty: &Type) -> bool {
    single_segment_ident(ty).is_some_and(|segment| NUMERIC_MATRIX.contains(&segment.as_str()))
}

/// Expands the derive into the full generated contract.
///
/// The function is one linear codegen script by design: splitting it would
/// scatter the emitted contract across helpers without reducing complexity.
#[allow(
    clippy::too_many_lines,
    reason = "linear codegen script; each emitted impl block reads top-to-bottom"
)]
pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream, syn::Error> {
    check_item_shape(input)?;
    reject_container_attrs(input)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named,
            _ => unreachable!("checked by check_item_shape"),
        },
        _ => unreachable!("checked by check_item_shape"),
    };
    let mut plans = Vec::new();
    for field in &fields.named {
        if let Some(plan) = FieldPlan::new(field)? {
            plans.push(plan);
        }
    }

    // Early DQ-2 validation: metadata cannot attach to a `Nested` child —
    // core exposes metadata hooks only on builder families, and extending the
    // sanctioned core delta for it is out of scope. Users compose such slots
    // via `#[form(schema = ..)]`.
    for plan in &plans {
        if plan.is_nested()
            && (plan.attrs.label.is_some()
                || plan.attrs.description.is_some()
                || plan.attrs.placeholder.is_some())
        {
            return Err(syn::Error::new(
                plan.ident.span(),
                format!(
                    "metadata attributes (label/description/placeholder) are not supported on \
                     nested field `{}` in v0; use `#[form(schema = <expr>)]` to compose the slot",
                    plan.ident
                ),
            ));
        }
    }

    // Qualified-primitive guard: multi-segment primitive paths are not
    // mapped by name; targeted diagnostic instead of downstream E0277 noise.
    reject_qualified_primitives(&plans)?;

    let active: Vec<&FieldPlan> = plans.iter().filter(|p| !p.attrs.skip).collect();
    let skipped: Vec<&FieldPlan> = plans.iter().filter(|p| p.attrs.skip).collect();

    // Dup-wire-key rejection (pre-1.0 accepted break): after rename
    // resolution, two non-skipped fields resolving to the SAME wire key would
    // silently collapse last-write-wins in core's object lookup. Skipped
    // fields never enter `active`, so they are exempt by construction.
    let mut seen: std::collections::HashMap<&str, &Ident> = std::collections::HashMap::new();
    for plan in &active {
        if let Some(previous) = seen.insert(plan.key.as_str(), &plan.ident) {
            return Err(syn::Error::new(
                plan.ident.span(),
                format!(
                    "duplicate wire key `\"{}\"`: fields `{}` and `{}` both resolve to it; \
                     object lookup is last-write-wins, so rename one with \
                     `#[form(rename = \"..\")]`",
                    plan.key, previous, plan.ident
                ),
            ));
        }
    }

    let struct_ident = &input.ident;
    let companion = format_ident!("{}Schema", input.ident);
    let vis = &input.vis;

    // --- composed representation: declaration order, one ObjectSchema (EX-2)
    let field_registrations = active.iter().map(|p| {
        let key = &p.key;
        let child = child_expr(p);
        quote_spanned! { p.ident.span()=> .field(#key, #child) }
    });

    // --- typed parse: reconstruction (EX-6). The bridging inserts feeding
    // validation are exactly `FormBridge::to_form_value` on the input, so
    // `Schema::parse` reuses it instead of duplicating the insert codegen.
    //
    // Wire asymmetry for the coerced matrix (EX-4): HTML-form currency means
    // numeric fields travel INTO validation as strings (what `coerced::<T>()`
    // consumes) while their validated outputs are `Value::I64/F64`, read back
    // via `FormBridge::from_validated`. Skipped fields are omitted entirely.
    let reconstruction_fields: Vec<TokenStream> = plans.iter().map(reconstruction_field).collect();

    // --- FormBridge for X: field-wise bridging (Decision 4e); skipped fields
    // are omitted from `to_form_value` (consistent with the schema) and read
    // back under their Rust identifier in `from_validated` (v0 asymmetry:
    // bridged round-trips of skipped payloads are not guaranteed).
    let mut to_form_inserts = Vec::new();
    for p in &active {
        let key = &p.key;
        let ident = &p.ident;
        let insert = if is_numeric_matrix(&p.ty) {
            quote_spanned! { p.ident.span()=>
                ::formars_core::value::Value::from(
                    ::std::string::ToString::to_string(&self.#ident),
                )
            }
        } else {
            let ty = &p.ty;
            quote_spanned! { p.ident.span()=>
                <#ty as ::formars_core::form::FormBridge>::to_form_value(&self.#ident)
            }
        };
        to_form_inserts.push(quote_spanned! { p.ident.span()=>{
            ::formars_core::value::Object::insert(&mut __obj, #key, #insert);
        }});
    }
    let mut from_validated_fields = Vec::new();
    for p in &active {
        let key = &p.key;
        let ident = &p.ident;
        let ty = &p.ty;
        from_validated_fields.push(quote_spanned! { p.ident.span()=>
            #ident: __obj
                .get(#key)
                .and_then(<#ty as ::formars_core::form::FormBridge>::from_validated)?,
        });
    }
    // v0 limitation: skipped payloads do not survive erased round-trips; they
    // are preserved verbatim only by the typed `parse(&X)` path, which owns
    // the input. The erased bridge therefore declines such reconstructions.
    let (from_validated_param, from_validated_body) = if skipped.is_empty() {
        (
            quote_spanned! { Span::call_site()=> v },
            quote_spanned! { Span::call_site()=>
                let ::formars_core::value::Value::Object(__obj) = v else {
                    return ::std::option::Option::None;
                };
                ::std::option::Option::Some(Self {
                    #(#from_validated_fields)*
                })
            },
        )
    } else {
        (
            quote_spanned! { Span::call_site()=> _ },
            quote_spanned! { Span::call_site()=> ::std::option::Option::None },
        )
    };

    let doc =
        format!("Companion schema generated by `#[derive(FormSchema)]` for [`{struct_ident}`].");

    let expanded = quote_spanned! { Span::call_site()=>
        #[automatically_derived]
        #[derive(Clone)]
        #[doc = #doc]
        #vis struct #companion {
            object: ::formars_core::types::ObjectSchema,
        }

        #[automatically_derived]
        impl #companion {
            /// Builds the full composed representation in declaration order.
            #[must_use]
            pub fn new() -> Self {
                Self {
                    object: ::formars_core::types::object()
                        #(#field_registrations)*
                }
            }

            /// The declared field's own metadata slot, served from the one
            /// composed representation.
            #[must_use]
            pub fn field_meta(&self, key: &str)
                -> ::std::option::Option<&::formars_core::schema::FieldMeta> {
                self.object.field_meta(key)
            }
        }

        #[automatically_derived]
        impl ::std::default::Default for #companion {
            fn default() -> Self {
                Self::new()
            }
        }

        #[automatically_derived]
        impl ::std::convert::AsRef<::formars_core::types::ObjectSchema> for #companion {
            fn as_ref(&self) -> &::formars_core::types::ObjectSchema {
                &self.object
            }
        }

        #[automatically_derived]
        impl ::std::fmt::Debug for #companion {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                // The composed object carries all state worth printing.
                ::std::fmt::Debug::fmt(&self.object, f)
            }
        }

        #[automatically_derived]
        impl ::formars_core::schema::Schema for #companion {
            type Input = #struct_ident;
            type Output = #struct_ident;

            fn parse(
                &self,
                input: &#struct_ident,
            ) -> ::std::result::Result<#struct_ident, ::formars_core::error::FormaError> {
                let ::formars_core::value::Value::Object(__obj) =
                    <#struct_ident as ::formars_core::form::FormBridge>::to_form_value(input)
                else {
                    ::std::unreachable!("derived `to_form_value` always returns `Value::Object`")
                };
                let __validated = ::formars_core::schema::Schema::parse(&self.object, &__obj)?;
                ::std::result::Result::Ok(#struct_ident {
                    #(#reconstruction_fields)*
                })
            }
        }

        #[automatically_derived]
        impl ::formars_core::schema::DynSchema for #companion {
            fn validate_value(
                &self,
                v: &::formars_core::value::Value,
            ) -> ::std::vec::Vec<::formars_core::error::FormaIssue> {
                ::formars_core::schema::DynSchema::validate_value(&self.object, v)
            }

            fn shape(&self) -> &::formars_core::schema::ShapeNode {
                ::formars_core::schema::DynSchema::shape(&self.object)
            }

            fn metadata(&self) -> &::formars_core::schema::FieldMeta {
                ::formars_core::schema::DynSchema::metadata(&self.object)
            }
        }

        #[automatically_derived]
        impl ::formars_core::form::FormSchema for #struct_ident {
            type Schema = #companion;

            fn form_schema() -> #companion {
                #companion::new()
            }
        }

        #[automatically_derived]
        impl ::formars_core::form::FormBridge for #struct_ident {
            fn to_form_value(&self) -> ::formars_core::value::Value {
                let mut __obj = ::formars_core::value::Object::new();
                #(#to_form_inserts)*
                ::formars_core::value::Value::Object(__obj)
            }

            fn from_validated(#from_validated_param: &::formars_core::value::Value) -> ::std::option::Option<Self> {
                #from_validated_body
            }
        }
    };

    Ok(expanded)
}

/// Reconstruction of one field from the validated output (EX-6): non-skipped
/// fields come from their wire keys via `FormBridge`; SKIPPED fields pass
/// through VERBATIM from the input (never defaulted), requiring `Clone`.
/// Post-validation lookup failures are impossible; they are defensively
/// mapped to a `TypeMismatch` issue at the field's joined path — never a panic.
fn reconstruction_field(p: &FieldPlan) -> TokenStream {
    let ident = &p.ident;
    if p.attrs.skip {
        return quote_spanned! { p.ident.span()=>
            #ident: ::std::clone::Clone::clone(&input.#ident),
        };
    }
    let key = &p.key;
    let ty = &p.ty;
    quote_spanned! { p.ident.span()=>
        #ident: match ::formars_core::value::Object::get(&__validated, #key)
            .and_then(<#ty as ::formars_core::form::FormBridge>::from_validated)
        {
            ::std::option::Option::Some(__field) => __field,
            ::std::option::Option::None => {
                return ::std::result::Result::Err(::formars_core::error::FormaError {
                    issues: ::std::vec![::formars_core::error::FormaIssue {
                        path: ::formars_core::error::FieldPath::key(#key),
                        code: ::formars_core::error::IssueCode::TypeMismatch,
                        message: ::std::borrow::Cow::Borrowed(
                            "validated output is missing this field",
                        ),
                        params: ::std::vec::Vec::new(),
                    }],
                });
            }
        },
    }
}
