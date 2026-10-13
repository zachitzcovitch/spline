//! The only module coupled to serde_derive_internals and Syn.

use proc_macro2::{Ident, Span};
use quote::ToTokens;
use serde_derive_internals::{ast, attr, name::MultiName, Ctxt, Derive};

use crate::model::*;

// This token is only used to recognize synthetic metadata, never to emit code
// referencing Serde's private runtime module.
const PRIVATE: &str = "__spline_inspection_private";

pub(super) fn inspect(source: &str, direction: Direction) -> Result<Container, Vec<Diagnostic>> {
    let input: syn::DeriveInput =
        syn::parse_str(source).map_err(|e| diagnostics(e, DiagnosticOrigin::Syntax))?;
    let context = Ctxt::new();
    let derive = match direction {
        Direction::Serialize => Derive::Serialize,
        Direction::Deserialize => Derive::Deserialize,
    };
    let parsed = ast::Container::from_ast(
        &context,
        &input,
        derive,
        &Ident::new(PRIVATE, Span::call_site()),
    );

    // Ctxt::Drop panics unless check() consumes it. In particular, consume it
    // before handling the None result returned for unsupported Rust unions.
    context
        .check()
        .map_err(|e| diagnostics(e, DiagnosticOrigin::Serde))?;
    let parsed = parsed.ok_or_else(|| {
        diagnostics(
            syn::Error::new_spanned(&input, "Serde returned no container and no diagnostic"),
            DiagnosticOrigin::Adapter,
        )
    })?;

    Ok(lower(&parsed, direction))
}

fn lower(container: &ast::Container<'_>, direction: Direction) -> Container {
    let attrs = &container.attrs;
    let data = match &container.data {
        ast::Data::Struct(style, fields) => Data::Struct {
            style: lower_style(*style),
            fields: fields.iter().map(|f| lower_field(f, direction)).collect(),
        },
        ast::Data::Enum(variants) => Data::Enum(
            variants
                .iter()
                .map(|v| lower_variant(v, direction))
                .collect(),
        ),
    };
    let conversion = match direction {
        Direction::Serialize => attrs.type_into().map(|t| Conversion::Into(syntax(t))),
        Direction::Deserialize => attrs
            .type_from()
            .map(|t| Conversion::From(syntax(t)))
            .or_else(|| {
                attrs
                    .type_try_from()
                    .map(|t| Conversion::TryFrom(syntax(t)))
            }),
    };

    Container {
        rust_name: container.ident.to_string(),
        wire_name: wire_name(attrs.name(), direction),
        direction,
        generics: syntax(container.generics),
        where_clause: container.generics.where_clause.as_ref().map(syntax),
        explicit_bounds: bounds(direction, attrs.ser_bound(), attrs.de_bound()),
        docs: docs(&container.original.attrs),
        deserialize_default: default_rule(attrs.default()),
        deny_unknown_fields: attrs.deny_unknown_fields(),
        transparent: attrs.transparent(),
        tagging: match attrs.tag() {
            attr::TagType::External => Tagging::External,
            attr::TagType::Internal { tag } => Tagging::Internal {
                tag: tag.to_string(),
            },
            attr::TagType::Adjacent { tag, content } => Tagging::Adjacent {
                tag: tag.to_string(),
                content: content.to_string(),
            },
            attr::TagType::None => Tagging::Untagged,
        },
        conversion,
        data,
    }
}

fn lower_variant(variant: &ast::Variant<'_>, direction: Direction) -> Variant {
    let attrs = &variant.attrs;
    let name = wire_name(attrs.name(), direction);
    Variant {
        rust_name: variant.ident.to_string(),
        aliases: aliases(
            attrs.aliases().iter().map(ToString::to_string),
            &name,
            direction,
        ),
        wire_name: name,
        skipped: match direction {
            Direction::Serialize => attrs.skip_serializing(),
            Direction::Deserialize => attrs.skip_deserializing(),
        },
        untagged: attrs.untagged(),
        other: attrs.other(),
        codec: codec(direction, attrs.serialize_with(), attrs.deserialize_with()),
        style: lower_style(variant.style),
        fields: variant
            .fields
            .iter()
            .map(|f| lower_field(f, direction))
            .collect(),
        docs: docs(&variant.original.attrs),
    }
}

fn lower_field(field: &ast::Field<'_>, direction: Direction) -> Field {
    let attrs = &field.attrs;
    let name = wire_name(attrs.name(), direction);
    Field {
        member: syntax(&field.member),
        rust_type: syntax(field.ty),
        aliases: aliases(
            attrs.aliases().iter().map(ToString::to_string),
            &name,
            direction,
        ),
        wire_name: name,
        skipped: match direction {
            Direction::Serialize => attrs.skip_serializing(),
            Direction::Deserialize => attrs.skip_deserializing(),
        },
        deserialize_default: default_rule(attrs.default()),
        omit_if: match direction {
            Direction::Serialize => attrs.skip_serializing_if().map(syntax),
            Direction::Deserialize => None,
        },
        codec: codec(direction, attrs.serialize_with(), attrs.deserialize_with()),
        flattened: attrs.flatten(),
        transparent_member: attrs.transparent(),
        borrowed_lifetimes: attrs.borrowed_lifetimes().iter().map(syntax).collect(),
        explicit_bounds: bounds(direction, attrs.ser_bound(), attrs.de_bound()),
        docs: docs(&field.original.attrs),
    }
}

fn wire_name(name: &MultiName, direction: Direction) -> String {
    match direction {
        Direction::Serialize => name.serialize_name().value.clone(),
        Direction::Deserialize => name.deserialize_name().value.clone(),
    }
}

fn aliases(
    names: impl Iterator<Item = String>,
    canonical: &str,
    direction: Direction,
) -> Vec<String> {
    if direction == Direction::Serialize {
        return Vec::new();
    }
    names.filter(|name| name != canonical).collect()
}

fn lower_style(style: ast::Style) -> Style {
    match style {
        ast::Style::Struct => Style::Named,
        ast::Style::Tuple => Style::Tuple,
        ast::Style::Newtype => Style::Newtype,
        ast::Style::Unit => Style::Unit,
    }
}

fn default_rule(default: &attr::Default) -> DefaultRule {
    match default {
        attr::Default::None => DefaultRule::Unspecified,
        attr::Default::Default => DefaultRule::Trait,
        attr::Default::Path(path) => DefaultRule::Function(syntax(path)),
    }
}

fn codec(
    direction: Direction,
    serialize: Option<&syn::ExprPath>,
    deserialize: Option<&syn::ExprPath>,
) -> Option<Codec> {
    let path = match direction {
        Direction::Serialize => serialize?,
        Direction::Deserialize => deserialize?,
    };
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    if direction == Direction::Deserialize {
        if segments == ["_serde", PRIVATE, "de", "borrow_cow_str"] {
            return Some(Codec::BorrowedCowString);
        }
        if segments == ["_serde", PRIVATE, "de", "borrow_cow_bytes"] {
            return Some(Codec::BorrowedCowBytes);
        }
    }
    Some(Codec::Function(syntax(path)))
}

fn bounds(
    direction: Direction,
    serialize: Option<&[syn::WherePredicate]>,
    deserialize: Option<&[syn::WherePredicate]>,
) -> Option<Vec<String>> {
    match direction {
        Direction::Serialize => serialize,
        Direction::Deserialize => deserialize,
    }
    .map(|predicates| predicates.iter().map(syntax).collect())
}

fn docs(attributes: &[syn::Attribute]) -> Vec<String> {
    attributes
        .iter()
        .filter_map(|attribute| match &attribute.meta {
            syn::Meta::NameValue(meta) if meta.path.is_ident("doc") => match &meta.value {
                syn::Expr::Lit(value) => match &value.lit {
                    syn::Lit::Str(text) => Some(text.value().trim().to_owned()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn syntax(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

fn diagnostics(error: syn::Error, origin: DiagnosticOrigin) -> Vec<Diagnostic> {
    error
        .into_iter()
        .map(|error| {
            let start = error.span().start();
            let end = error.span().end();
            Diagnostic {
                message: error.to_string(),
                origin,
                start: Position {
                    line: start.line,
                    column: start.column,
                },
                end: Position {
                    line: end.line,
                    column: end.column,
                },
            }
        })
        .collect()
}
