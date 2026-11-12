use std::collections::BTreeMap;
use std::fmt::Write;

use splinekit_schema::{DefinitionId, Presence, Shape, UnknownFields};

use crate::names::Properties;
use crate::Error;

/// Renders with idiomatic property names (API exports).
pub fn shape(
    value: &Shape,
    symbols: &BTreeMap<DefinitionId, String>,
    indent: usize,
) -> Result<String, Error> {
    shape_with(value, symbols, indent, Properties::Idiomatic)
}

pub(crate) fn shape_with(
    value: &Shape,
    symbols: &BTreeMap<DefinitionId, String>,
    indent: usize,
    properties: Properties,
) -> Result<String, Error> {
    let shape = |value: &Shape, symbols, indent| shape_with(value, symbols, indent, properties);
    Ok(match value {
        Shape::Null => "null".into(),
        Shape::Boolean => "boolean".into(),
        Shape::String | Shape::Formatted(_) => "string".into(),
        Shape::Integer { .. } | Shape::Number { .. } => "number".into(),
        Shape::Literal(value) => string(value),
        // `string & {}` keeps the known values as editor suggestions.
        Shape::Open(known) => format!("{} | (string & {{}})", shape(known, symbols, indent)?),
        Shape::Ref(id) => symbols
            .get(id)
            .ok_or_else(|| Error::MissingReference(id.clone()))?
            .clone(),
        Shape::Array(element) => {
            let rendered = shape(element, symbols, indent)?;
            if matches!(element.as_ref(), Shape::Union(_) | Shape::Open(_)) {
                format!("({rendered})[]")
            } else {
                format!("{rendered}[]")
            }
        }
        Shape::Tuple(elements) => {
            let values = elements
                .iter()
                .map(|s| shape(s, symbols, indent))
                .collect::<Result<Vec<_>, _>>()?;
            format!("[{}]", values.join(", "))
        }
        Shape::Map(element) => format!("{{ [key: string]: {} }}", shape(element, symbols, indent)?),
        Shape::Union(alternatives) => {
            let values = alternatives
                .iter()
                .map(|s| shape(s, symbols, indent))
                .collect::<Result<Vec<_>, _>>()?;
            values.join(" | ")
        }
        Shape::Object(object) => {
            if object.fields.is_empty() {
                // TS {} means any non-nullish value, not an empty JSON object.
                match object.unknown_fields {
                    UnknownFields::Ignore => "{ [key: string]: unknown }".into(),
                    UnknownFields::Reject => "{ [key: string]: never }".into(),
                }
            } else {
                let mut output = String::from("{\n");
                let padding = "  ".repeat(indent + 1);
                for field in &object.fields {
                    output.push_str(&docs(&field.documentation.join("\n"), indent + 1));
                    let optional = if field.presence == Presence::Optional {
                        "?"
                    } else {
                        ""
                    };
                    writeln!(
                        output,
                        "{padding}{}{optional}: {};",
                        string(&properties.of(field)),
                        shape(&field.shape, symbols, indent + 1)?
                    )
                    .unwrap();
                }
                if object.unknown_fields == UnknownFields::Ignore {
                    writeln!(output, "{padding}[key: string]: unknown;").unwrap();
                }
                write!(output, "{}}}", "  ".repeat(indent)).unwrap();
                output
            }
        }
    })
}

pub fn docs(value: &str, indent: usize) -> String {
    if value.is_empty() {
        return String::new();
    }
    let padding = "  ".repeat(indent);
    let safe = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace("*/", "*\\/");
    let mut output = format!("{padding}/**\n");
    for line in safe.split('\n') {
        writeln!(output, "{padding} * {line}").unwrap();
    }
    writeln!(output, "{padding} */").unwrap();
    output
}

/// Prefer conventional property spelling while preserving arbitrary wire keys.
pub fn property(value: &str) -> String {
    if value.chars().enumerate().all(|(index, c)| {
        c.is_ascii_alphabetic() || matches!(c, '_' | '$') || (index > 0 && c.is_ascii_digit())
    }) && !value.is_empty()
    {
        value.into()
    } else {
        string(value)
    }
}

/// JSON-compatible double-quoted literal, also escaping JS line separators.
pub fn string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c < '\u{20}' || matches!(c, '\u{2028}' | '\u{2029}') => {
                write!(output, "\\u{:04x}", c as u32).unwrap();
            }
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

/// For a string union (a string enum), its values as constants beside the
/// type, so code compares against `Status.ACTIVE` rather than a literal it
/// could mistype. Each key is its value in capitals (`"allowance_exhausted"`
/// is `ALLOWANCE_EXHAUSTED`), so key and value read as the same word. A value
/// that can't make a key, or two that make the same one, means no constants.
pub(crate) fn string_constants(symbol: &str, shape: &Shape) -> Option<String> {
    let Shape::Union(alternatives) = shape else {
        return None;
    };
    let values: Vec<&str> = alternatives
        .iter()
        .map(|alternative| match alternative {
            Shape::Literal(value) => Some(value.as_str()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let keys: Vec<String> = values
        .iter()
        .map(|value| constant_key(value))
        .collect::<Option<_>>()?;
    let mut seen = std::collections::BTreeSet::new();
    if !keys.iter().all(|key| seen.insert(key.as_str())) {
        return None;
    }
    let mut source = format!("export const {symbol} = {{\n");
    for (key, value) in keys.iter().zip(&values) {
        source.push_str(&format!("  {key}: {},\n", string(value)));
    }
    source.push_str("} as const;\n\n");
    Some(source)
}

/// A value in capitals with `_` between words; `None` when that isn't a
/// plain identifier (empty, or starting with a digit).
fn constant_key(value: &str) -> Option<String> {
    let key = value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_uppercase();
    key.starts_with(|c: char| c.is_ascii_alphabetic())
        .then_some(key)
}
