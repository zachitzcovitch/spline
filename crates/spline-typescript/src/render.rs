use std::collections::BTreeMap;
use std::fmt::Write;

use spline_schema::{DefinitionId, Presence, Shape, UnknownFields};

use crate::Error;

pub fn shape(
    value: &Shape,
    symbols: &BTreeMap<DefinitionId, String>,
    indent: usize,
) -> Result<String, Error> {
    Ok(match value {
        Shape::Null => "null".into(),
        Shape::Boolean => "boolean".into(),
        Shape::String => "string".into(),
        Shape::Integer { .. } => "number".into(),
        Shape::Literal(value) => string(value),
        Shape::Ref(id) => symbols
            .get(id)
            .ok_or_else(|| Error::MissingReference(id.clone()))?
            .clone(),
        Shape::Array(element) => {
            let rendered = shape(element, symbols, indent)?;
            if matches!(element.as_ref(), Shape::Union(_)) {
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
                        string(&field.name),
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
