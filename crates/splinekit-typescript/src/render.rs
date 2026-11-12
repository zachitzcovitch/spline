use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use splinekit_schema::{DefinitionId, Graph, Object, Presence, Shape, UnknownFields};

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
    constants_object(symbol, &values)
}

/// For a tagged union (objects told apart by one literal field, such as an
/// error's `code` or a flow's `phase`), its tag values as constants named
/// after the type and the tag: `AcceptErrorCode.QUOTE_EXPIRED`,
/// `FlowPhase.NEEDS_CUSTOMER`. None when no single field tells every
/// alternative apart, or the name is already a type's.
pub(crate) fn tag_constants(
    symbol: &str,
    shape: &Shape,
    graph: &Graph,
    taken: &BTreeSet<String>,
) -> Option<String> {
    let alternatives: Vec<&Object> = match resolve(shape, graph)? {
        Shape::Union(alternatives) => alternatives
            .iter()
            .map(|alternative| match resolve(alternative, graph)? {
                Shape::Object(object) => Some(object),
                _ => None,
            })
            .collect::<Option<_>>()?,
        Shape::Object(object) => vec![object],
        _ => return None,
    };
    let tags: Vec<(&str, Vec<&str>)> = alternatives
        .first()?
        .fields
        .iter()
        .filter_map(|field| {
            let values: Vec<&str> = alternatives
                .iter()
                .map(|object| tag_value(object, &field.name, graph))
                .collect::<Option<_>>()?;
            let distinct: BTreeSet<_> = values.iter().collect();
            (distinct.len() == values.len()).then_some((field.name.as_str(), values))
        })
        .collect();
    let [(tag, values)] = tags.as_slice() else {
        return None;
    };
    let name = format!("{symbol}{}", pascal(tag)?);
    if taken.contains(&name) {
        return None;
    }
    constants_object(&name, values)
}

/// A required field's literal value, following aliases.
fn tag_value<'a>(object: &'a Object, name: &str, graph: &'a Graph) -> Option<&'a str> {
    let field = object
        .fields
        .iter()
        .find(|field| field.name == name && field.presence == Presence::Required)?;
    match resolve(&field.shape, graph)? {
        Shape::Literal(value) => Some(value.as_str()),
        _ => None,
    }
}

fn resolve<'a>(mut shape: &'a Shape, graph: &'a Graph) -> Option<&'a Shape> {
    for _ in 0..32 {
        match shape {
            Shape::Ref(id) => shape = &graph.definitions().get(id)?.shape,
            other => return Some(other),
        }
    }
    None
}

/// `code` → `Code`, `next_action` → `NextAction`.
fn pascal(name: &str) -> Option<String> {
    let key = constant_key(name)?;
    Some(
        key.split('_')
            .map(|word| {
                let lower = word.to_ascii_lowercase();
                let mut chars = lower.chars();
                chars
                    .next()
                    .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect(),
    )
}

/// `export const {name} = { KEY: "value", … } as const;` with each key its
/// value in capitals. None when a value can't make a key, or two make the
/// same one.
fn constants_object(name: &str, values: &[&str]) -> Option<String> {
    let keys: Vec<String> = values
        .iter()
        .map(|value| constant_key(value))
        .collect::<Option<_>>()?;
    let mut seen = std::collections::BTreeSet::new();
    if !keys.iter().all(|key| seen.insert(key.as_str())) {
        return None;
    }
    let mut source = format!("export const {name} = {{\n");
    for (key, value) in keys.iter().zip(values) {
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
