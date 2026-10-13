//! All coupling to Serde's unstable AST is localized here. Types and spans stay
//! as compiler-side syntax; no generated code references Serde private modules.

use proc_macro2::{Ident, Span};
use serde_derive_internals::{ast, attr, name::MultiName, Ctxt, Derive};
use syn::spanned::Spanned;

use crate::model::*;

pub fn parse(input: &syn::DeriveInput, mode: Mode) -> syn::Result<Container> {
    let context = Ctxt::new();
    let derive = match mode {
        Mode::Serialize => Derive::Serialize,
        Mode::Deserialize => Derive::Deserialize,
    };
    let parsed = ast::Container::from_ast(
        &context,
        input,
        derive,
        &Ident::new("__spline_schema_private", Span::call_site()),
    );
    context.check()?;
    let parsed = parsed.ok_or_else(|| syn::Error::new_spanned(input, "no Serde container"))?;
    let data = match &parsed.data {
        ast::Data::Struct(style, fields) => Data::Struct(
            lower_style(*style),
            fields.iter().map(|f| field(f, mode)).collect(),
        ),
        ast::Data::Enum(variants) => Data::Enum(
            variants
                .iter()
                .map(|variant| Variant {
                    name: name(variant.attrs.name(), mode),
                    attrs: variant.original.attrs.clone(),
                    style: lower_style(variant.style),
                    fields: variant.fields.iter().map(|f| field(f, mode)).collect(),
                    skipped: match mode {
                        Mode::Serialize => variant.attrs.skip_serializing(),
                        Mode::Deserialize => variant.attrs.skip_deserializing(),
                    },
                    span: variant.original.span(),
                })
                .collect(),
        ),
    };
    Ok(Container {
        data,
        transparent: parsed.attrs.transparent(),
        defaulted: !parsed.attrs.default().is_none(),
        deny_unknown_fields: parsed.attrs.deny_unknown_fields(),
        tagging: match parsed.attrs.tag() {
            attr::TagType::External => Tagging::External,
            attr::TagType::Internal { tag } => {
                Tagging::Internal(syn::LitStr::new(tag, input.ident.span()))
            }
            attr::TagType::Adjacent { tag, content } => Tagging::Adjacent {
                tag: syn::LitStr::new(tag, input.ident.span()),
                content: syn::LitStr::new(content, input.ident.span()),
            },
            attr::TagType::None => Tagging::Untagged,
        },
    })
}

fn field(field: &ast::Field<'_>, mode: Mode) -> Field {
    Field {
        ty: field.ty.clone(),
        attrs: field.original.attrs.clone(),
        name: name(field.attrs.name(), mode),
        skipped: match mode {
            Mode::Serialize => field.attrs.skip_serializing(),
            Mode::Deserialize => field.attrs.skip_deserializing(),
        },
        defaulted: !field.attrs.default().is_none(),
        omission: match mode {
            Mode::Serialize => field.attrs.skip_serializing_if().cloned(),
            Mode::Deserialize => None,
        },
        custom_codec: match mode {
            Mode::Serialize => field.attrs.serialize_with().is_some(),
            Mode::Deserialize => field.attrs.deserialize_with().is_some(),
        },
        transparent_member: field.attrs.transparent(),
    }
}

fn name(name: &MultiName, mode: Mode) -> syn::LitStr {
    let value = match mode {
        Mode::Serialize => name.serialize_name(),
        Mode::Deserialize => name.deserialize_name(),
    };
    syn::LitStr::new(&value.value, value.span)
}

fn lower_style(style: ast::Style) -> Style {
    match style {
        ast::Style::Struct => Style::Named,
        ast::Style::Newtype => Style::Newtype,
        ast::Style::Tuple => Style::Tuple,
        ast::Style::Unit => Style::Unit,
    }
}
