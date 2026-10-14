use proc_macro2::TokenStream;
use quote::quote;

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
            Verb::Get => quote!(#runtime::contract::http::HttpMethod::Get),
            Verb::Post => quote!(#runtime::contract::http::HttpMethod::Post),
            Verb::Put => quote!(#runtime::contract::http::HttpMethod::Put),
            Verb::Patch => quote!(#runtime::contract::http::HttpMethod::Patch),
            Verb::Delete => quote!(#runtime::contract::http::HttpMethod::Delete),
        };
        let path = &self.path;
        let body = match &self.body {
            Some(body) => quote!(::core::option::Option::Some(::std::string::String::from(#body))),
            None => quote!(::core::option::Option::None),
        };
        let status = self.status;
        quote!(#runtime::contract::http::HttpBinding {
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
            quote!(&::std::format!("{}.{}", self::__spline_namespace(), #wire_name))
        };
        verify.push(quote! {
            #runtime::http::verify_serde::<#name::Input>()?;
            #runtime::http::verify_serde::<super::#output>()?;
            #runtime::http::verify_serde::<super::#error>()?;
        });
        client_methods.push(quote! {
            pub async fn #name(&self, #(#fields: super::#types),*)
                -> ::core::result::Result<super::#output, #runtime::http::CallError<super::#error>>
            {
                self.inner.call(#id, #name::Input { #(#fields),* }).await
            }
        });
        policies.push(quote!(#runtime::http::ErrorPolicy<super::#error>));
        let route_method = match method.binding.verb {
            Verb::Get => quote!(#runtime::http::axum::routing::get),
            Verb::Post => quote!(#runtime::http::axum::routing::post),
            Verb::Put => quote!(#runtime::http::axum::routing::put),
            Verb::Patch => quote!(#runtime::http::axum::routing::patch),
            Verb::Delete => quote!(#runtime::http::axum::routing::delete),
        };
        routes.push(quote! {
            let __spline_plan = ::std::sync::Arc::new(
                __spline_description.operations()[#wire_name].http.clone().expect("macro declared HTTP binding")
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
                        let __spline_input = match #runtime::http::server::decode::<#name::Input>(__spline_request, &plan).await {
                            Ok(input) => input,
                            Err(error) => return #runtime::http::axum::response::IntoResponse::into_response(error),
                        };
                        let result = super::#trait_name::#name(&*service, #(__spline_input.#fields),*).await;
                        #runtime::http::server::respond(result, plan.binding.success_status, &*errors)
                    }
                }
            ));
        });
    }
    Ok(quote! {
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

        // Generated composition bounds; implementations still target the authored
        // resource trait and the ordinary ErrorPolicy implementations.
        #[doc(hidden)]
        #[allow(private_bounds)]
        pub trait __Service: super::#trait_name {}
        impl<S: super::#trait_name> __Service for S {}
        #[doc(hidden)]
        pub trait __ErrorPolicy: #(#policies)+* {}
        impl<P: #(#policies)+*> __ErrorPolicy for P {}

        pub fn router<S, P>(service: S, errors: P)
            -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
        where S: super::#trait_name + 'static, P: #(#policies)+* {
            let service = ::std::sync::Arc::new(service);
            router_with(move |_| {
                let service = ::std::sync::Arc::clone(&service);
                async move { Ok(service) }
            }, errors)
        }

        pub fn router_with<S, P, F, Fut>(factory: F, errors: P)
            -> ::core::result::Result<#runtime::http::axum::Router, #runtime::http::BuildError>
        where
            S: super::#trait_name + 'static,
            P: #(#policies)+*,
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
            S: super::#trait_name + 'static,
            P: #(#policies)+*,
            F: ::core::ops::Fn(#runtime::http::RequestContext) -> Fut + ::core::marker::Send + ::core::marker::Sync + 'static,
            Fut: ::core::future::Future<Output = ::core::result::Result<::std::sync::Arc<S>, #runtime::http::Fault>> + ::core::marker::Send + 'static,
        {
            let __spline_description = describe()?;
            let mut __spline_router = #runtime::http::axum::Router::new();
            #(#routes)*
            Ok(__spline_router)
        }
    })
}
