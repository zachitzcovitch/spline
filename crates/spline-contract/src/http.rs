//! HTTP-specific metadata; the contract's identities and DTO shapes stay neutral.

use crate::{Error, Operation, OperationKind, Result};
use spline_schema::{Graph, Shape};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
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
            return Err(Error::Http("cyclic alias in binding input".into()));
        }
        shape = &graph
            .definitions()
            .get(id)
            .ok_or_else(|| Error::Http(format!("missing schema {id}")))?
            .shape;
    }
    Ok(shape)
}

pub(crate) fn plan(operation: &Operation, binding: HttpBinding, graph: &Graph) -> Result<HttpPlan> {
    let fail = |message: &str| Error::Http(format!("{}: {message}", operation.id));
    if !matches!(resolve(&operation.input, graph)?, Shape::Object(_)) {
        return Err(fail("HTTP operations require a named input object"));
    }
    if !matches!(binding.success_status, 200 | 201) {
        return Err(fail("supported success statuses are 200 and 201"));
    }
    if binding.method == HttpMethod::Get
        && (binding.body.is_some() || operation.kind == OperationKind::Mutation)
    {
        return Err(fail("GET must be a query without a request body"));
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
                if !super::identifier(name) || !names.insert(name.to_owned()) {
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
                return Err(Error::Http("recursive URL argument".into()));
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
                        return Err(Error::Http("mixed scalar URL union".into()));
                    }
                    result = Some(kind);
                }
            }
            (result, nullable)
        }
        _ => {
            return Err(Error::Http(
                "URL arguments must be strings, booleans, or supported integers".into(),
            ))
        }
    })
}
