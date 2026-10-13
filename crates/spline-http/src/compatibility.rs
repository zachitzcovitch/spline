use crate::BuildError;
use spline_schema::{
    DefinitionId, Direction, Graph, Presence, Registry, Schema, Shape, UnknownFields,
};
use std::collections::BTreeSet;

/// Conservative structural check for native Rust clients sharing server DTOs.
/// Custom schema overrides remain declarations, not proofs of serializer code.
pub fn verify_serde<T: Schema>() -> Result<(), BuildError> {
    let mut registry = Registry::new();
    let source = registry.register::<T>(Direction::Serialize)?;
    let target = registry.register::<T>(Direction::Deserialize)?;
    let graph = registry.finish()?;
    if compatible(&source, &target, &graph, &mut BTreeSet::new(), 256) {
        Ok(())
    } else {
        Err(BuildError::Configuration(format!(
            "{} has incompatible serialize/deserialize schemas for a shared Rust client DTO",
            T::info().name
        )))
    }
}

fn compatible(
    source: &Shape,
    target: &Shape,
    graph: &Graph,
    visited: &mut BTreeSet<(DefinitionId, DefinitionId)>,
    budget: usize,
) -> bool {
    if budget == 0 {
        return false;
    }
    let mut check =
        |source: &Shape, target: &Shape| compatible(source, target, graph, visited, budget - 1);
    match (source, target) {
        (Shape::Ref(a), Shape::Ref(b)) => {
            if !visited.insert((a.clone(), b.clone())) {
                return true;
            }
            compatible(
                &graph.definitions()[a].shape,
                &graph.definitions()[b].shape,
                graph,
                visited,
                budget - 1,
            )
        }
        (Shape::Ref(a), _) => check(&graph.definitions()[a].shape, target),
        (_, Shape::Ref(b)) => check(source, &graph.definitions()[b].shape),
        (Shape::Union(sources), _) => sources.iter().all(|source| check(source, target)),
        (_, Shape::Union(targets)) => targets.iter().any(|target| {
            // A failed alternative must not mark a pair as proven for the next one.
            compatible(source, target, graph, &mut visited.clone(), budget - 1)
        }),
        (Shape::Object(a), Shape::Object(b)) => {
            !(a.unknown_fields == UnknownFields::Ignore
                && b.unknown_fields == UnknownFields::Reject)
                && (a.unknown_fields != UnknownFields::Ignore
                    || b.fields
                        .iter()
                        .all(|field| a.fields.iter().any(|known| known.name == field.name)))
                && a.fields.iter().all(|field| {
                    b.fields
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                        .is_some_and(|other| check(&field.shape, &other.shape))
                })
                && b.fields
                    .iter()
                    .filter(|f| f.presence == Presence::Required)
                    .all(|field| {
                        a.fields.iter().any(|other| {
                            other.name == field.name && other.presence == Presence::Required
                        })
                    })
        }
        (Shape::Array(a), Shape::Array(b)) | (Shape::Map(a), Shape::Map(b)) => check(a, b),
        (Shape::Tuple(a), Shape::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| check(a, b))
        }
        (Shape::Literal(_), Shape::String) => true,
        _ => source == target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spline_schema::{Context, Field, Object, TypeInfo};

    #[test]
    fn open_producer_cannot_supply_unconstrained_values_to_known_optional_fields() {
        struct Wire;
        impl Schema for Wire {
            fn info() -> TypeInfo {
                TypeInfo::named("test.OpenProducer")
            }
            fn describe(cx: &mut Context<'_>) -> spline_schema::Result<Shape> {
                let fields = if cx.direction() == Direction::Serialize {
                    vec![]
                } else {
                    vec![Field {
                        name: "tag".into(),
                        shape: Shape::String,
                        presence: Presence::Optional,
                        documentation: vec![],
                    }]
                };
                Ok(Shape::Object(Object {
                    fields,
                    unknown_fields: UnknownFields::Ignore,
                }))
            }
        }
        // The producer permits {"tag": 1}; the consumer's optional string does not.
        assert!(verify_serde::<Wire>().is_err());
    }
}
