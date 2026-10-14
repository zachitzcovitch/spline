use std::collections::{BTreeMap, BTreeSet};

use spline_schema::{DefinitionId, Direction, Graph, Name, Shape};

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
            && equivalent(
                &graph.definitions()[id].shape,
                &graph.definitions()[&other].shape,
                graph,
                &mut BTreeSet::new(),
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

/// Compare both directions through references, including mutual recursion.
/// Only the same logical type can share a name. Presence, field names, numeric
/// ranges, and unknown-field rules must agree; documentation is not type shape.
fn equivalent(
    left: &Shape,
    right: &Shape,
    graph: &Graph,
    visited: &mut BTreeSet<(DefinitionId, DefinitionId)>,
) -> bool {
    match (left, right) {
        (Shape::Ref(a), Shape::Ref(b)) => {
            a.name == b.name
                && (!visited.insert((a.clone(), b.clone()))
                    || equivalent(
                        &graph.definitions()[a].shape,
                        &graph.definitions()[b].shape,
                        graph,
                        visited,
                    ))
        }
        (Shape::Array(a), Shape::Array(b)) | (Shape::Map(a), Shape::Map(b)) => {
            equivalent(a, b, graph, visited)
        }
        (Shape::Tuple(a), Shape::Tuple(b)) | (Shape::Union(a), Shape::Union(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| equivalent(a, b, graph, visited))
        }
        (Shape::Object(a), Shape::Object(b)) => {
            a.unknown_fields == b.unknown_fields
                && a.fields.len() == b.fields.len()
                && a.fields.iter().zip(&b.fields).all(|(a, b)| {
                    a.name == b.name
                        && a.presence == b.presence
                        && equivalent(&a.shape, &b.shape, graph, visited)
                })
        }
        _ => left == right,
    }
}
