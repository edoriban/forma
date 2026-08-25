//! Closed `#[form(...)]` grammar: parsing and early validation with spans
//! (design Decision 5, spec AT-5/DQ-2). All errors fire BEFORE any expansion.

use proc_macro2::Span;
use syn::spanned::Spanned;

/// Parsed `#[form(...)]` payload of ONE field.
#[derive(Default)]
pub(crate) struct FieldAttrs {
    pub(crate) schema: Option<syn::Expr>,
    pub(crate) rename: Option<syn::LitStr>,
    pub(crate) skip: bool,
    /// Span of the `skip` key itself, for conflict diagnostics.
    skip_span: Option<Span>,
    pub(crate) label: Option<syn::LitStr>,
    pub(crate) description: Option<syn::LitStr>,
    pub(crate) placeholder: Option<syn::LitStr>,
}

const VALID_KEYS: &str = "schema, rename, skip, label, description, placeholder";

impl FieldAttrs {
    /// Parses every `#[form(...)]` attribute on one field. Unknown keys,
    /// duplicates, malformed values and `skip` conflicts are hard errors
    /// spanned at the offending token.
    pub(crate) fn parse(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut out = Self::default();
        for attr in attrs.iter().filter(|a| a.path().is_ident("form")) {
            attr.parse_nested_meta(|meta| {
                let key_span = meta.path.span();
                let key = meta
                    .path
                    .get_ident()
                    .map_or_else(|| "::".to_string(), std::string::ToString::to_string);
                match key.as_str() {
                    "schema" => set(
                        &mut out.schema,
                        meta.value()?.parse::<syn::Expr>()?,
                        key_span,
                    ),
                    "rename" => set(
                        &mut out.rename,
                        meta.value()?.parse::<syn::LitStr>()?,
                        key_span,
                    ),
                    "label" => set(
                        &mut out.label,
                        meta.value()?.parse::<syn::LitStr>()?,
                        key_span,
                    ),
                    "description" => set(
                        &mut out.description,
                        meta.value()?.parse::<syn::LitStr>()?,
                        key_span,
                    ),
                    "placeholder" => set(
                        &mut out.placeholder,
                        meta.value()?.parse::<syn::LitStr>()?,
                        key_span,
                    ),
                    "skip" => {
                        if out.skip {
                            return Err(syn::Error::new(key_span, "duplicate `skip` attribute"));
                        }
                        out.skip = true;
                        out.skip_span = Some(key_span);
                        Ok(())
                    }
                    other => Err(syn::Error::new(
                        key_span,
                        format!("unknown attribute `{other}`; expected one of: {VALID_KEYS}"),
                    )),
                }
            })?;
        }

        let FieldAttrs {
            ref schema,
            ref rename,
            skip,
            skip_span,
            ref label,
            ref description,
            ref placeholder,
        } = out;
        if skip
            && has_any(
                rename.as_ref(),
                schema.as_ref(),
                label.as_ref(),
                description.as_ref(),
                placeholder.as_ref(),
            )
        {
            return Err(syn::Error::new(
                skip_span.unwrap_or_else(Span::call_site),
                "`skip` cannot be combined with any other `#[form(..)]` key: \
                 a skipped field has nothing left to configure",
            ));
        }
        Ok(out)
    }
}

/// Sets `slot`, rejecting a duplicate key at the key's own span.
fn set<T>(slot: &mut Option<T>, value: T, span: proc_macro2::Span) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(span, "duplicate attribute"));
    }
    *slot = Some(value);
    Ok(())
}

fn has_any(
    rename: Option<&syn::LitStr>,
    schema: Option<&syn::Expr>,
    label: Option<&syn::LitStr>,
    description: Option<&syn::LitStr>,
    placeholder: Option<&syn::LitStr>,
) -> bool {
    rename.is_some()
        || schema.is_some()
        || label.is_some()
        || description.is_some()
        || placeholder.is_some()
}

#[cfg(test)]
mod tests {
    use super::{FieldAttrs, VALID_KEYS};
    use syn::parse_quote;

    fn parse_field(field: &syn::Field) -> syn::Result<FieldAttrs> {
        FieldAttrs::parse(&field.attrs)
    }

    #[test]
    fn c1_accepted_shapes_populate_all_slots() {
        let f: syn::Field = parse_quote! {
            #[form(rename = "wire")]
            #[form(label = "L", description = "D", placeholder = "P")]
            name: String
        };
        let attrs = parse_field(&f).expect("valid attributes");
        assert_eq!(
            attrs.rename.as_ref().map(syn::LitStr::value),
            Some("wire".to_string())
        );
        assert_eq!(
            attrs.label.as_ref().map(syn::LitStr::value),
            Some("L".to_string())
        );
        assert_eq!(
            attrs.description.as_ref().map(syn::LitStr::value),
            Some("D".to_string())
        );
        assert_eq!(
            attrs.placeholder.as_ref().map(syn::LitStr::value),
            Some("P".to_string())
        );
        assert!(!attrs.skip);
        assert!(attrs.schema.is_none());

        let f: syn::Field = parse_quote! {
            #[form(schema = string().trim().min(2))]
            code: String
        };
        let attrs = parse_field(&f).expect("schema override parses");
        assert!(attrs.schema.is_some());
    }

    #[test]
    fn c1_skip_flag_form_parses() {
        let f: syn::Field = parse_quote! {
            #[form(skip)]
            secret: u32
        };
        let attrs = parse_field(&f).expect("bare flag parses");
        assert!(attrs.skip);
    }

    #[test]
    fn c1_unknown_key_errors_listing_all_six_valid_keys() {
        let f: syn::Field = parse_quote! {
            #[form(lable = "Name")]
            name: String
        };
        let syn::Result::Err(err) = parse_field(&f) else {
            panic!("typo'd key must be rejected");
        };
        let msg = err.to_string();
        assert!(msg.contains("`lable`"), "names the offender: {msg}");
        for k in [
            "schema",
            "rename",
            "skip",
            "label",
            "description",
            "placeholder",
        ] {
            assert!(msg.contains(k), "lists valid key {k}: {msg}");
        }
        assert_eq!(VALID_KEYS.split(", ").count(), 6);
    }

    #[test]
    fn c1_duplicate_key_errors() {
        let f: syn::Field = parse_quote! {
            #[form(rename = "a")]
            #[form(rename = "b")]
            name: String
        };
        let syn::Result::Err(err) = parse_field(&f) else {
            panic!("duplicate must be rejected");
        };
        assert!(err.to_string().contains("duplicate"), "{err}");
    }

    #[test]
    fn c1_skip_combined_with_any_other_key_conflicts() {
        for conflict in [
            quote::quote! { #[form(skip, rename = "r")] },
            quote::quote! { #[form(skip, label = "l")] },
            quote::quote! { #[form(skip, schema = string())] },
            quote::quote! { #[form(skip, description = "d")] },
            quote::quote! { #[form(skip, placeholder = "p")] },
        ] {
            let f: syn::Field = syn::parse_quote! {
                #conflict
                name: String
            };
            let syn::Result::Err(err) = parse_field(&f) else {
                panic!("skip conflicts must be rejected");
            };
            assert!(err.to_string().contains("cannot be combined"), "{err}");
        }
    }

    #[test]
    fn c1_rename_with_non_literal_value_errors() {
        let f: syn::Field = parse_quote! {
            #[form(rename = 42)]
            name: String
        };
        assert!(parse_field(&f).is_err(), "non-literal rename rejected");

        // Missing value after `schema =` is also malformed.
        let f: syn::Field = parse_quote! {
            #[form(schema)]
            name: String
        };
        assert!(
            parse_field(&f).is_err(),
            "`schema` without `= expr` rejected"
        );
    }

    // Full span RENDERING cannot be asserted here: under `cfg(test)` the
    // proc-macro2 fallback makes every span equivalent. Span quality is
    // pinned by the trybuild snapshot suite (Phase D) instead.

    #[test]
    fn c1_bare_field_parses_to_defaults() {
        let f: syn::Field = parse_quote! { email: String };
        let attrs = parse_field(&f).expect("no attrs is fine");
        assert!(!attrs.skip);
        assert!(attrs.schema.is_none());
        assert!(attrs.rename.is_none());
        assert!(attrs.label.is_none());
        assert!(attrs.description.is_none());
        assert!(attrs.placeholder.is_none());
    }
}
