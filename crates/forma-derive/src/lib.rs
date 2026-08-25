//! Derive entrypoint. All real work lives in [`expansion`]; attribute
//! validation lives in `attrs`.

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod attrs;
mod expansion;

/// Derives a `forma` form schema companion for the annotated struct.
///
/// See the crate-level docs for the supported shapes and attribute grammar.
#[proc_macro_derive(FormSchema, attributes(form))]
pub fn derive_form_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match expansion::expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
