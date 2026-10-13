use syn::punctuated::Punctuated;
use syn::{Attribute, LitStr, Meta, Path, Token, Type};

use crate::model::Mode;

#[derive(Default)]
pub struct Options {
    pub name: Option<LitStr>,
    pub runtime: Option<Path>,
    pub only: Option<Mode>,
    pub missing: Option<bool>,
    schema: Option<Type>,
    serialize_as: Option<Type>,
    deserialize_as: Option<Type>,
}

impl Options {
    pub fn wire(&self, mode: Mode) -> Option<&Type> {
        match mode {
            Mode::Serialize => self.serialize_as.as_ref(),
            Mode::Deserialize => self.deserialize_as.as_ref(),
        }
        .or(self.schema.as_ref())
    }

    pub fn enabled(&self, mode: Mode) -> bool {
        self.only.is_none_or(|only| only == mode)
    }
}

pub fn parse(attributes: &[Attribute], container: bool) -> syn::Result<Options> {
    let mut options = Options::default();
    for attribute in attributes.iter().filter(|a| a.path().is_ident("spline")) {
        attribute.parse_nested_meta(|meta| {
            let value: LitStr = meta.value()?.parse()?;
            if meta.path.is_ident("name") && container {
                validate_name(&value)?;
                set(&mut options.name, value.clone(), &value, "name")?;
            } else if meta.path.is_ident("crate") && container {
                set(&mut options.runtime, value.parse()?, &value, "crate")?;
            } else if meta.path.is_ident("only") && container {
                let mode = match value.value().as_str() {
                    "serialize" => Mode::Serialize,
                    "deserialize" => Mode::Deserialize,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            value,
                            "expected serialize or deserialize",
                        ))
                    }
                };
                set(&mut options.only, mode, &value, "only")?;
            } else if meta.path.is_ident("missing") && container {
                let accepted = match value.value().as_str() {
                    "accepted" => true,
                    "rejected" => false,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            value,
                            "expected accepted or rejected",
                        ))
                    }
                };
                set(&mut options.missing, accepted, &value, "missing")?;
            } else if meta.path.is_ident("schema") {
                set(&mut options.schema, value.parse()?, &value, "schema")?;
            } else if meta.path.is_ident("serialize_as") {
                set(
                    &mut options.serialize_as,
                    value.parse()?,
                    &value,
                    "serialize_as",
                )?;
            } else if meta.path.is_ident("deserialize_as") {
                set(
                    &mut options.deserialize_as,
                    value.parse()?,
                    &value,
                    "deserialize_as",
                )?;
            } else {
                return Err(meta.error("unsupported spline option at this location"));
            }
            Ok(())
        })?;
    }
    if let Some(schema) = &options.schema {
        if options.serialize_as.is_some() || options.deserialize_as.is_some() {
            return Err(syn::Error::new_spanned(
                schema,
                "schema conflicts with serialize_as/deserialize_as; choose shared or directional overrides",
            ));
        }
    }
    Ok(options)
}

fn set<T>(slot: &mut Option<T>, value: T, literal: &LitStr, option: &str) -> syn::Result<()> {
    if slot.replace(value).is_some() {
        return Err(syn::Error::new_spanned(
            literal,
            format!("duplicate spline option {option}"),
        ));
    }
    Ok(())
}

fn validate_name(name: &LitStr) -> syn::Result<()> {
    for part in name.value().split('.') {
        let mut chars = part.chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(syn::Error::new_spanned(
                name,
                "logical names must be dot-separated ASCII identifiers",
            ));
        }
    }
    Ok(())
}

/// Only classifies attribute keys. Serde's adapter owns value interpretation.
pub fn check_serde(attributes: &[Attribute], allowed: &[&str], mode: Mode) -> syn::Result<()> {
    for attribute in attributes.iter().filter(|a| a.path().is_ident("serde")) {
        let items = attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for item in items {
            if !allowed.iter().any(|name| item.path().is_ident(name)) {
                return Err(syn::Error::new_spanned(item.path(), format!(
                    "unsupported Serde representation in {} Schema; use a manual implementation or whole-type schema override",
                    mode.label(),
                )));
            }
        }
    }
    Ok(())
}

pub fn docs(attributes: &[Attribute]) -> Vec<LitStr> {
    attributes
        .iter()
        .filter_map(|attribute| {
            if !attribute.path().is_ident("doc") {
                return None;
            }
            let Meta::NameValue(meta) = &attribute.meta else {
                return None;
            };
            let syn::Expr::Lit(expression) = &meta.value else {
                return None;
            };
            let syn::Lit::Str(value) = &expression.lit else {
                return None;
            };
            Some(LitStr::new(value.value().trim(), value.span()))
        })
        .collect()
}
