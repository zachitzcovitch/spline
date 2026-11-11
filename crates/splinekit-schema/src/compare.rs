use std::collections::BTreeSet;

use crate::{DefinitionId, Graph, Shape};

impl Graph {
    /// Compare directional shapes through references, including mutual recursion.
    /// References must identify the same logical type; documentation and source
    /// variant identities do not affect structural equivalence. Missing references
    /// are not equivalent. Field order and union-alternative order are preserved.
    pub fn equivalent(&self, left: &Shape, right: &Shape) -> bool {
        equivalent(left, right, self, &mut BTreeSet::new())
    }
}

fn equivalent(
    left: &Shape,
    right: &Shape,
    graph: &Graph,
    visited: &mut BTreeSet<(DefinitionId, DefinitionId)>,
) -> bool {
    match (left, right) {
        (Shape::Ref(a), Shape::Ref(b)) => {
            let (Some(left), Some(right)) =
                (graph.definitions().get(a), graph.definitions().get(b))
            else {
                return false;
            };
            a.name == b.name
                && (!visited.insert((a.clone(), b.clone()))
                    || equivalent(&left.shape, &right.shape, graph, visited))
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
