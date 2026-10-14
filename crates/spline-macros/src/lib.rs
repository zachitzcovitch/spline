//! Serde's internal AST stays in adapter.rs; code generation consumes local IR.

#![forbid(unsafe_code)]

mod adapter;
mod api;
mod attrs;
mod contract;
mod execution;
mod expand;
mod http;
mod model;

use proc_macro::TokenStream;

#[proc_macro_derive(Schema, attributes(serde, spline))]
pub fn schema(input: TokenStream) -> TokenStream {
    let result = syn::parse(input).and_then(expand::expand);
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

#[proc_macro_attribute]
pub fn contract(attributes: TokenStream, input: TokenStream) -> TokenStream {
    contract::expand(attributes.into(), input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Compose resource contracts into a neutral application API with optional bindings.
#[proc_macro]
pub fn api(input: TokenStream) -> TokenStream {
    api::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
