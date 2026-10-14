use proc_macro2::TokenStream;
use quote::quote;

pub struct Method {
    pub name: syn::Ident,
    pub wire_name: syn::LitStr,
    pub fields: Vec<syn::Ident>,
    pub types: Vec<syn::Ident>,
    pub output: syn::Ident,
    pub error: syn::Ident,
    pub query: bool,
}

pub fn operation(method: &Method, runtime: &syn::Path, trait_name: &syn::Ident) -> TokenStream {
    let Method {
        name,
        wire_name,
        fields,
        output,
        error,
        query,
        ..
    } = method;
    let kind = if *query {
        quote!(#runtime::contract::OperationKind::Query)
    } else {
        quote!(#runtime::contract::OperationKind::Mutation)
    };
    quote! {
        /// Typed execution signature for this operation.
        pub struct Operation;
        impl #runtime::runtime::Operation for Operation {
            type Input = Input;
            type Output = super::super::#output;
            type Error = super::super::#error;
            fn id() -> ::std::string::String { ::std::format!("{}.{}", super::__spline_namespace(), #wire_name) }
            fn kind() -> #runtime::contract::OperationKind { #kind }
        }

        /// A typed Tower adapter over an authored implementation. The base
        /// adapter is always ready; outer Tower layers can provide backpressure.
        pub struct Service<S> { inner: ::std::sync::Arc<S> }
        impl<S> Clone for Service<S> {
            fn clone(&self) -> Self { Self { inner: ::std::sync::Arc::clone(&self.inner) } }
        }
        impl<S> Service<S> {
            pub fn new(inner: ::std::sync::Arc<S>) -> Self { Self { inner } }
        }
        impl<S: super::super::#trait_name + 'static> #runtime::runtime::tower::Service<Input> for Service<S> {
            type Response = ::core::result::Result<super::super::#output, super::super::#error>;
            type Error = ::core::convert::Infallible;
            type Future = #runtime::runtime::BoxFuture<::core::result::Result<Self::Response, Self::Error>>;
            fn poll_ready(&mut self, _: &mut ::core::task::Context<'_>) -> ::core::task::Poll<::core::result::Result<(), Self::Error>> {
                ::core::task::Poll::Ready(Ok(()))
            }
            fn call(&mut self, input: Input) -> Self::Future {
                let service = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move { Ok(super::super::#trait_name::#name(&*service, #(input.#fields),*).await) })
            }
        }
    }
}

pub fn resource(methods: &[Method], runtime: &syn::Path, trait_name: &syn::Ident) -> TokenStream {
    let calls = methods.iter().map(|method| {
        let Method { name, fields, types, output, error, .. } = method;
        quote! {
            pub async fn #name(&self, #(#fields: super::super::#types),*)
                -> ::core::result::Result<super::super::#output, #runtime::runtime::CallError<super::super::#error>>
            {
                #runtime::runtime::invoke(super::#name::Service::new(::std::sync::Arc::clone(&self.inner)), super::#name::Input { #(#fields),* }).await
            }
        }
    });
    quote! {
        /// A protocol-neutral definition value for this resource.
        pub struct Definition;
        impl #runtime::runtime::Definition for Definition {
            type Description = #runtime::contract::ContractDescription;
            fn describe() -> #runtime::contract::Result<Self::Description> { describe() }
        }
        /// Select an adapter from the contract without supplying an implementation.
        pub fn contract() -> Definition { Definition }
        pub type Services<S> = #runtime::runtime::Services<Definition, S>;
        /// Share this implementation across calls to the selected adapter.
        pub fn shared<S: super::#trait_name + 'static>(implementation: S) -> Services<S> {
            #runtime::runtime::Services::new(implementation)
        }
        impl<S: super::#trait_name + 'static> #runtime::runtime::InProcessBinding<S> for Definition {
            type Client = in_process::Client<S>;
            fn client_shared(implementation: ::std::sync::Arc<S>) -> #runtime::contract::Result<Self::Client> {
                describe()?;
                Ok(in_process::Client::__from_shared(implementation))
            }
        }

        #[doc(hidden)] #[allow(private_bounds)]
        pub trait __Service: super::#trait_name {}
        impl<S: super::#trait_name> __Service for S {}

        /// Direct, typed calls through the same Tower adapters used by bindings.
        pub mod in_process {
            use super::__spline_runtime;
            pub struct Client<S> { inner: ::std::sync::Arc<S> }
            impl<S> Clone for Client<S> {
                fn clone(&self) -> Self { Self { inner: ::std::sync::Arc::clone(&self.inner) } }
            }
            impl<S: super::super::#trait_name + 'static> Client<S> {
                pub fn new(service: S) -> #runtime::contract::Result<Self> {
                    super::describe()?;
                    Ok(Self::__from_shared(::std::sync::Arc::new(service)))
                }
                #[doc(hidden)]
                pub fn __from_shared(inner: ::std::sync::Arc<S>) -> Self { Self { inner } }
                #(#calls)*
            }
        }
    }
}
