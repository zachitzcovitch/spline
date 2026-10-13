use serde_json::Value;
use spline::*;

pub fn graph<T: Schema>(direction: Direction) -> (Shape, Graph) {
    let mut registry = Registry::new();
    let root = registry.register::<T>(direction).unwrap();
    (root, registry.finish().unwrap())
}

pub fn body<'a>(root: &'a Shape, graph: &'a Graph) -> &'a Shape {
    match root {
        Shape::Ref(id) => &graph.definitions().get(id).unwrap().shape,
        shape => shape,
    }
}

pub fn field<'a>(shape: &'a Shape, name: &str) -> &'a Field {
    let Shape::Object(object) = shape else {
        panic!("expected object")
    };
    object
        .fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
}

// Structural test oracle, not a production validator or proof of custom predicates.
pub fn matches(graph: &Graph, shape: &Shape, value: &Value) -> bool {
    fn visit(graph: &Graph, shape: &Shape, value: &Value, budget: usize) -> bool {
        if budget == 0 {
            return false;
        }
        let child = |shape: &Shape, value: &Value| visit(graph, shape, value, budget - 1);
        match shape {
            Shape::Null => value.is_null(),
            Shape::Boolean => value.is_boolean(),
            Shape::String => value.is_string(),
            Shape::Literal(literal) => value.as_str() == Some(literal),
            Shape::Integer { signed: true, bits } => value.as_i64().is_some_and(|n| {
                let limit = 1_i64 << (bits - 1);
                (-limit..limit).contains(&n)
            }),
            Shape::Integer {
                signed: false,
                bits,
            } => value.as_u64().is_some_and(|n| n < (1_u64 << bits)),
            Shape::Array(element) => value
                .as_array()
                .is_some_and(|a| a.iter().all(|v| child(element, v))),
            Shape::Map(element) => value
                .as_object()
                .is_some_and(|o| o.values().all(|v| child(element, v))),
            Shape::Tuple(elements) => value.as_array().is_some_and(|a| {
                a.len() == elements.len() && elements.iter().zip(a).all(|(s, v)| child(s, v))
            }),
            Shape::Union(alternatives) => alternatives.iter().any(|s| child(s, value)),
            Shape::Ref(id) => child(&graph.definitions().get(id).unwrap().shape, value),
            Shape::Object(object) => value.as_object().is_some_and(|o| {
                object.fields.iter().all(|f| match o.get(&f.name) {
                    Some(v) => child(&f.shape, v),
                    None => f.presence == Presence::Optional,
                }) && (object.unknown_fields == UnknownFields::Ignore
                    || o.keys()
                        .all(|key| object.fields.iter().any(|f| &f.name == key)))
            }),
        }
    }
    visit(graph, shape, value, 256)
}
