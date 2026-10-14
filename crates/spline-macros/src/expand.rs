use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::model::*;
use crate::{adapter, attrs};

pub fn expand(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let options = attrs::parse(&input.attrs, true)?;
    validate_locations(&input)?;
    for parameter in &input.generics.params {
        if !matches!(parameter, syn::GenericParam::Type(_)) {
            return Err(syn::Error::new_spanned(parameter,
                "Schema currently supports owned DTOs with type parameters; lifetime and const parameters need a manual implementation"));
        }
    }
    let runtime = options
        .runtime
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::spline));
    let mut bodies = Vec::new();
    let mut variant_names = Vec::new();
    let mut used_types = Vec::new();
    let mut missing = quote!(#runtime::MissingField::Rejected);
    for mode in [Mode::Deserialize, Mode::Serialize] {
        let direction = mode_token(mode, &runtime);
        if !options.enabled(mode) {
            let message = format!("{} Schema is disabled for this type", mode.label());
            bodies.push(quote! {
                #direction => ::core::result::Result::Err(
                    #runtime::Error::Unsupported(::std::string::String::from(#message))
                )
            });
            continue;
        }
        let container = adapter::parse(&input, mode)?;
        if options.wire(mode).is_none() {
            if let (syn::Data::Enum(source), Data::Enum(variants)) = (&input.data, &container.data)
            {
                let names: Vec<_> = source
                    .variants
                    .iter()
                    .zip(variants)
                    .filter(|(_, variant)| !variant.skipped)
                    .map(|(source, _)| source.ident.to_string().trim_start_matches("r#").to_owned())
                    .collect();
                variant_names
                    .push(quote!(#direction => ::core::option::Option::Some(&[#(#names),*])));
            }
        }
        if mode == Mode::Deserialize {
            missing = missing_expression(&container, &options, &runtime)?;
        }
        let mut emitter = Emitter {
            runtime: runtime.clone(),
            mode,
            used_types: Vec::new(),
        };
        let shape = if let Some(wire) = options.wire(mode) {
            emitter.reference(wire)
        } else {
            check_container_attributes(&input.attrs, mode)?;
            emitter.container(&container, input.ident.span())?
        };
        used_types.extend(emitter.used_types);
        bodies.push(quote!(#direction => ::core::result::Result::Ok(#shape)));
    }

    let relevant = relevant_parameters(&input.generics, &used_types);
    let mut generics = input.generics.clone();
    let mut arguments = Vec::new();
    for parameter in input.generics.type_params() {
        let ident = &parameter.ident;
        let predicate = if relevant.contains(&ident.to_string()) {
            arguments.push(quote!(<#ident as #runtime::Schema>::info().name));
            syn::parse_quote!(#ident: #runtime::Schema)
        } else {
            syn::parse_quote!(#ident: 'static)
        };
        generics.make_where_clause().predicates.push(predicate);
    }
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let ident = &input.ident;
    let type_name = ident.to_string().trim_start_matches("r#").to_owned();
    let path = match &options.name {
        Some(name) => quote!(#name),
        None => quote!(::std::format!(
            "{}.{}", ::core::module_path!().replace("::", "."), #type_name
        )),
    };
    let documentation = attrs::docs(&input.attrs)
        .iter()
        .map(syn::LitStr::value)
        .collect::<Vec<_>>()
        .join("\n");
    let unknown_variants =
        (variant_names.len() < 2).then(|| quote!(_ => ::core::option::Option::None,));

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #runtime::Schema for #ident #type_generics #where_clause {
            fn info() -> #runtime::TypeInfo {
                let mut __spline_info = #runtime::TypeInfo::named(
                    #runtime::Name::applied(#path, ::std::vec![#(#arguments),*])
                ).documented(#documentation);
                __spline_info.missing = <Self as #runtime::Schema>::missing_field();
                __spline_info
            }

            fn describe(__spline_cx: &mut #runtime::Context<'_>) -> #runtime::Result<#runtime::Shape> {
                match __spline_cx.direction() {
                    #(#bodies),*
                }
            }

            fn missing_field() -> #runtime::MissingField {
                #missing
            }

            fn variants(direction: #runtime::Direction) -> ::core::option::Option<&'static [&'static str]> {
                match direction {
                    #(#variant_names,)*
                    #unknown_variants
                }
            }
        }
    })
}

fn validate_locations(input: &syn::DeriveInput) -> syn::Result<()> {
    let fields: Vec<&syn::Field> = match &input.data {
        syn::Data::Struct(data) => data.fields.iter().collect(),
        syn::Data::Enum(data) => {
            for variant in &data.variants {
                if let Some(attribute) = variant.attrs.iter().find(|a| a.path().is_ident("spline"))
                {
                    return Err(syn::Error::new_spanned(
                        attribute,
                        "spline options belong on the container or its fields, not variants",
                    ));
                }
            }
            data.variants.iter().flat_map(|v| &v.fields).collect()
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "Schema does not support unions",
            ))
        }
    };
    for field in fields {
        attrs::parse(&field.attrs, false)?;
    }
    Ok(())
}

fn missing_expression(
    container: &Container,
    options: &attrs::Options,
    runtime: &syn::Path,
) -> syn::Result<TokenStream> {
    if let Some(accepted) = options.missing {
        return Ok(if accepted {
            quote!(#runtime::MissingField::Accepted)
        } else {
            quote!(#runtime::MissingField::Rejected)
        });
    }
    if options.wire(Mode::Deserialize).is_none() && container.transparent {
        if let Data::Struct(_, fields) = &container.data {
            if let Some(field) = fields.iter().find(|field| field.transparent_member) {
                let field_options = attrs::parse(&field.attrs, false)?;
                if !field.custom_codec && field_options.wire(Mode::Deserialize).is_none() {
                    let ty = &field.ty;
                    return Ok(quote_spanned!(ty.span()=>
                        <#ty as #runtime::Schema>::missing_field()
                    ));
                }
            }
        }
    }
    // A present-value override does not establish missing-value semantics.
    Ok(quote!(#runtime::MissingField::Rejected))
}

struct Emitter {
    runtime: syn::Path,
    mode: Mode,
    used_types: Vec<syn::Type>,
}

impl Emitter {
    fn reference(&mut self, ty: &syn::Type) -> TokenStream {
        self.used_types.push(ty.clone());
        quote_spanned!(ty.span()=> __spline_cx.schema::<#ty>()?)
    }

    fn container(
        &mut self,
        container: &Container,
        span: proc_macro2::Span,
    ) -> syn::Result<TokenStream> {
        let runtime = self.runtime.clone();
        match &container.data {
            Data::Struct(style, fields) => {
                if !matches!(container.tagging, Tagging::External) {
                    return Err(syn::Error::new(
                        span,
                        "Serde tagging on structs needs a manual Schema",
                    ));
                }
                if container.transparent {
                    let field = fields
                        .iter()
                        .find(|field| field.transparent_member)
                        .ok_or_else(|| {
                            syn::Error::new(span, "no transparent field in this direction")
                        })?;
                    return self.scalar_field(field);
                }
                match style {
                    Style::Named => {
                        let fields = self.fields(fields, container.defaulted)?;
                        let unknown = self.unknown(container.deny_unknown_fields);
                        Ok(quote!(#runtime::Shape::Object(#runtime::Object {
                            fields: #fields, unknown_fields: #unknown,
                        })))
                    }
                    Style::Newtype => self.scalar_field(&fields[0]),
                    Style::Unit => Ok(quote!(#runtime::Shape::Null)),
                    Style::Tuple => Err(syn::Error::new(
                        span,
                        "tuple structs need a manual Schema in this first slice",
                    )),
                }
            }
            Data::Enum(variants) => self.enumeration(container, variants, span),
        }
    }

    fn fields(&mut self, fields: &[Field], defaulted: bool) -> syn::Result<TokenStream> {
        let mut names = BTreeSet::new();
        let mut values = Vec::new();
        for field in fields.iter().filter(|field| !field.skipped) {
            if !names.insert(field.name.value()) {
                return Err(syn::Error::new_spanned(
                    &field.name,
                    "duplicate wire field name in this Schema direction",
                ));
            }
            values.push(self.field(field, defaulted)?);
        }
        let runtime = &self.runtime;
        let count = values.len();
        Ok(quote!({
            let __spline_fields: [::core::option::Option<#runtime::Field>; #count] = [#(#values),*];
            ::core::iter::IntoIterator::into_iter(__spline_fields).flatten().collect()
        }))
    }

    fn field(&mut self, field: &Field, container_default: bool) -> syn::Result<TokenStream> {
        check_field_attributes(field, self.mode)?;
        let options = attrs::parse(&field.attrs, false)?;
        let wire = options.wire(self.mode);
        if field.custom_codec && wire.is_none() {
            return Err(codec_error(field, self.mode));
        }
        let ty = wire.unwrap_or(&field.ty);
        self.used_types.push(ty.clone());
        let runtime = &self.runtime;
        let name = &field.name;
        let defaulted = field.defaulted || container_default;
        let custom_codec = field.custom_codec;
        let precise_none = field.omission.as_ref().is_some_and(standard_option_none);
        let omission = if field.omission.is_none() {
            quote!(#runtime::Omission::Never)
        } else if precise_none {
            quote!(#runtime::Omission::IfNone)
        } else {
            quote!(#runtime::Omission::Conditional)
        };
        let rules = quote!(#runtime::FieldRules {
            skipped: false, defaulted: #defaulted, omission: #omission, custom_codec: #custom_codec,
        });
        let call = if wire.is_some() {
            quote_spanned!(ty.span()=> __spline_cx.field_with::<#ty>(#name, #rules)?)
        } else if precise_none {
            quote_spanned!(ty.span()=> __spline_cx.option_field::<#ty>(#name, #rules)?)
        } else {
            quote_spanned!(ty.span()=> __spline_cx.field::<#ty>(#name, #rules)?)
        };
        let docs = attrs::docs(&field.attrs);
        Ok(quote!({
            let __spline_field = #call;
            __spline_field.map(|mut __spline_field| {
                __spline_field.documentation = ::std::vec![#(::std::string::String::from(#docs)),*];
                __spline_field
            })
        }))
    }

    fn scalar_field(&mut self, field: &Field) -> syn::Result<TokenStream> {
        check_field_attributes(field, self.mode)?;
        if field.skipped
            || field.omission.is_some()
            || (self.mode == Mode::Deserialize && field.defaulted)
        {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "positional payload skipping, omission, or input defaults need a manual Schema",
            ));
        }
        let options = attrs::parse(&field.attrs, false)?;
        let wire = options.wire(self.mode);
        if field.custom_codec && wire.is_none() {
            return Err(codec_error(field, self.mode));
        }
        Ok(self.reference(wire.unwrap_or(&field.ty)))
    }

    fn enumeration(
        &mut self,
        container: &Container,
        variants: &[Variant],
        span: proc_macro2::Span,
    ) -> syn::Result<TokenStream> {
        if matches!(container.tagging, Tagging::Untagged) {
            return Err(syn::Error::new(
                span,
                "untagged enums need a manual Schema or whole-type override",
            ));
        }
        let mut names = BTreeSet::new();
        let mut shapes = Vec::new();
        for variant in variants.iter().filter(|variant| !variant.skipped) {
            check_variant_attributes(variant, self.mode)?;
            if !names.insert(variant.name.value()) {
                return Err(syn::Error::new_spanned(
                    &variant.name,
                    "duplicate wire variant name in this Schema direction",
                ));
            }
            let runtime = self.runtime.clone();
            let name = &variant.name;
            let unknown = self.unknown(container.deny_unknown_fields);
            if matches!(container.tagging, Tagging::Internal(_)) {
                let Tagging::Internal(tag) = &container.tagging else {
                    unreachable!()
                };
                if !matches!(variant.style, Style::Unit | Style::Named) {
                    return Err(syn::Error::new(
                        variant.span,
                        "internally tagged newtype/tuple variants need a manual Schema",
                    ));
                }
                let fields = self.fields(&variant.fields, false)?;
                shapes.push(quote!({
                    let mut __spline_fields: ::std::vec::Vec<#runtime::Field> = #fields;
                    __spline_fields.insert(0, #runtime::Field::required(
                        #tag, #runtime::Shape::Literal(::std::string::String::from(#name))
                    ));
                    #runtime::Shape::Object(#runtime::Object {
                        fields: __spline_fields, unknown_fields: #unknown,
                    })
                }));
                continue;
            }
            let payload = match variant.style {
                Style::Unit => quote!(#runtime::Shape::Null),
                Style::Newtype => self.scalar_field(&variant.fields[0])?,
                Style::Named => {
                    let fields = self.fields(&variant.fields, false)?;
                    quote!(#runtime::Shape::Object(#runtime::Object {
                        fields: #fields, unknown_fields: #unknown,
                    }))
                }
                Style::Tuple => {
                    return Err(syn::Error::new(
                        variant.span,
                        "tuple variants need a manual Schema in this first slice",
                    ))
                }
            };
            match &container.tagging {
                Tagging::External if variant.style == Style::Unit => {
                    shapes
                        .push(quote!(#runtime::Shape::Literal(::std::string::String::from(#name))));
                }
                Tagging::External => shapes.push(quote!(#runtime::Shape::Object(#runtime::Object {
                    fields: ::std::vec![#runtime::Field::required(#name, #payload)],
                    unknown_fields: #runtime::UnknownFields::Reject,
                }))),
                Tagging::Adjacent { tag, content } => {
                    let content_field = if variant.style == Style::Unit {
                        if self.mode == Mode::Deserialize {
                            quote! {
                                let mut __spline_content = #runtime::Field::required(#content, #runtime::Shape::Null);
                                __spline_content.presence = #runtime::Presence::Optional;
                                __spline_fields.push(__spline_content);
                            }
                        } else {
                            TokenStream::new()
                        }
                    } else {
                        let presence = if self.mode == Mode::Deserialize
                            && variant.style == Style::Newtype
                        {
                            let field = &variant.fields[0];
                            if field.custom_codec
                                || attrs::parse(&field.attrs, false)?.wire(self.mode).is_some()
                            {
                                return Err(syn::Error::new_spanned(&field.ty,
                                    "adjacent newtype payload codecs/overrides need a manual deserialize Schema (missing-content behavior differs from struct fields)"));
                            }
                            let ty = &field.ty;
                            quote!(if <#ty as #runtime::Schema>::missing_field() == #runtime::MissingField::Accepted {
                                #runtime::Presence::Optional
                            } else { #runtime::Presence::Required })
                        } else {
                            quote!(#runtime::Presence::Required)
                        };
                        quote! {
                            let mut __spline_content = #runtime::Field::required(#content, #payload);
                            __spline_content.presence = #presence;
                            __spline_fields.push(__spline_content);
                        }
                    };
                    shapes.push(quote!({
                        let mut __spline_fields = ::std::vec![#runtime::Field::required(
                            #tag, #runtime::Shape::Literal(::std::string::String::from(#name))
                        )];
                        #content_field
                        #runtime::Shape::Object(#runtime::Object {
                            fields: __spline_fields, unknown_fields: #unknown,
                        })
                    }));
                }
                _ => unreachable!(),
            }
        }
        if shapes.is_empty() {
            return Err(syn::Error::new(
                span,
                format!("enum has no variants in {} Schema", self.mode.label()),
            ));
        }
        let runtime = &self.runtime;
        Ok(quote!(#runtime::Shape::Union(::std::vec![#(#shapes),*])))
    }

    fn unknown(&self, deny: bool) -> TokenStream {
        let runtime = &self.runtime;
        if deny || self.mode == Mode::Serialize {
            quote!(#runtime::UnknownFields::Reject)
        } else {
            quote!(#runtime::UnknownFields::Ignore)
        }
    }
}

fn codec_error(field: &Field, mode: Mode) -> syn::Error {
    syn::Error::new_spanned(
        &field.ty,
        format!(
        "custom Serde {} codec needs #[spline(schema = \"WireType\")] or a directional override",
        mode.label(),
    ),
    )
}

fn check_container_attributes(attributes: &[syn::Attribute], mode: Mode) -> syn::Result<()> {
    let mut allowed = vec![
        "rename",
        "rename_all",
        "rename_all_fields",
        "deny_unknown_fields",
        "default",
        "transparent",
        "tag",
        "content",
        "bound",
        "crate",
        "expecting",
    ];
    match mode {
        Mode::Serialize => allowed.extend(["from", "try_from"]),
        Mode::Deserialize => allowed.push("into"),
    }
    attrs::check_serde(attributes, &allowed, mode)
}

fn check_field_attributes(field: &Field, mode: Mode) -> syn::Result<()> {
    let mut allowed = vec![
        "rename",
        "default",
        "skip",
        "skip_serializing",
        "skip_deserializing",
        "skip_serializing_if",
        "with",
        "serialize_with",
        "deserialize_with",
        "bound",
    ];
    if mode == Mode::Serialize {
        allowed.extend(["alias", "borrow"]);
    }
    attrs::check_serde(&field.attrs, &allowed, mode)
}

fn check_variant_attributes(variant: &Variant, mode: Mode) -> syn::Result<()> {
    let mut allowed = vec![
        "rename",
        "rename_all",
        "skip",
        "skip_serializing",
        "skip_deserializing",
    ];
    if mode == Mode::Serialize {
        allowed.push("alias");
    }
    attrs::check_serde(&variant.attrs, &allowed, mode)
}

/// Short/aliased function paths are opaque and receive a conservative shape.
fn standard_option_none(path: &syn::ExprPath) -> bool {
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    path.qself.is_none()
        && path.path.leading_colon.is_some()
        && path
            .path
            .segments
            .iter()
            .all(|s| matches!(s.arguments, syn::PathArguments::None))
        && (segments == ["core", "option", "Option", "is_none"]
            || segments == ["std", "option", "Option", "is_none"])
}

fn mode_token(mode: Mode, runtime: &syn::Path) -> TokenStream {
    match mode {
        Mode::Deserialize => quote!(#runtime::Direction::Deserialize),
        Mode::Serialize => quote!(#runtime::Direction::Serialize),
    }
}

fn relevant_parameters(generics: &syn::Generics, types: &[syn::Type]) -> BTreeSet<String> {
    struct Parameters {
        candidates: BTreeSet<String>,
        used: BTreeSet<String>,
    }
    impl<'ast> Visit<'ast> for Parameters {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if path.path.leading_colon.is_none() {
                if let Some(segment) = path.path.segments.first() {
                    let name = segment.ident.to_string();
                    if self.candidates.contains(&name) {
                        self.used.insert(name);
                    }
                }
            }
            visit::visit_type_path(self, path);
        }
    }
    let mut parameters = Parameters {
        candidates: generics
            .type_params()
            .map(|p| p.ident.to_string())
            .collect(),
        used: BTreeSet::new(),
    };
    for ty in types {
        parameters.visit_type(ty);
    }
    parameters.used
}
