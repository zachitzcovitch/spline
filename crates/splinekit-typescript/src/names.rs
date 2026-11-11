use std::collections::{BTreeMap, BTreeSet};

use splinekit_schema::{DefinitionId, Direction, Graph, Name};

use crate::Error;

/// Public identifiers use the type's final name component. Full logical names
/// remain graph/runtime identities; collisions require a deliberate override.
pub(crate) fn assign(
    graph: &Graph,
    selected: &BTreeSet<DefinitionId>,
    overrides: &BTreeMap<DefinitionId, String>,
) -> Result<BTreeMap<DefinitionId, String>, Error> {
    let mut symbols = BTreeMap::new();
    let mut owners = BTreeMap::<String, DefinitionId>::new();
    for id in selected {
        let other = DefinitionId {
            name: id.name.clone(),
            direction: match id.direction {
                Direction::Deserialize => Direction::Serialize,
                Direction::Serialize => Direction::Deserialize,
            },
        };
        let both = selected.contains(&other);
        let shared = both
            && overrides.get(id) == overrides.get(&other)
            && graph.equivalent(
                &graph.definitions()[id].shape,
                &graph.definitions()[&other].shape,
            );
        let symbol = overrides.get(id).cloned().unwrap_or_else(|| {
            let base = component(&id.name);
            if both && !shared {
                format!(
                    "{base}{}",
                    match id.direction {
                        Direction::Deserialize => "Input",
                        Direction::Serialize => "Output",
                    }
                )
            } else {
                base
            }
        });
        if let Some(owner) = owners.get(&symbol) {
            if !shared || owner != &other {
                return Err(Error::SymbolCollision(symbol));
            }
        } else {
            owners.insert(symbol.clone(), id.clone());
        }
        symbols.insert(id.clone(), symbol);
    }
    Ok(symbols)
}

fn component(name: &Name) -> String {
    let base = name.path().rsplit('.').next().unwrap();
    if name.arguments().is_empty() {
        base.into()
    } else {
        format!(
            "{base}Of{}",
            name.arguments()
                .iter()
                .map(component)
                .collect::<Vec<_>>()
                .join("And")
        )
    }
}

/// The TypeScript property name for a field: camelCase of its authored
/// (Rust) name, falling back to the wire name. Names without `_` or `-` are
/// kept; leading and trailing underscores are preserved.
pub(crate) fn field(field: &splinekit_schema::Field) -> String {
    camel(field.source())
}

pub(crate) fn camel(name: &str) -> String {
    let start = name.trim_start_matches('_');
    let core = start.trim_end_matches('_');
    if !core.contains(['_', '-']) {
        return name.to_owned();
    }
    let mut output = "_".repeat(name.len() - start.len());
    for (index, word) in core.split(['_', '-']).enumerate() {
        if index == 0 {
            output.push_str(word);
            continue;
        }
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            output.extend(first.to_uppercase());
            output.push_str(characters.as_str());
        }
    }
    output.push_str(&"_".repeat(start.len() - core.len()));
    output
}

/// Which property names a TypeScript declaration uses: the wire's, for types
/// that describe raw JSON, or idiomatic ones, for API exports whose runtime
/// maps between them and the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Properties {
    Wire,
    Idiomatic,
}

impl Properties {
    pub(crate) fn of(self, field: &splinekit_schema::Field) -> String {
        match self {
            Self::Wire => field.name.clone(),
            Self::Idiomatic => self::field(field),
        }
    }
}
