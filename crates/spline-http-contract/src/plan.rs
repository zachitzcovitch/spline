//! HTTP-specific metadata; the contract's identities and DTO shapes stay neutral.

use crate::{Error, Result};
use spline_contract::{Operation, OperationKind};
use spline_schema::{Graph, Shape};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpBinding {
    pub method: HttpMethod,
    pub path: String,
    pub body: Option<String>,
    pub success_status: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    String,
    Boolean,
    Integer { signed: bool, bits: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlParameter {
    pub name: String,
    pub scalar: Scalar,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpPlan {
    pub binding: HttpBinding,
    pub path_parameters: Vec<UrlParameter>,
    pub query_parameters: Vec<UrlParameter>,
    pub(crate) route_key: String,
}

pub(crate) fn resolve<'a>(mut shape: &'a Shape, graph: &'a Graph) -> Result<&'a Shape> {
    let mut visited = BTreeSet::new();
    while let Shape::Ref(id) = shape {
        if !visited.insert(id) {
            return Err(Error::Binding("cyclic alias in binding input".into()));
        }
        shape = &graph
            .definitions()
            .get(id)
            .ok_or_else(|| Error::Binding(format!("missing schema {id}")))?
            .shape;
    }
    Ok(shape)
}

pub(crate) fn plan(operation: &Operation, binding: HttpBinding, graph: &Graph) -> Result<HttpPlan> {
    let fail = |message: &str| Error::Binding(format!("{}: {message}", operation.id));
    if !matches!(resolve(&operation.input, graph)?, Shape::Object(_)) {
        return Err(fail("HTTP operations require a named input object"));
    }
    if !matches!(binding.success_status, 200 | 201 | 204) {
        return Err(fail("supported success statuses are 200, 201, and 204"));
    }
    if binding.success_status == 204 && !matches!(resolve(&operation.output, graph)?, Shape::Null) {
        return Err(fail("204 requires a unit/null output schema"));
    }
    if binding.method == HttpMethod::Get
        && (binding.body.is_some() || operation.kind == OperationKind::Mutation)
    {
        return Err(fail("GET must be a query without a request body"));
    }
    if matches!(
        binding.method,
        HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
    ) && operation.kind != OperationKind::Mutation
    {
        return Err(fail("PUT, PATCH, and DELETE must be mutations"));
    }
    if matches!(binding.method, HttpMethod::Put | HttpMethod::Patch) && binding.body.is_none() {
        return Err(fail("PUT and PATCH require a JSON body mapping"));
    }
    if binding.method == HttpMethod::Delete && binding.body.is_some() {
        return Err(fail(
            "DELETE cannot have a JSON body mapping in this profile",
        ));
    }
    if !binding.path.starts_with('/') || binding.path.contains(['?', '#']) {
        return Err(fail("path must be absolute without a query or fragment"));
    }
    let mut names = BTreeSet::new();
    let mut route_key = String::new();
    if binding.path != "/" {
        for segment in binding.path.split('/').skip(1) {
            route_key.push('/');
            if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                if !crate::identifier(name) || !names.insert(name.to_owned()) {
                    return Err(fail("path placeholders must be distinct argument names"));
                }
                route_key.push_str("{}");
            } else {
                if segment.is_empty()
                    || matches!(segment, "." | "..")
                    || !segment
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_~.".contains(c))
                {
                    return Err(fail("unsupported literal path segment"));
                }
                route_key.push_str(segment);
            }
        }
    } else {
        route_key.push('/');
    }
    for name in &names {
        if !operation
            .arguments
            .iter()
            .any(|argument| &argument.name == name)
        {
            return Err(fail("path placeholder has no matching argument"));
        }
    }
    if let Some(body) = &binding.body {
        if names.contains(body)
            || !operation
                .arguments
                .iter()
                .any(|argument| &argument.name == body)
        {
            return Err(fail(
                "body must select one argument that is not in the path",
            ));
        }
    }
    let mut path_parameters = Vec::new();
    let mut query_parameters = Vec::new();
    for argument in &operation.arguments {
        if binding.body.as_ref() == Some(&argument.name) {
            continue;
        }
        let (scalar, nullable) = scalar(&argument.shape, graph, &mut BTreeSet::new())?;
        let scalar =
            scalar.ok_or_else(|| fail("URL arguments must have a scalar representation"))?;
        let parameter = UrlParameter {
            name: argument.name.clone(),
            scalar,
            optional: argument.optional,
        };
        if names.contains(&argument.name) {
            if nullable || argument.optional {
                return Err(fail("path arguments must be required and non-nullable"));
            }
            path_parameters.push(parameter);
        } else {
            if nullable && !argument.optional {
                return Err(fail("nullable query arguments must accept omission"));
            }
            query_parameters.push(parameter);
        }
    }
    Ok(HttpPlan {
        binding,
        path_parameters,
        query_parameters,
        route_key,
    })
}

fn scalar(
    shape: &Shape,
    graph: &Graph,
    visited: &mut BTreeSet<spline_schema::DefinitionId>,
) -> Result<(Option<Scalar>, bool)> {
    Ok(match shape {
        Shape::Ref(id) => {
            if !visited.insert(id.clone()) {
                return Err(Error::Binding("recursive URL argument".into()));
            }
            let result = scalar(&graph.definitions()[id].shape, graph, visited)?;
            visited.remove(id);
            result
        }
        Shape::String | Shape::Literal(_) => (Some(Scalar::String), false),
        Shape::Boolean => (Some(Scalar::Boolean), false),
        Shape::Integer { signed, bits } => (
            Some(Scalar::Integer {
                signed: *signed,
                bits: *bits,
            }),
            false,
        ),
        Shape::Null => (None, true),
        Shape::Union(shapes) => {
            let mut result = None;
            let mut nullable = false;
            for shape in shapes {
                let (kind, null) = scalar(shape, graph, visited)?;
                nullable |= null;
                if let Some(kind) = kind {
                    if result.is_some_and(|previous| previous != kind) {
                        return Err(Error::Binding("mixed scalar URL union".into()));
                    }
                    result = Some(kind);
                }
            }
            (result, nullable)
        }
        _ => {
            return Err(Error::Binding(
                "URL arguments must be strings, booleans, or supported integers".into(),
            ))
        }
    })
}
/// Axum permits different methods on the same path spelling, but not competing
/// parameter spellings for the same route pattern.
pub(crate) fn conflicts(left: &HttpPlan, right: &HttpPlan) -> bool {
    left.route_key == right.route_key
        && (left.binding.path != right.binding.path || left.binding.method == right.binding.method)
}
