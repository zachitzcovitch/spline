use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Verb {
    fn parse(path: &syn::Path) -> Option<Self> {
        if path.is_ident("get") {
            Some(Self::Get)
        } else if path.is_ident("post") {
            Some(Self::Post)
        } else if path.is_ident("put") {
            Some(Self::Put)
        } else if path.is_ident("patch") {
            Some(Self::Patch)
        } else if path.is_ident("delete") {
            Some(Self::Delete)
        } else {
            None
        }
    }
}

pub struct Binding {
    pub verb: Verb,
    pub path: syn::LitStr,
    pub body: Option<syn::LitStr>,
    pub status: u16,
}

pub fn take(attributes: &mut Vec<syn::Attribute>) -> syn::Result<Option<Binding>> {
    let mut binding = None;
    let mut retained = Vec::new();
    for attribute in attributes.drain(..) {
        if !attribute.path().is_ident("http") {
            retained.push(attribute);
            continue;
        }
        if binding.is_some() {
            return Err(syn::Error::new_spanned(attribute, "duplicate HTTP binding"));
        }
        let mut route = None;
        let mut body = None;
        let mut status = None;
        attribute.parse_nested_meta(|meta| {
            if let Some(verb) = Verb::parse(&meta.path) {
                let path: syn::LitStr = meta.value()?.parse()?;
                if route.replace((verb, path)).is_some() {
                    return Err(meta.error("expected one HTTP method/path"));
                }
            } else if meta.path.is_ident("body") {
                if body
                    .replace(meta.value()?.parse::<syn::LitStr>()?)
                    .is_some()
                {
                    return Err(meta.error("duplicate HTTP body mapping"));
                }
            } else if meta.path.is_ident("status") {
                let value: syn::LitInt = meta.value()?.parse()?;
                if status.replace(value.base10_parse::<u16>()?).is_some() {
                    return Err(meta.error("duplicate success status"));
                }
            } else {
                return Err(meta.error(
                    "supported HTTP options are get, post, put, patch, delete, body, and status",
                ));
            }
            Ok(())
        })?;
        let (verb, path) = route.ok_or_else(|| {
            syn::Error::new_spanned(
                &attribute,
                "HTTP binding requires get, post, put, patch, or delete",
            )
        })?;
        let status = status.unwrap_or(200);
        if !matches!(status, 200 | 201 | 204) {
            return Err(syn::Error::new_spanned(
                &attribute,
                "supported success statuses are 200, 201, and 204",
            ));
        }
        if verb == Verb::Get && body.is_some() {
            return Err(syn::Error::new_spanned(
                &attribute,
                "GET cannot have a JSON body mapping",
            ));
        }
        if verb == Verb::Delete && body.is_some() {
            return Err(syn::Error::new_spanned(
                &attribute,
                "DELETE cannot have a JSON body mapping in this profile",
            ));
        }
        if matches!(verb, Verb::Put | Verb::Patch) && body.is_none() {
            return Err(syn::Error::new_spanned(
                &attribute,
                "PUT and PATCH require a JSON body mapping",
            ));
        }
        binding = Some(Binding {
            verb,
            path,
            body,
            status,
        });
    }
    *attributes = retained;
    Ok(binding)
}

impl Binding {
    pub fn validate(&self, query: bool, fields: &[syn::Ident]) -> syn::Result<()> {
        if self.verb == Verb::Get && !query {
            return Err(syn::Error::new_spanned(
                &self.path,
                "mutations cannot be bound to GET",
            ));
        }
        if query && matches!(self.verb, Verb::Put | Verb::Patch | Verb::Delete) {
            return Err(syn::Error::new_spanned(
                &self.path,
                "PUT, PATCH, and DELETE must be mutations",
            ));
        }
        let names: Vec<_> = fields
            .iter()
            .map(|f| f.to_string().trim_start_matches("r#").to_owned())
            .collect();
        let mut mapped = std::collections::BTreeSet::new();
        let path = self.path.value();
        if !path.starts_with('/') {
            return Err(syn::Error::new_spanned(
                &self.path,
                "HTTP path must start with /",
            ));
        }
        for segment in path.split('/').skip(1) {
            if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                if !names.iter().any(|n| n == name) || !mapped.insert(name.to_owned()) {
                    return Err(syn::Error::new_spanned(
                        &self.path,
                        "path placeholder must name a distinct operation argument",
                    ));
                }
            }
        }
        if let Some(body) = &self.body {
            if !names.contains(&body.value()) || mapped.contains(&body.value()) {
                return Err(syn::Error::new_spanned(
                    body,
                    "body must name an argument not already in the path",
                ));
            }
        }
        Ok(())
    }

    pub fn metadata(&self, runtime: &syn::Path) -> TokenStream {
        let method = match self.verb {
            Verb::Get => quote!(#runtime::http::contract::HttpMethod::Get),
            Verb::Post => quote!(#runtime::http::contract::HttpMethod::Post),
            Verb::Put => quote!(#runtime::http::contract::HttpMethod::Put),
            Verb::Patch => quote!(#runtime::http::contract::HttpMethod::Patch),
            Verb::Delete => quote!(#runtime::http::contract::HttpMethod::Delete),
        };
        let path = &self.path;
        let body = match &self.body {
            Some(body) => quote!(::core::option::Option::Some(::std::string::String::from(#body))),
            None => quote!(::core::option::Option::None),
        };
        let status = self.status;
        quote!(#runtime::http::contract::HttpBinding {
            method: #method, path: ::std::string::String::from(#path), body: #body, success_status: #status,
        })
    }
}

pub struct Method {
    pub name: syn::Ident,
    pub wire_name: syn::LitStr,
    pub fields: Vec<syn::Ident>,
    pub types: Vec<syn::Ident>,
    pub output: syn::Ident,
    pub output_span: proc_macro2::Span,
    pub error: syn::Ident,
    pub binding: Binding,
}

pub fn generate(
    methods: &[Method],
    runtime: &syn::Path,
    trait_name: &syn::Ident,
    namespace: Option<&syn::LitStr>,
) -> syn::Result<TokenStream> {
    let mut client_methods = Vec::new();
    let mut verify = Vec::new();
    let mut routes = Vec::new();
    let mut policies = Vec::new();
    let mut bindings = Vec::new();
    let mut error_mappings = Vec::new();
    let mut no_content = Vec::new();
    let mut operation_routers = Vec::new();
    for method in methods {
        let name = &method.name;
        if [
            "new",
            "with_http",
            "with_response_policy",
            "__from_inner",
            "Client",
            "__Service",
            "__ErrorPolicy",
        ]
        .contains(&name.to_string().as_str())
        {
            return Err(syn::Error::new_spanned(
                name,
                "operation name conflicts with the HTTP client API",
            ));
        }
        let fields = &method.fields;
        let types = &method.types;
        let output = &method.output;
        let error = &method.error;
        let wire_name = &method.wire_name;
        let id = if let Some(namespace) = namespace {
            let id = format!("{}.{}", namespace.value(), wire_name.value());
            quote!(#id)
        } else {
            quote!(&::std::format!("{}.{}", super::__spline_namespace(), #wire_name))
        };
        let metadata = method.binding.metadata(runtime);
        bindings.push(quote! { ( <super::#name::Operation as #runtime::runtime::Operation>::id(), #metadata ) });
        error_mappings.push(quote! {
            (
                <super::#name::Operation as #runtime::runtime::Operation>::id(),
                <P as #runtime::http::ErrorPolicy<super::super::#error>>::mapping(errors),
            )
        });
        if method.binding.status == 204 {
            no_content.push(quote_spanned! {method.output_span=> #runtime::__private::assert_no_content::<super::super::#output>(); });
        }
        verify.push(quote! {
            #runtime::http::verify_serde::<super::#name::Input>()?;
            #runtime::http::verify_serde::<super::super::#output>()?;
            #runtime::http::verify_serde::<super::super::#error>()?;
        });
        client_methods.push(quote! {
            pub async fn #name(&self, #(#fields: super::super::#types),*)
                -> ::core::result::Result<super::super::#output, #runtime::http::CallError<super::super::#error>>
            {
                self.inner.call(#id, super::#name::Input { #(#fields),* }).await
            }
        });
        policies.push(quote!(#runtime::http::ErrorPolicy<super::super::#error>));
        let route_method = match method.binding.verb {
            Verb::Get => quote!(#runtime::http::axum::routing::get),
            Verb::Post => quote!(#runtime::http::axum::routing::post),
            Verb::Put => quote!(#runtime::http::axum::routing::put),
            Verb::Patch => quote!(#runtime::http::axum::routing::patch),
            Verb::Delete => quote!(#runtime::http::axum::routing::delete),
        };
        routes.push(quote! {
            let __spline_id = <super::#name::Operation as #runtime::runtime::Operation>::id();
            let __spline_plan = ::std::sync::Arc::new(
                __spline_description.plans()[&__spline_id].clone()
            );
            let __spline_path = __spline_plan.binding.path.clone();
            let __spline_route_factory = ::std::sync::Arc::clone(&__spline_factory);
            let __spline_route_errors = ::std::sync::Arc::clone(&__spline_errors);
            __spline_router = __spline_router.route(&__spline_path, #route_method(
                move |__spline_request: #runtime::http::axum::extract::Request| {
                    let factory = ::std::sync::Arc::clone(&__spline_route_factory);
                    let errors = ::std::sync::Arc::clone(&__spline_route_errors);
                    let plan = ::std::sync::Arc::clone(&__spline_plan);
                    async move {
                        let context = #runtime::http::RequestContext::from_request(&__spline_request);
                        let service = match factory(context).await {
                            Ok(service) => service,
                            Err(error) => return #runtime::http::axum::response::IntoResponse::into_response(error),
                        };
                        let __spline_input = match #runtime::http::server::decode::<super::#name::Input>(__spline_request, &plan).await {
                            Ok(input) => input,
                            Err(error) => return #runtime::http::axum::response::IntoResponse::into_response(error),
                        };
                        let result = #runtime::runtime::invoke(super::#name::Service::new(service), __spline_input).await;
                        #runtime::http::server::respond_call(result, plan.binding.success_status, &*errors)
                    }
                }
            ));
        });
        operation_routers.push(quote! {
            /// Bind a configured typed Tower service to this operation's HTTP route.
            pub mod #name {
                use super::__spline_runtime;
                pub fn router<S, P>(service: S, errors: P) -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
                where
                    S: #runtime::runtime::tower::Service<super::super::#name::Input,
                        Response = ::core::result::Result<super::super::super::#output, super::super::super::#error>>
                        + Clone + Send + Sync + 'static,
                    S::Future: Send,
                    S::Error: Into<#runtime::runtime::BoxError>,
                    P: #runtime::http::ErrorPolicy<super::super::super::#error>,
                {
                    let mut description = super::describe()?;
                    let id = <super::super::#name::Operation as #runtime::runtime::Operation>::id();
                    description.errors(&id, <P as #runtime::http::ErrorPolicy<super::super::super::#error>>::mapping(&errors))?;
                    let plan = ::std::sync::Arc::new(description.plans()[&id].clone());
                    let path = plan.binding.path.clone();
                    let service = ::std::sync::Arc::new(service);
                    let errors = ::std::sync::Arc::new(errors);
                    Ok(#runtime::http::axum::Router::new().route(&path, #route_method(
                        move |request: #runtime::http::axum::extract::Request| {
                            let service = (*service).clone();
                            let plan = ::std::sync::Arc::clone(&plan);
                            let errors = ::std::sync::Arc::clone(&errors);
                            async move {
                                let input = match #runtime::http::server::decode::<super::super::#name::Input>(request, &plan).await {
                                    Ok(input) => input,
                                    Err(error) => return #runtime::http::axum::response::IntoResponse::into_response(error),
                                };
                                let result = #runtime::runtime::invoke(service, input).await;
                                #runtime::http::server::respond_call(result, plan.binding.success_status, &*errors)
                            }
                        }
                    )))
                }
            }
        });
    }
    Ok(quote! {
        use super::__spline_runtime;

        impl<S, P> #runtime::http::HttpServiceBinding<S, P> for super::Definition
        where S: super::super::#trait_name + 'static, P: __ErrorPolicy {
            fn router_shared(service: ::std::sync::Arc<S>, errors: P)
                -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
            {
                router_with(move |_| {
                    let service = ::std::sync::Arc::clone(&service);
                    async move { Ok(service) }
                }, errors)
            }
        }
        impl<S, P> #runtime::http::HttpFactoryBinding<S, P> for super::Definition
        where S: super::super::#trait_name + 'static, P: __ErrorPolicy {
            fn router_with<F, Fut>(factory: F, errors: P)
                -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
            where
                F: Fn(#runtime::http::RequestContext) -> Fut + Send + Sync + 'static,
                Fut: ::core::future::Future<Output = ::core::result::Result<::std::sync::Arc<S>, #runtime::http::Fault>> + Send + 'static,
            {
                router_with(factory, errors)
            }
        }

        /// Select and validate this contract's HTTP binding.
        pub fn describe() -> ::core::result::Result<#runtime::http::contract::ContractDescription, #runtime::http::BuildError> {
            #(#no_content)*
            let mut builder = #runtime::http::contract::ContractBuilder::new(super::describe()?);
            for (id, binding) in __bindings() { builder.bind(&id, binding)?; }
            Ok(builder.finish()?)
        }
        /// Describe the binding with the selected policy's application-error responses.
        pub fn describe_with<P: __ErrorPolicy>(errors: &P)
            -> ::core::result::Result<#runtime::http::contract::ContractDescription, #runtime::http::BuildError>
        {
            let mut description = describe()?;
            for (id, mapping) in __error_mappings(errors) { description.errors(&id, mapping)?; }
            Ok(description)
        }
        #[doc(hidden)]
        pub fn __error_mappings<P: __ErrorPolicy>(errors: &P)
            -> ::std::vec::Vec<(::std::string::String, #runtime::http::contract::ErrorMapping)>
        {
            ::std::vec![#(#error_mappings),*]
        }
        #[doc(hidden)]
        pub fn __bindings() -> ::std::vec::Vec<(::std::string::String, #runtime::http::contract::HttpBinding)> {
            #(#no_content)*
            ::std::vec![#(#bindings),*]
        }

        #[derive(Clone, Debug)]
        pub struct Client { inner: #runtime::http::Client }

        impl Client {
            pub fn new(base_url: &str) -> ::core::result::Result<Self, #runtime::http::BuildError> {
                Self::__from_inner(#runtime::http::Client::new(base_url, &describe()?)?)
            }
            pub fn with_http(base_url: &str, client: #runtime::http::reqwest::Client)
                -> ::core::result::Result<Self, #runtime::http::BuildError>
            {
                Self::__from_inner(#runtime::http::Client::with_http(base_url, &describe()?, client)?)
            }
            #[doc(hidden)]
            pub fn __from_inner(inner: #runtime::http::Client) -> ::core::result::Result<Self, #runtime::http::BuildError> {
                #(#verify)*
                Ok(Self { inner })
            }
            pub fn with_response_policy(mut self, policy: #runtime::http::ResponsePolicy) -> Self {
                self.inner = self.inner.with_response_policy(policy);
                self
            }
            #(#client_methods)*
        }

        #[doc(hidden)]
        pub trait __ErrorPolicy: Send + Sync + 'static #(+ #policies)* {}
        impl<P: Send + Sync + 'static #(+ #policies)*> __ErrorPolicy for P {}

        pub fn router<S, P>(service: S, errors: P)
            -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
        where S: super::super::#trait_name + 'static, P: __ErrorPolicy {
            let service = ::std::sync::Arc::new(service);
            router_with(move |_| {
                let service = ::std::sync::Arc::clone(&service);
                async move { Ok(service) }
            }, errors)
        }

        pub fn router_with<S, P, F, Fut>(factory: F, errors: P)
            -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
        where
            S: super::super::#trait_name + 'static,
            P: __ErrorPolicy,
            F: ::core::ops::Fn(#runtime::http::RequestContext) -> Fut + ::core::marker::Send + ::core::marker::Sync + 'static,
            Fut: ::core::future::Future<Output = ::core::result::Result<::std::sync::Arc<S>, #runtime::http::Fault>> + ::core::marker::Send + 'static,
        {
            __router_with(::std::sync::Arc::new(factory), ::std::sync::Arc::new(errors))
        }

        #[doc(hidden)]
        pub fn __router_with<S, P, F, Fut>(
            __spline_factory: ::std::sync::Arc<F>, __spline_errors: ::std::sync::Arc<P>,
        ) -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
        where
            S: super::super::#trait_name + 'static,
            P: __ErrorPolicy,
            F: ::core::ops::Fn(#runtime::http::RequestContext) -> Fut + ::core::marker::Send + ::core::marker::Sync + 'static,
            Fut: ::core::future::Future<Output = ::core::result::Result<::std::sync::Arc<S>, #runtime::http::Fault>> + ::core::marker::Send + 'static,
        {
            let __spline_description = describe_with(&*__spline_errors)?;
            #[allow(unused_mut)]
            let mut __spline_router = #runtime::http::axum::Router::new();
            #(#routes)*
            Ok(__spline_router)
        }
        #(#operation_routers)*
    })
}
