use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use spline_contract::Operation;
use spline_http_contract::{ErrorResponses, HttpPlan};
use spline_schema::{DefinitionId, Direction, Graph, Name, Presence, Shape, UnknownFields};

use crate::{operation::FRAMEWORK_SCHEMA, Error, Result};

pub(crate) struct Schemas<'a> {
    pub graph: &'a Graph,
    names: BTreeMap<DefinitionId, String>,
}
impl<'a> Schemas<'a> {
    pub fn new(
        graph: &'a Graph,
        operations: &[&Operation],
        plans: &BTreeMap<String, HttpPlan>,
        overrides: &BTreeMap<DefinitionId, String>,
    ) -> Result<Self> {
        let mut selected = BTreeSet::new();
        for op in operations {
            for argument in &op.arguments {
                select(
                    &argument.shape,
                    Direction::Deserialize,
                    graph,
                    &mut selected,
                )?;
            }
            if plans[&op.id].binding.success_status != 204 {
                select(&op.output, Direction::Serialize, graph, &mut selected)?;
            }
            select(&op.error, Direction::Serialize, graph, &mut selected)?;
            if let ErrorResponses::Declared(responses) = &plans[&op.id].errors {
                for response in responses.values() {
                    select(&response.body, Direction::Serialize, graph, &mut selected)?;
                }
            }
        }
        cycles(graph, &selected)?;
        let mut names = BTreeMap::new();
        let mut owners = BTreeMap::<String, DefinitionId>::new();
        for id in &selected {
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
            let name = overrides.get(id).cloned().unwrap_or_else(|| {
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
            component_name(&name)?;
            if name == FRAMEWORK_SCHEMA {
                return Err(Error::ComponentCollision(name));
            }
            if let Some(owner) = owners.get(&name) {
                if !shared || owner != &other {
                    return Err(Error::ComponentCollision(name));
                }
            } else {
                owners.insert(name.clone(), id.clone());
            }
            names.insert(id.clone(), name);
        }
        Ok(Self { graph, names })
    }

    pub fn components(&self) -> Result<Map<String, Value>> {
        let mut components = Map::new();
        for (id, name) in &self.names {
            if components.contains_key(name) {
                continue;
            }
            let definition = &self.graph.definitions()[id];
            let mut schema = self.render(&definition.shape)?;
            if !definition.documentation.is_empty() {
                schema["description"] = definition.documentation.clone().into();
            }
            components.insert(name.clone(), schema);
        }
        Ok(components)
    }

    pub fn render(&self, shape: &Shape) -> Result<Value> {
        Ok(match shape {
            Shape::Null => json!({"type":"null"}),
            Shape::Boolean => json!({"type":"boolean"}),
            Shape::String => json!({"type":"string"}),
            Shape::Literal(value) => json!({"type":"string","const":value}),
            Shape::Integer { signed, bits } => {
                if !matches!(bits, 8 | 16 | 32) {
                    return Err(spline_schema::Error::Unsupported(format!(
                        "{bits}-bit JSON integer"
                    ))
                    .into());
                }
                let (min, max) = if *signed {
                    let limit = 1_i64 << (bits - 1);
                    (-limit, limit - 1)
                } else {
                    (0, (1_i64 << bits) - 1)
                };
                json!({"type":"integer","minimum":min,"maximum":max})
            }
            Shape::Array(element) => json!({"type":"array","items":self.render(element)?}),
            Shape::Tuple(elements) => {
                let mut tuple = json!({"type":"array","items":false,"minItems":elements.len(),"maxItems":elements.len()});
                if !elements.is_empty() {
                    tuple["prefixItems"] = json!(elements
                        .iter()
                        .map(|e| self.render(e))
                        .collect::<Result<Vec<_>>>()?);
                }
                tuple
            }
            Shape::Map(element) => {
                json!({"type":"object","additionalProperties":self.render(element)?})
            }
            Shape::Object(object) => {
                let mut properties = Map::new();
                let mut required = Vec::new();
                for field in &object.fields {
                    let mut value = self.render(&field.shape)?;
                    if !field.documentation.is_empty() {
                        value["description"] = field.documentation.join("\n").into();
                    }
                    properties.insert(field.name.clone(), value);
                    if field.presence == Presence::Required {
                        required.push(field.name.clone());
                    }
                }
                let mut value = json!({"type":"object","properties":properties,"additionalProperties":object.unknown_fields==UnknownFields::Ignore});
                if !required.is_empty() {
                    value["required"] = json!(required);
                }
                value
            }
            Shape::Union(elements) => {
                json!({"anyOf":elements.iter().map(|e|self.render(e)).collect::<Result<Vec<_>>>()?})
            }
            Shape::Ref(id) => {
                let name = self
                    .names
                    .get(id)
                    .ok_or_else(|| spline_schema::Error::UnresolvedReference(id.clone()))?;
                json!({"$ref":format!("#/components/schemas/{name}")})
            }
        })
    }

    /// URL null means omission. The binding has already validated a scalar shape.
    pub fn present(&self, shape: &Shape) -> Result<Option<Value>> {
        match shape {
            Shape::Null => Ok(None),
            Shape::Ref(id) => self.present(&self.graph.definitions()[id].shape),
            Shape::Union(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    if let Some(value) = self.present(element)? {
                        values.push(value);
                    }
                }
                Ok(match values.len() {
                    0 => None,
                    1 => values.pop(),
                    _ => Some(json!({"anyOf":values})),
                })
            }
            _ => self.render(shape).map(Some),
        }
    }

    pub fn argument_documentation(&self, op: &Operation, name: &str) -> String {
        let mut shape = &op.input;
        while let Shape::Ref(id) = shape {
            shape = &self.graph.definitions()[id].shape;
        }
        if let Shape::Object(object) = shape {
            if let Some(field) = object.fields.iter().find(|field| field.name == name) {
                return field.documentation.join("\n");
            }
        }
        String::new()
    }
}

pub(crate) fn component_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err(Error::InvalidComponentName(name.into()));
    }
    Ok(())
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

fn select(
    shape: &Shape,
    direction: Direction,
    graph: &Graph,
    selected: &mut BTreeSet<DefinitionId>,
) -> Result<()> {
    match shape {
        Shape::Ref(id) => {
            if id.direction != direction {
                return Err(spline_schema::Error::DirectionMismatch(id.clone()).into());
            }
            let definition = graph
                .definitions()
                .get(id)
                .ok_or_else(|| spline_schema::Error::UnresolvedReference(id.clone()))?;
            if selected.insert(id.clone()) {
                select(&definition.shape, direction, graph, selected)?;
            }
        }
        Shape::Array(value) | Shape::Map(value) => select(value, direction, graph, selected)?,
        Shape::Tuple(values) | Shape::Union(values) => {
            for value in values {
                select(value, direction, graph, selected)?;
            }
        }
        Shape::Object(object) => {
            for field in &object.fields {
                select(&field.shape, direction, graph, selected)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn cycles(graph: &Graph, selected: &BTreeSet<DefinitionId>) -> Result<()> {
    fn edges(shape: &Shape, children: &mut Vec<DefinitionId>) {
        match shape {
            Shape::Ref(id) => children.push(id.clone()),
            Shape::Union(shapes) => {
                for shape in shapes {
                    edges(shape, children);
                }
            }
            _ => {}
        }
    }
    fn visit(
        id: &DefinitionId,
        graph: &Graph,
        done: &mut BTreeSet<DefinitionId>,
        stack: &mut Vec<DefinitionId>,
    ) -> Result<()> {
        if let Some(start) = stack.iter().position(|parent| parent == id) {
            let mut cycle = stack[start..].to_vec();
            cycle.push(id.clone());
            return Err(Error::UnguardedRecursion(cycle));
        }
        if done.contains(id) {
            return Ok(());
        }
        stack.push(id.clone());
        let mut children = Vec::new();
        edges(&graph.definitions()[id].shape, &mut children);
        for child in children {
            visit(&child, graph, done, stack)?;
        }
        stack.pop();
        done.insert(id.clone());
        Ok(())
    }
    let mut done = BTreeSet::new();
    for id in selected {
        visit(id, graph, &mut done, &mut Vec::new())?;
    }
    Ok(())
}
