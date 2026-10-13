use std::collections::{BTreeMap, BTreeSet};

use spline_schema::{DefinitionId, Direction, Graph, Shape};

use crate::Error;

pub fn symbol(value: &str) -> Result<(), Error> {
    let mut characters = value.chars();
    let start = characters
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '$'));
    let rest = characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$'));
    // Conservative declaration-name policy including primitive type keywords.
    let reserved = [
        "abstract",
        "any",
        "as",
        "asserts",
        "async",
        "await",
        "bigint",
        "boolean",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "constructor",
        "continue",
        "debugger",
        "declare",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "from",
        "function",
        "get",
        "global",
        "if",
        "implements",
        "import",
        "in",
        "infer",
        "instanceof",
        "interface",
        "intrinsic",
        "is",
        "keyof",
        "let",
        "module",
        "namespace",
        "never",
        "new",
        "null",
        "number",
        "object",
        "of",
        "override",
        "package",
        "private",
        "protected",
        "public",
        "readonly",
        "require",
        "return",
        "satisfies",
        "set",
        "static",
        "string",
        "super",
        "switch",
        "symbol",
        "this",
        "throw",
        "true",
        "try",
        "type",
        "typeof",
        "undefined",
        "unique",
        "unknown",
        "using",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ];
    if !start || !rest || reserved.contains(&value) {
        Err(Error::InvalidSymbol(value.into()))
    } else {
        Ok(())
    }
}

pub fn shape(value: &Shape, direction: Direction, graph: &Graph) -> Result<(), Error> {
    match value {
        Shape::Ref(id) => {
            if id.direction != direction {
                return Err(Error::WrongDirection(id.clone()));
            }
            if !graph.definitions().contains_key(id) {
                return Err(Error::MissingReference(id.clone()));
            }
        }
        Shape::Array(element) | Shape::Map(element) => shape(element, direction, graph)?,
        Shape::Tuple(elements) | Shape::Union(elements) => {
            if matches!(value, Shape::Union(_)) && elements.is_empty() {
                return Err(Error::InvalidShape("cannot export an empty union".into()));
            }
            for element in elements {
                shape(element, direction, graph)?;
            }
        }
        Shape::Object(object) => {
            let mut names = BTreeSet::new();
            for field in &object.fields {
                if !names.insert(&field.name) {
                    return Err(Error::InvalidShape(format!(
                        "duplicate object field: {}",
                        field.name
                    )));
                }
                shape(&field.shape, direction, graph)?;
            }
        }
        Shape::Integer { bits, .. } if !matches!(bits, 8 | 16 | 32) => {
            return Err(Error::InvalidShape(format!(
                "integer width {bits} is outside the evaluated JSON profile"
            )));
        }
        _ => {}
    }
    Ok(())
}

/// TypeScript permits recursive aliases through structural containers, but not
/// cycles made only of aliases/unions. Check those unguarded edges separately.
pub fn cycles(graph: &Graph) -> Result<(), Error> {
    fn unguarded(value: &Shape, edges: &mut BTreeSet<DefinitionId>) {
        match value {
            Shape::Ref(id) => {
                edges.insert(id.clone());
            }
            Shape::Union(values) => {
                for value in values {
                    unguarded(value, edges);
                }
            }
            _ => {}
        }
    }
    fn visit(
        id: &DefinitionId,
        edges: &BTreeMap<DefinitionId, BTreeSet<DefinitionId>>,
        done: &mut BTreeSet<DefinitionId>,
        stack: &mut Vec<DefinitionId>,
    ) -> Result<(), Error> {
        if let Some(start) = stack.iter().position(|ancestor| ancestor == id) {
            let mut cycle = stack[start..].to_vec();
            cycle.push(id.clone());
            return Err(Error::UnguardedRecursion(cycle));
        }
        if done.contains(id) {
            return Ok(());
        }
        stack.push(id.clone());
        for child in &edges[id] {
            visit(child, edges, done, stack)?;
        }
        stack.pop();
        done.insert(id.clone());
        Ok(())
    }
    let mut edges = BTreeMap::new();
    for (id, definition) in graph.definitions() {
        let mut children = BTreeSet::new();
        unguarded(&definition.shape, &mut children);
        edges.insert(id.clone(), children);
    }
    let mut done = BTreeSet::new();
    for id in edges.keys() {
        visit(id, &edges, &mut done, &mut Vec::new())?;
    }
    Ok(())
}
