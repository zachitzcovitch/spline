use std::collections::{BTreeMap, BTreeSet};

use spline_schema::{DefinitionId, Direction, Graph, Name};

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
