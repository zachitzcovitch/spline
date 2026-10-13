//! Project response JSON onto a client's known wire shape before typed decoding.

use serde_json::{Map, Value};
use spline_schema::{Graph, Presence, Shape, UnknownFields};

/// How response objects are checked against the client's generated contract.
/// Requests always retain their separately authored validation rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResponsePolicy {
    /// Validate known fields and discard unknown properties of closed objects.
    /// Open objects and map entries retain their declared semantics.
    #[default]
    Compatible,
    /// Enforce the schema's unknown-field policy without allowing additive fields.
    Strict,
}

pub(crate) fn project(
    value: &Value,
    shape: &Shape,
    graph: &Graph,
    policy: ResponsePolicy,
) -> Result<Value, String> {
    project_at(value, shape, graph, policy, 0)
}

fn project_at(
    value: &Value,
    shape: &Shape,
    graph: &Graph,
    policy: ResponsePolicy,
    depth: usize,
) -> Result<Value, String> {
    if depth >= 256 {
        return Err("response exceeds the schema traversal limit".into());
    }
    let child = |value: &Value, shape: &Shape| project_at(value, shape, graph, policy, depth + 1);
    let mismatch = || Err("response does not match its declared wire shape".into());
    match shape {
        Shape::Ref(id) => {
            let definition = graph
                .definitions()
                .get(id)
                .ok_or_else(|| format!("missing response schema {id}"))?;
            child(value, &definition.shape)
        }
        Shape::Null if value.is_null() => Ok(Value::Null),
        Shape::Boolean if value.is_boolean() => Ok(value.clone()),
        Shape::String if value.is_string() => Ok(value.clone()),
        Shape::Literal(literal) if value.as_str() == Some(literal.as_str()) => Ok(value.clone()),
        Shape::Integer { signed, bits } => {
            if !matches!(bits, 8 | 16 | 32) {
                return mismatch();
            }
            let valid = if *signed {
                let bound = 1_i64 << (bits - 1);
                value.as_i64().is_some_and(|n| (-bound..bound).contains(&n))
            } else {
                value.as_u64().is_some_and(|n| n < 1_u64 << bits)
            };
            if valid {
                Ok(value.clone())
            } else {
                mismatch()
            }
        }
        Shape::Array(element) => {
            let Some(values) = value.as_array() else {
                return mismatch();
            };
            values
                .iter()
                .map(|value| child(value, element))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        Shape::Tuple(elements) => {
            let Some(values) = value
                .as_array()
                .filter(|values| values.len() == elements.len())
            else {
                return mismatch();
            };
            values
                .iter()
                .zip(elements)
                .map(|(v, s)| child(v, s))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        Shape::Map(element) => {
            let Some(values) = value.as_object() else {
                return mismatch();
            };
            values
                .iter()
                .map(|(name, value)| child(value, element).map(|value| (name.clone(), value)))
                .collect::<Result<Map<_, _>, _>>()
                .map(Value::Object)
        }
        Shape::Object(object) => {
            let Some(values) = value.as_object() else {
                return mismatch();
            };
            if policy == ResponsePolicy::Strict
                && object.unknown_fields == UnknownFields::Reject
                && values
                    .keys()
                    .any(|name| !object.fields.iter().any(|f| &f.name == name))
            {
                return Err("response contains an unknown property".into());
            }
            let mut result = if object.unknown_fields == UnknownFields::Ignore {
                values.clone()
            } else {
                Map::new()
            };
            for field in &object.fields {
                match values.get(&field.name) {
                    Some(value) => {
                        result.insert(field.name.clone(), child(value, &field.shape)?);
                    }
                    None if field.presence == Presence::Required => {
                        return Err(format!("response is missing required field {}", field.name))
                    }
                    None => {}
                }
            }
            Ok(Value::Object(result))
        }
        Shape::Union(alternatives) => {
            let mut result = None;
            for alternative in alternatives {
                if let Ok(candidate) = child(value, alternative) {
                    if result
                        .as_ref()
                        .is_some_and(|previous| previous != &candidate)
                    {
                        return Err("response matches ambiguous union alternatives".into());
                    }
                    result = Some(candidate);
                }
            }
            result.ok_or_else(|| "response does not match any known union variant".into())
        }
        _ => mismatch(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use spline_schema::{Context, Field, Object, Registry, Schema, TypeInfo};

    fn object(fields: Vec<Field>, unknown_fields: UnknownFields) -> Shape {
        Shape::Object(Object {
            fields,
            unknown_fields,
        })
    }

    #[test]
    fn open_object_keys_and_map_entries_retain_their_declared_semantics() {
        let shape = object(
            vec![Field::required("known", Shape::String)],
            UnknownFields::Ignore,
        );
        let input = json!({ "known": "x", "future": { "anything": true } });
        for policy in [ResponsePolicy::Compatible, ResponsePolicy::Strict] {
            assert_eq!(
                project(&input, &shape, &Graph::default(), policy).unwrap(),
                input
            );
            assert!(project(&json!({ "known": 7 }), &shape, &Graph::default(), policy).is_err());
        }
        let map = Shape::Map(Box::new(Shape::String));
        assert!(project(
            &json!({ "added": 7 }),
            &map,
            &Graph::default(),
            ResponsePolicy::Compatible
        )
        .is_err());
    }

    #[test]
    fn closed_empty_objects_remain_objects_and_tuples_keep_their_arity() {
        let empty = object(vec![], UnknownFields::Reject);
        assert_eq!(
            project(
                &json!({ "added": 7 }),
                &empty,
                &Graph::default(),
                ResponsePolicy::Compatible
            )
            .unwrap(),
            json!({})
        );
        for input in [json!(7), json!(null), json!([])] {
            assert!(project(
                &input,
                &empty,
                &Graph::default(),
                ResponsePolicy::Compatible
            )
            .is_err());
        }
        let tuple = Shape::Tuple(vec![empty, Shape::Boolean]);
        assert_eq!(
            project(
                &json!([{ "added": 7 }, true]),
                &tuple,
                &Graph::default(),
                ResponsePolicy::Compatible
            )
            .unwrap(),
            json!([{}, true])
        );
        assert!(project(
            &json!([{}, true, "extra"]),
            &tuple,
            &Graph::default(),
            ResponsePolicy::Compatible
        )
        .is_err());
    }

    #[test]
    fn union_projection_rejects_ambiguity_but_accepts_identical_null_alternatives() {
        let shape = Shape::Union(vec![
            object(
                vec![Field::required("a", Shape::String)],
                UnknownFields::Reject,
            ),
            object(
                vec![Field::required("b", Shape::String)],
                UnknownFields::Reject,
            ),
        ]);
        assert!(project(
            &json!({ "a": "x", "b": "x" }),
            &shape,
            &Graph::default(),
            ResponsePolicy::Compatible
        )
        .is_err());
        let nullable = Shape::Union(vec![
            Shape::Null,
            Shape::Union(vec![Shape::String, Shape::Null]),
        ]);
        assert_eq!(
            project(
                &Value::Null,
                &nullable,
                &Graph::default(),
                ResponsePolicy::Compatible
            )
            .unwrap(),
            Value::Null
        );
    }

    #[test]
    fn recursive_references_project_finite_values_and_enforce_a_depth_limit() {
        struct Node;
        impl Schema for Node {
            fn info() -> TypeInfo {
                TypeInfo::named("test.Node")
            }
            fn describe(cx: &mut Context<'_>) -> spline_schema::Result<Shape> {
                Ok(object(
                    vec![Field::required(
                        "children",
                        Shape::Array(Box::new(cx.schema::<Self>()?)),
                    )],
                    UnknownFields::Reject,
                ))
            }
        }
        let mut registry = Registry::new();
        let root = registry
            .register::<Node>(spline_schema::Direction::Serialize)
            .unwrap();
        let graph = registry.finish().unwrap();
        let input = json!({ "children": [{ "children": [], "added": true }], "added": true });
        assert_eq!(
            project(&input, &root, &graph, ResponsePolicy::Compatible).unwrap(),
            json!({ "children": [{ "children": [] }] })
        );
        let mut deep = json!({ "children": [] });
        for _ in 0..100 {
            deep = json!({ "children": [deep] });
        }
        assert!(project(&deep, &root, &graph, ResponsePolicy::Compatible).is_err());
    }
}
