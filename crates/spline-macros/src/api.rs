use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{braced, Token};

struct Api {
    runtime: syn::Path,
    docs: Vec<syn::Attribute>,
    visibility: syn::Visibility,
    module: syn::Ident,
    resources: Vec<(syn::Ident, syn::Path)>,
}

impl Parse for Api {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let runtime = if input.peek(Token![crate]) {
            input.parse::<Token![crate]>()?;
            input.parse::<Token![=]>()?;
            let runtime = input.parse::<syn::LitStr>()?.parse()?;
            input.parse::<Token![;]>()?;
            runtime
        } else {
            syn::parse_quote!(::spline)
        };
        let docs = input.call(syn::Attribute::parse_outer)?;
        for attr in &docs {
            if !attr.path().is_ident("doc") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "API declarations accept documentation attributes only",
                ));
            }
        }
        let visibility = input.parse()?;
        input.parse::<Token![mod]>()?;
        let module = input.parse()?;
        let body;
        braced!(body in input);
        let mut resources = Vec::new();
        let mut names = BTreeSet::new();
        while !body.is_empty() {
            let group: syn::Ident = body.parse()?;
            let name = group.to_string().trim_start_matches("r#").to_owned();
            if !names.insert(name.clone()) {
                return Err(syn::Error::new_spanned(
                    group,
                    "duplicate API resource group",
                ));
            }
            if [
                "new",
                "with_http",
                "with_response_policy",
                "__from_inner",
                "clone",
            ]
            .contains(&name.as_str())
            {
                return Err(syn::Error::new_spanned(
                    group,
                    "resource group conflicts with the application client API",
                ));
            }
            if !name.is_ascii() {
                return Err(syn::Error::new_spanned(
                    group,
                    "resource groups must be ASCII identifiers",
                ));
            }
            body.parse::<Token![:]>()?;
            resources.push((group, body.parse()?));
            if !body.is_empty() {
                body.parse::<Token![,]>()?;
            }
        }
        if resources.is_empty() {
            return Err(syn::Error::new_spanned(
                &module,
                "API requires at least one resource",
            ));
        }
        Ok(Self {
            runtime,
            docs,
            visibility,
            module,
            resources,
        })
    }
}

pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let Api {
        runtime,
        docs,
        visibility,
        module,
        resources,
    } = syn::parse2(input)?;
    let documentation = crate::attrs::docs(&docs)
        .iter()
        .map(syn::LitStr::value)
        .collect::<Vec<_>>()
        .join("\n");
    // Resolve module paths where they were authored, before entering the generated
    // companion module. This preserves relative paths and renamed dependencies.
    let runtime_alias = format_ident!("__Spline_{}_Runtime", module);
    let aliases: Vec<_> = (0..resources.len())
        .map(|i| format_ident!("__Spline_{}_Resource{}", module, i))
        .collect();
    let local: Vec<_> = (0..resources.len())
        .map(|i| format_ident!("__resource{}", i))
        .collect();
    let groups: Vec<_> = resources.iter().map(|(group, _)| group).collect();
    let names: Vec<_> = groups
        .iter()
        .map(|group| group.to_string().trim_start_matches("r#").to_owned())
        .collect();
    let paths: Vec<_> = resources.iter().map(|(_, path)| path).collect();
    let service_bounds = quote!(#(#local::__Service)+*);
    let error_bounds = quote!(#(#local::__ErrorPolicy)+*);
    Ok(quote! {
        #[doc(hidden)]
        use #runtime as #runtime_alias;
        #(#[doc(hidden)] use #paths as #aliases;)*
        #(#docs)*
        #visibility mod #module {
            use super::#runtime_alias as __runtime;
            #(use super::#aliases as #local;)*

            pub fn describe() -> __runtime::contract::Result<__runtime::contract::ApplicationDescription> {
                let mut application = __runtime::contract::ApplicationBuilder::new(#documentation);
                #(#local::register(&mut application, #names)?;)*
                application.finish()
            }

            #[derive(Clone, Debug)]
            pub struct Client { #(#groups: #local::Client,)* }

            impl Client {
                pub fn new(base_url: &str) -> ::core::result::Result<Self, __runtime::http::BuildError> {
                    Self::__from_inner(__runtime::http::Client::for_application(base_url, &describe()?)?)
                }
                pub fn with_http(base_url: &str, http: __runtime::http::reqwest::Client)
                    -> ::core::result::Result<Self, __runtime::http::BuildError>
                {
                    Self::__from_inner(__runtime::http::Client::for_application_with_http(base_url, &describe()?, http)?)
                }
                fn __from_inner(inner: __runtime::http::Client) -> ::core::result::Result<Self, __runtime::http::BuildError> {
                    Ok(Self { #(#groups: #local::Client::__from_inner(inner.clone())?,)* })
                }
                pub fn with_response_policy(self, policy: __runtime::http::ResponsePolicy) -> Self {
                    Self { #(#groups: self.#groups.with_response_policy(policy),)* }
                }
                #(pub fn #groups(&self) -> &#local::Client { &self.#groups })*
            }

            pub fn router<S, P>(service: S, errors: P)
                -> ::core::result::Result<__runtime::http::axum::Router, __runtime::http::BuildError>
            where S: #service_bounds + 'static, P: #error_bounds {
                let service = ::std::sync::Arc::new(service);
                router_with(move |_| {
                    let service = ::std::sync::Arc::clone(&service);
                    async move { Ok(service) }
                }, errors)
            }

            pub fn router_with<S, P, F, Fut>(factory: F, errors: P)
                -> ::core::result::Result<__runtime::http::axum::Router, __runtime::http::BuildError>
            where
                S: #service_bounds + 'static,
                P: #error_bounds,
                F: ::core::ops::Fn(__runtime::http::RequestContext) -> Fut + ::core::marker::Send + ::core::marker::Sync + 'static,
                Fut: ::core::future::Future<Output = ::core::result::Result<::std::sync::Arc<S>, __runtime::http::Fault>> + ::core::marker::Send + 'static,
            {
                // Validate the entire application before Axum merges any routes.
                describe()?;
                let factory = ::std::sync::Arc::new(factory);
                let errors = ::std::sync::Arc::new(errors);
                let mut router = __runtime::http::axum::Router::new();
                #(router = router.merge(#local::__router_with(
                    ::std::sync::Arc::clone(&factory), ::std::sync::Arc::clone(&errors),
                )?);)*
                Ok(router)
            }
        }
    })
}
