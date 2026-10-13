//! Serde's internal AST stays in adapter.rs; code generation consumes local IR.

#![forbid(unsafe_code)]

mod adapter;
mod attrs;
mod contract;
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
