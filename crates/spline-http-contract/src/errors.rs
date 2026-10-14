//! Application-error declarations and their validated HTTP response shapes.

use std::collections::BTreeMap;

use spline_schema::{Field, Graph, Object, Shape, UnknownFields};

use crate::{Error, Result};

pub const ERROR_HEADER: &str = "x-spline-error";

/// A policy's declared status mapping. Opaque policies make no static status claim.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ErrorMapping {
    #[default]
    Opaque,
    /// Assign the complete serialized application-error shape one status.
    Status(u16),
    /// Exhaustively map source variant identities, independently of wire names.
    Variants(Vec<(String, u16)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorResponse {
    /// Complete JSON application-error envelope, including kind and value.
    pub body: Shape,
    pub content_type: &'static str,
    pub headers: BTreeMap<String, String>,
}

/// Declared application responses only. Framework/factory faults are distinct and
/// may use other statuses; this is not an exhaustive catalog of all HTTP responses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ErrorResponses {
    #[default]
    Opaque,
    Declared(BTreeMap<u16, ErrorResponse>),
}

pub(crate) fn responses(
    mapping: ErrorMapping,
    error: &Shape,
    graph: &Graph,
) -> Result<ErrorResponses> {
    let mut grouped: BTreeMap<u16, Vec<Shape>> = BTreeMap::new();
    match mapping {
        ErrorMapping::Opaque => return Ok(ErrorResponses::Opaque),
        ErrorMapping::Status(status) => {
            validate_status(status)?;
            grouped.insert(status, vec![error.clone()]);
        }
        ErrorMapping::Variants(rules) => {
            let Shape::Ref(id) = error else {
                return Err(Error::Binding(
                    "error variant mappings require a named enum schema".into(),
                ));
            };
            let definition = graph
                .definitions()
                .get(id)
                .ok_or_else(|| Error::Binding(format!("missing error schema {id}")))?;
            let (Some(names), Shape::Union(alternatives)) =
                (&definition.variants, &definition.shape)
            else {
                return Err(Error::Binding("error schema has no source variant identities; derive Schema without a whole-type serialization override, or use a whole-error status policy".into()));
            };
            let mut sorted = BTreeMap::new();
            for (name, status) in rules {
                validate_status(status)?;
                if sorted.insert(name.clone(), status).is_some() {
                    return Err(Error::Binding(format!(
                        "duplicate error variant mapping: {name}"
                    )));
                }
                if !names.contains(&name) {
                    return Err(Error::Binding(format!(
                        "unknown serialized error variant: {name}"
                    )));
                }
            }
            for name in names {
                if !sorted.contains_key(name) {
                    return Err(Error::Binding(format!(
                        "missing status for error variant: {name}"
                    )));
                }
            }
            // Stable grouping independent of the policy's rule order.
            for (name, status) in sorted {
                let index = names
                    .iter()
                    .position(|candidate| candidate == &name)
                    .unwrap();
                grouped
                    .entry(status)
                    .or_default()
                    .push(alternatives[index].clone());
            }
        }
    }
    Ok(ErrorResponses::Declared(
        grouped
            .into_iter()
            .map(|(status, mut shapes)| {
                let value = if shapes.len() == 1 {
                    shapes.pop().unwrap()
                } else {
                    Shape::Union(shapes)
                };
                (
                    status,
                    ErrorResponse {
                        body: Shape::Object(Object {
                            fields: vec![
                                Field::required("kind", Shape::Literal("application".into())),
                                Field::required("value", value),
                            ],
                            unknown_fields: UnknownFields::Reject,
                        }),
                        content_type: "application/json",
                        headers: BTreeMap::from([(ERROR_HEADER.into(), "application".into())]),
                    },
                )
            })
            .collect(),
    ))
}

fn validate_status(status: u16) -> Result<()> {
    if !(400..600).contains(&status) {
        return Err(Error::Binding(format!(
            "application error status must be 4xx or 5xx, got {status}"
        )));
    }
    Ok(())
}
