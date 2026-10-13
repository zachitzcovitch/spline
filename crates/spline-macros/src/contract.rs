use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::attrs;

pub fn expand(attributes: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let mut namespace = None;
    let mut module = None;
    let mut runtime = None;
    syn::meta::parser(|meta| {
        let value: syn::LitStr = meta.value()?.parse()?;
        if meta.path.is_ident("namespace") {
            if namespace.replace(value.clone()).is_some() {
                return Err(meta.error("duplicate contract namespace"));
            }
        } else if meta.path.is_ident("module") {
            if module.replace(value.parse::<syn::Ident>()?).is_some() {
                return Err(meta.error("duplicate contract module"));
            }
        } else if meta.path.is_ident("crate") {
            if runtime.replace(value.parse::<syn::Path>()?).is_some() {
                return Err(meta.error("duplicate contract crate path"));
            }
        } else {
            return Err(
                meta.error("unsupported contract option; expected namespace, module, or crate")
            );
        }
        Ok(())
    })
    .parse2(attributes)?;
    let mut item: syn::ItemTrait = syn::parse2(input)?;
    let namespace = namespace.ok_or_else(|| {
        syn::Error::new_spanned(&item.ident, "contract requires namespace = \"resource\"")
    })?;
    if !namespace.value().split('.').all(identifier) {
        return Err(syn::Error::new_spanned(
            namespace,
            "namespace must be dot-separated ASCII identifiers",
        ));
    }
    if !item.generics.params.is_empty()
        || item.generics.where_clause.is_some()
        || !item.supertraits.is_empty()
        || item.unsafety.is_some()
        || item.modifiers.auto_token.is_some()
    {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "this contract slice requires a safe, non-generic trait without supertraits",
        ));
    }
    check_docs_only(&item.attrs)?;
    let source_runtime = runtime.unwrap_or_else(|| syn::parse_quote!(::spline));
    let trait_name = &item.ident;
    let runtime_alias = format_ident!("__Spline_{}_Runtime", trait_name);
    let runtime: syn::Path = syn::parse_quote!(__spline_runtime);
    let module =
        module.unwrap_or_else(|| format_ident!("{}_contract", snake_case(&unraw(trait_name))));
    let visibility = &item.vis;
    let serde_path = quote!(#runtime::__private::serde).to_string();
    let runtime_path = runtime.to_token_stream().to_string();
    let contract_docs = documentation(&item.attrs);
    let mut wire_names = BTreeSet::new();
    let mut aliases = Vec::new();
    let mut requests = Vec::new();
    let mut registrations = Vec::new();
    let mut assertions = Vec::new();
    let mut http_methods = Vec::new();

    for trait_item in &mut item.items {
        let syn::TraitItem::Fn(method) = trait_item else {
            return Err(syn::Error::new_spanned(
                trait_item,
                "contracts contain operation methods only",
            ));
        };
        let method_name = method.sig.ident.clone();
        let rust_name = unraw(&method_name);
        let http = crate::http::take(&mut method.attrs)?;
        let (kind, name) = operation_attributes(&mut method.attrs, &rust_name, method_name.span())?;
        if !wire_names.insert(name.value()) {
            return Err(syn::Error::new_spanned(
                &name,
                "duplicate contract operation name",
            ));
        }
        let sig = &method.sig;
        if sig.asyncness.is_none()
            || sig.constness.is_some()
            || !matches!(sig.safety, syn::Safety::Default)
            || sig.abi.is_some()
            || sig.variadic.is_some()
            || !sig.generics.params.is_empty()
            || sig.generics.where_clause.is_some()
            || method.default.is_some()
        {
            return Err(syn::Error::new_spanned(
                sig,
                "operations must be async, safe, non-generic method declarations without a body",
            ));
        }
        let Some(syn::FnArg::Receiver(receiver)) = sig.inputs.first() else {
            return Err(syn::Error::new_spanned(
                sig,
                "operations require an &self receiver",
            ));
        };
        if receiver.mutability.is_some()
            || !matches!(receiver.kind, syn::ReceiverKind::Reference(_, None, None))
        {
            return Err(syn::Error::new_spanned(
                receiver,
                "operations require an unannotated &self receiver",
            ));
        }
        check_docs_only(&receiver.attrs)?;
        let (output, error) = result_types(&sig.output)?;
        owned_type(&output)?;
        owned_type(&error)?;
        let output_alias = format_ident!("__Spline_{}_{}_Output", trait_name, method_name);
        let error_alias = format_ident!("__Spline_{}_{}_Error", trait_name, method_name);
        aliases.push(quote! {
            #[doc(hidden)] #[allow(non_camel_case_types)] type #output_alias = #output;
            #[doc(hidden)] #[allow(non_camel_case_types)] type #error_alias = #error;
        });
        let mut fields = Vec::new();
        let mut field_names = Vec::new();
        let mut field_types = Vec::new();
        let mut arguments = BTreeSet::new();
        for (index, argument) in sig.inputs.iter().skip(1).enumerate() {
            let syn::FnArg::Typed(argument) = argument else {
                return Err(syn::Error::new_spanned(argument, "unexpected receiver"));
            };
            check_docs_only(&argument.attrs)?;
            let syn::Pat::Ident(pattern) = argument.pat.as_ref() else {
                return Err(syn::Error::new_spanned(
                    &argument.pat,
                    "operation arguments must be named identifiers",
                ));
            };
            if pattern.by_ref.is_some() || pattern.mutability.is_some() || pattern.subpat.is_some()
            {
                return Err(syn::Error::new_spanned(
                    pattern,
                    "operation arguments must be plain named identifiers",
                ));
            }
            if !arguments.insert(unraw(&pattern.ident)) {
                return Err(syn::Error::new_spanned(
                    pattern,
                    "duplicate operation argument",
                ));
            }
            owned_type(&argument.ty)?;
            let alias = format_ident!("__Spline_{}_{}_Arg{}", trait_name, method_name, index);
            let ty = &argument.ty;
            aliases.push(quote! {
                #[doc(hidden)] #[allow(non_camel_case_types)] type #alias = #ty;
            });
            let field = &pattern.ident;
            let docs = &argument.attrs;
            // Resolve original argument syntax through an alias in its original
            // module, so generated Input names and relative paths cannot capture it.
            fields.push(quote! { #(#docs)* pub #field: super::super::#alias });
            field_names.push(field.clone());
            field_types.push(alias);
        }
        if let Some(binding) = &http {
            binding.validate(kind, &field_names)?;
        }
        let input_derives = if http.is_some() {
            quote!(#runtime::Schema, #runtime::__private::serde::Deserialize, #runtime::__private::serde::Serialize)
        } else {
            quote!(#runtime::Schema, #runtime::__private::serde::Deserialize)
        };
        let only_input = if http.is_some() {
            quote!()
        } else {
            quote!(only = "deserialize",)
        };
        let input_name = format!("{}.{}.Args", namespace.value(), name.value());
        requests.push(quote! {
            #[allow(non_snake_case)]
            pub mod #method_name {
                use super::__spline_runtime;
                #[derive(#input_derives)]
                #[spline(name = #input_name, #only_input crate = #runtime_path)]
                #[serde(crate = #serde_path, deny_unknown_fields)]
                pub struct Input { #(#fields,)* }
            }
        });
        assertions.push(quote_spanned! {method_name.span()=>
            #runtime::__private::assert_input::<#method_name::Input>();
            #runtime::__private::assert_output::<super::#output_alias>();
            #runtime::__private::assert_output::<super::#error_alias>();
        });
        let docs = documentation(&method.attrs);
        let kind = if kind {
            quote!(#runtime::contract::OperationKind::Query)
        } else {
            quote!(#runtime::contract::OperationKind::Mutation)
        };
        registrations.push(quote! {
            __spline_builder.operation::<#method_name::Input, super::#output_alias, super::#error_alias>(
                #name, #rust_name, #kind, #docs,
            )?;
        });
        if let Some(binding) = http {
            if binding.status == 204 {
                assertions.push(quote_spanned! {output.span()=>
                    #runtime::__private::assert_no_content::<super::#output_alias>();
                });
            }
            let metadata = binding.metadata(&runtime);
            registrations.push(quote! { __spline_builder.http(#name, #metadata)?; });
            http_methods.push(crate::http::Method {
                name: method_name.clone(),
                wire_name: name,
                fields: field_names,
                types: field_types,
                output: output_alias,
                error: error_alias,
                binding,
            });
        }
        method.sig.asyncness = None;
        method.sig.output = syn::parse_quote!(-> impl ::core::future::Future<
            Output = ::core::result::Result<#output, #error>
        > + ::core::marker::Send);
    }
    if requests.is_empty() {
        return Err(syn::Error::new_spanned(
            trait_name,
            "contract requires at least one operation",
        ));
    }
    item.supertraits
        .push(syn::parse_quote!(::core::marker::Send));
    item.supertraits
        .push(syn::parse_quote!(::core::marker::Sync));
    item.colon_token = Some(Default::default());
    let http_code = if http_methods.is_empty() {
        quote!()
    } else {
        if http_methods.len() != requests.len() {
            return Err(syn::Error::new_spanned(
                trait_name,
                "HTTP contracts require a binding on every operation",
            ));
        }
        crate::http::generate(&http_methods, &runtime, trait_name, &namespace)?
    };
    Ok(quote! {
        #item
        #[doc(hidden)]
        use #source_runtime as #runtime_alias;
        #(#aliases)*
        #visibility mod #module {
            use super::#runtime_alias as __spline_runtime;
            #(#requests)*
            /// Describe the contract and collect its directional schemas.
            pub fn describe() -> #runtime::contract::Result<#runtime::contract::ContractDescription> {
                let mut __spline_builder = #runtime::contract::ContractBuilder::new(#namespace, #contract_docs)?;
                __register_operations(&mut __spline_builder)?;
                __spline_builder.finish()
            }
            /// Register this resource into an application's shared schema registry.
            pub fn register(application: &mut #runtime::contract::ApplicationBuilder, group: &str)
                -> #runtime::contract::Result<()>
            {
                application.resource(group, #namespace, #contract_docs, __register_operations)
            }
            fn __register_operations(__spline_builder: &mut #runtime::contract::ContractBuilder)
                -> #runtime::contract::Result<()>
            {
                #(#assertions)*
                #(#registrations)*
                Ok(())
            }
            #http_code
        }
    })
}

fn operation_attributes(
    attributes: &mut Vec<syn::Attribute>,
    rust_name: &str,
    method_span: proc_macro2::Span,
) -> syn::Result<(bool, syn::LitStr)> {
    let mut operation = None;
    let mut docs = Vec::new();
    for attribute in attributes.drain(..) {
        if attribute.path().is_ident("doc") {
            docs.push(attribute);
            continue;
        }
        let query = attribute.path().is_ident("query");
        if !query && !attribute.path().is_ident("mutation") {
            return Err(syn::Error::new_spanned(attribute,
                "unsupported operation attribute; this metadata slice accepts doc, query, and mutation"));
        }
        if operation.is_some() {
            return Err(syn::Error::new_spanned(
                attribute,
                "operation needs exactly one query or mutation annotation",
            ));
        }
        let mut name = syn::LitStr::new(rust_name, attribute.span());
        if !matches!(attribute.meta, syn::Meta::Path(_)) {
            let mut has_name = false;
            attribute.parse_nested_meta(|meta| {
                if !meta.path.is_ident("name") || has_name {
                    return Err(meta.error("expected one optional name = \"operation\""));
                }
                name = meta.value()?.parse()?;
                has_name = true;
                Ok(())
            })?;
        }
        if !identifier(&name.value()) {
            return Err(syn::Error::new_spanned(
                name,
                "operation name must be an ASCII identifier",
            ));
        }
        operation = Some((query, name));
    }
    *attributes = docs;
    operation.ok_or_else(|| {
        syn::Error::new(
            method_span,
            "each operation requires #[query] or #[mutation]",
        )
    })
}

fn result_types(output: &syn::ReturnType) -> syn::Result<(syn::Type, syn::Type)> {
    let fail = || {
        syn::Error::new_spanned(
            output,
            "operation return type must be Result<Output, Error>",
        )
    };
    let syn::ReturnType::Type(_, ty) = output else {
        return Err(fail());
    };
    let syn::Type::Path(path) = ty.as_ref() else {
        return Err(fail());
    };
    let names: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    if path.qself.is_some()
        || !(names == ["Result"]
            || names == ["std", "result", "Result"]
            || names == ["core", "result", "Result"])
    {
        return Err(fail());
    }
    let syn::PathArguments::AngleBracketed(arguments) =
        &path.path.segments.last().unwrap().arguments
    else {
        return Err(fail());
    };
    let types: Vec<_> = arguments.args.iter().collect();
    match types.as_slice() {
        [syn::GenericArgument::Type(output), syn::GenericArgument::Type(error)] => {
            Ok((output.clone(), error.clone()))
        }
        _ => Err(fail()),
    }
}

fn owned_type(ty: &syn::Type) -> syn::Result<()> {
    struct Check(Option<syn::Error>);
    impl<'ast> Visit<'ast> for Check {
        fn visit_type(&mut self, ty: &'ast syn::Type) {
            let invalid = matches!(
                ty,
                syn::Type::Reference(_)
                    | syn::Type::ImplTrait(_)
                    | syn::Type::TraitObject(_)
                    | syn::Type::Infer(_)
                    | syn::Type::Macro(_)
            );
            if invalid {
                self.0 = Some(syn::Error::new_spanned(
                    ty,
                    "operation wire types must be concrete owned types",
                ));
            } else {
                visit::visit_type(self, ty);
            }
        }
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if path
                .path
                .segments
                .iter()
                .any(|segment| segment.ident == "Self")
            {
                self.0 = Some(syn::Error::new_spanned(
                    path,
                    "Self-dependent operation types are unsupported",
                ));
            }
            visit::visit_type_path(self, path);
        }
    }
    let mut check = Check(None);
    check.visit_type(ty);
    match check.0 {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn check_docs_only(attributes: &[syn::Attribute]) -> syn::Result<()> {
    for attribute in attributes {
        if !attribute.path().is_ident("doc") {
            return Err(syn::Error::new_spanned(
                attribute,
                "only documentation attributes are supported at this contract location",
            ));
        }
    }
    Ok(())
}

fn documentation(attributes: &[syn::Attribute]) -> String {
    attrs::docs(attributes)
        .iter()
        .map(syn::LitStr::value)
        .collect::<Vec<_>>()
        .join("\n")
}

fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn unraw(ident: &syn::Ident) -> String {
    ident.to_string().trim_start_matches("r#").into()
}

fn snake_case(value: &str) -> String {
    let chars: Vec<_> = value.chars().collect();
    let mut output = String::new();
    for (index, character) in chars.iter().copied().enumerate() {
        if character.is_ascii_uppercase()
            && index > 0
            && (chars[index - 1].is_ascii_lowercase()
                || chars[index - 1].is_ascii_digit()
                || chars.get(index + 1).is_some_and(char::is_ascii_lowercase))
        {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    output
}
