use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use crate::{
    adapter, attrs,
    model::{Data, Mode},
};

pub fn expand(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let options = attrs::parse(&input.attrs, true)?;
    let runtime = options
        .runtime
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::spline));
    let whole = status(&input.attrs)?;
    let ident = &input.ident;
    let mut constants = Vec::new();
    let mut rules = Vec::new();
    let mut arms = Vec::new();

    let mut add_status = |expression: syn::Expr| {
        let name = format_ident!("__SPLINE_HTTP_STATUS_{}", constants.len());
        constants.push(quote_spanned! {expression.span()=>
            const #name: #runtime::http::StatusCode = #expression;
            const _: () = ::core::assert!(#name.as_u16() >= 400 && #name.as_u16() < 600,
                "HTTP application error status must be 4xx or 5xx");
        });
        name
    };

    let mapping = match &input.data {
        syn::Data::Enum(data) => {
            if data.variants.is_empty() {
                return Err(syn::Error::new_spanned(
                    ident,
                    "HTTP Error requires a nonempty error type",
                ));
            }
            if whole.is_none() {
                if options.wire(Mode::Serialize).is_some() || !options.enabled(Mode::Serialize) {
                    return Err(syn::Error::new_spanned(ident,
                        "variant statuses require derived serialization shapes; use a container #[http(status = ...)] for a whole-type schema override"));
                }
                let parsed = adapter::parse(&input, Mode::Serialize)?;
                let Data::Enum(variants) = parsed.data else {
                    unreachable!()
                };
                if variants.iter().any(|variant| variant.skipped) {
                    return Err(syn::Error::new_spanned(ident,
                        "variant statuses do not support skipped serialized variants; use a whole-error status or custom policy"));
                }
            }
            for variant in &data.variants {
                reject_field_statuses(&variant.fields)?;
                let expression = status(&variant.attrs)?;
                if whole.is_some() {
                    if expression.is_some() {
                        return Err(syn::Error::new_spanned(
                            variant,
                            "choose a whole-error status or per-variant statuses, not both",
                        ));
                    }
                    continue;
                }
                let expression = expression.ok_or_else(|| {
                    syn::Error::new_spanned(
                        &variant.ident,
                        "each error variant requires #[http(status = StatusCode::...)]",
                    )
                })?;
                let constant = add_status(expression);
                let name = &variant.ident;
                let pattern = match &variant.fields {
                    syn::Fields::Unit => quote!(Self::#name),
                    syn::Fields::Named(_) => quote!(Self::#name { .. }),
                    syn::Fields::Unnamed(_) => quote!(Self::#name (..)),
                };
                arms.push(quote!(#pattern => #constant));
                let source_name = name.to_string().trim_start_matches("r#").to_owned();
                rules.push(quote!((::std::string::String::from(#source_name), #constant.as_u16())));
            }
            if let Some(expression) = whole {
                let constant = add_status(expression);
                arms.push(quote!(_ => #constant));
                quote!(#runtime::http::contract::ErrorMapping::Status(#constant.as_u16()))
            } else {
                quote!(#runtime::http::contract::ErrorMapping::Variants(::std::vec![#(#rules),*]))
            }
        }
        syn::Data::Struct(data) => {
            reject_field_statuses(&data.fields)?;
            let expression = whole.ok_or_else(|| {
                syn::Error::new_spanned(
                    ident,
                    "an error struct requires a container #[http(status = StatusCode::...)]",
                )
            })?;
            let constant = add_status(expression);
            arms.push(quote!(_ => #constant));
            quote!(#runtime::http::contract::ErrorMapping::Status(#constant.as_u16()))
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "HTTP Error does not support unions",
            ))
        }
    };
    let mut generics = input.generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(Self: #runtime::Schema));
    let (implementation, types, bounds) = generics.split_for_impl();
    Ok(quote! {
        const _: () = {
            #(#constants)*
            #[automatically_derived]
            impl #implementation #runtime::http::Error for #ident #types #bounds {
                fn status(&self) -> #runtime::http::StatusCode {
                    match self { #(#arms,)* }
                }
                fn mapping() -> #runtime::http::contract::ErrorMapping {
                    #mapping
                }
            }
        };
    })
}

fn status(attributes: &[syn::Attribute]) -> syn::Result<Option<syn::Expr>> {
    let mut status = None;
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("http"))
    {
        attribute.parse_nested_meta(|meta| {
            if !meta.path.is_ident("status") {
                return Err(meta.error("HTTP Error accepts only status = StatusCode::..."));
            }
            let expression = meta.value()?.parse()?;
            if status.replace(expression).is_some() {
                return Err(meta.error("duplicate HTTP error status"));
            }
            Ok(())
        })?;
    }
    Ok(status)
}

fn reject_field_statuses(fields: &syn::Fields) -> syn::Result<()> {
    for field in fields {
        if let Some(attribute) = field
            .attrs
            .iter()
            .find(|attribute| attribute.path().is_ident("http"))
        {
            return Err(syn::Error::new_spanned(
                attribute,
                "HTTP error statuses belong on the error type or its variants, not fields",
            ));
        }
    }
    Ok(())
}
