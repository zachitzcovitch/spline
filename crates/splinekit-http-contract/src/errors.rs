//! Error responses on the wire: RFC 9457 problem details, machine-first.
//!
//! Every error response is `application/problem+json` with a numeric `status`
//! equal to the HTTP status and a string `code`. Clients branch on `code` and
//! the fields beside it; nothing in an error is text meant for end users.
//!
//! - An application error is the application's own error value, an object
//!   tagged by `code` (`#[serde(tag = "code")]`), with `status` added.
//! - A framework fault (invalid input, a rejected request, a failed service)
//!   is `status`, `code` and an optional developer-facing `detail`.
//!
//! No header marks either kind. A client recognizes an application error by
//! decoding it as one of the operation's declared errors at that status;
//! application errors may not declare the binding's own fault codes or use
//! RFC 9457's member names as fields, so the two can't be confused.
//!
//! A contract can declare where its problem types live
//! (`#[http(problem_types = "https://example.com/errors")]` on the trait).
//! Then every error also carries RFC 9457's `type`: the base, a slash, and
//! the code with underscores as hyphens ([`problem_type`]). Clients accept
//! only problem details whose `type` matches their `code`, so a proxy's or
//! gateway's JSON can't pass as the service's own answer.

use std::collections::{BTreeMap, BTreeSet};

use splinekit_schema::{Field, Graph, Object, Presence, Shape, UnknownFields};

use crate::{Error, Result};

pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// A problem's RFC 9457 `type` under `base`: `{base}/{code}`, with the code's
/// underscores as hyphens (`invalid_request` is `{base}/invalid-request`).
pub fn problem_type(base: &str, code: &str) -> String {
    format!("{base}/{}", code.replace('_', "-"))
}

/// A problem-type base must be an absolute URI a code can be appended to: a
/// scheme, no whitespace, query, fragment or trailing slash.
pub fn check_problem_types(base: &str) -> Result<()> {
    let scheme = base.split_once(':').map(|(scheme, _)| scheme);
    let valid_scheme = scheme.is_some_and(|scheme| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if !valid_scheme
        || base.ends_with('/')
        || base.contains(|c: char| c.is_whitespace() || matches!(c, '?' | '#'))
    {
        return Err(Error::Binding(format!(
            "problem types {base:?} must be an absolute URI without a query, fragment or trailing slash"
        )));
    }
    Ok(())
}
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// RFC 9457's members. Application error fields can't use these names.
pub const PROBLEM_MEMBERS: [&str; 5] = ["type", "title", "status", "detail", "instance"];

/// The codes of the faults the binding itself sends. Application errors
/// can't declare them.
pub const FRAMEWORK_CODES: [&str; 5] = [
    "invalid_request",
    "body_too_large",
    "unsupported_media_type",
    "internal",
    "service_failure",
];

fn status_field() -> Field {
    Field::required(
        "status",
        Shape::Integer {
            signed: false,
            bits: 16,
        },
    )
}

/// RFC 9457's `type`, sent when the contract declares its problem types.
fn type_field() -> Field {
    Field {
        presence: Presence::Optional,
        ..Field::required("type", Shape::String)
    }
}

/// A framework fault's body. Other members are ignored, so intermediaries and
/// later versions can add RFC 9457 members.
pub fn framework_error_schema() -> Shape {
    Shape::Object(Object {
        fields: vec![
            type_field(),
            status_field(),
            Field::required("code", Shape::String),
            Field {
                presence: Presence::Optional,
                ..Field::required("detail", Shape::String)
            },
        ],
        unknown_fields: UnknownFields::Ignore,
    })
}

/// An application error's body: each `code`-tagged alternative of `error`
/// with `status` added. `error` must have passed [`application_error_codes`].
pub fn application_error_schema(error: &Shape, graph: &Graph) -> Result<Shape> {
    Ok(match resolve(error, graph)? {
        Shape::Union(alternatives) => Shape::Union(
            alternatives
                .iter()
                .map(|alternative| application_error_schema(alternative, graph))
                .collect::<Result<_>>()?,
        ),
        Shape::Object(object) => {
            let mut fields = vec![type_field(), status_field()];
            fields.extend(object.fields.iter().cloned());
            Shape::Object(Object {
                fields,
                unknown_fields: object.unknown_fields,
            })
        }
        _ => return Err(not_tagged()),
    })
}

/// The `code`s an operation's error type declares, checking that it has the
/// wire form above: objects (or a union of objects) with a required literal
/// `code` that isn't a framework code, and no field named like an RFC 9457
/// member.
pub fn application_error_codes(error: &Shape, graph: &Graph) -> Result<BTreeSet<String>> {
    let mut codes = BTreeSet::new();
    validate(error, graph, &mut codes)?;
    Ok(codes)
}

fn validate(shape: &Shape, graph: &Graph, codes: &mut BTreeSet<String>) -> Result<()> {
    match resolve(shape, graph)? {
        Shape::Union(alternatives) => alternatives
            .iter()
            .try_for_each(|alternative| validate(alternative, graph, codes)),
        Shape::Object(object) => {
            let code = object
                .fields
                .iter()
                .find(|field| field.name == "code" && field.presence == Presence::Required)
                .map(|field| resolve(&field.shape, graph))
                .transpose()?;
            let Some(Shape::Literal(code)) = code else {
                return Err(not_tagged());
            };
            if FRAMEWORK_CODES.contains(&code.as_str()) {
                return Err(Error::Binding(format!(
                    "application error code `{code}` is reserved for framework faults"
                )));
            }
            if !codes.insert(code.clone()) {
                return Err(Error::Binding(format!(
                    "application error code `{code}` is declared twice"
                )));
            }
            if let Some(field) = object
                .fields
                .iter()
                .find(|field| PROBLEM_MEMBERS.contains(&field.name.as_str()))
            {
                return Err(Error::Binding(format!(
                    "application error `{code}` has a field named `{}`, which problem details reserve",
                    field.name
                )));
            }
            Ok(())
        }
        _ => Err(not_tagged()),
    }
}

fn not_tagged() -> Error {
    Error::Binding(
        "HTTP application errors must serialize as objects tagged by `code` (#[serde(tag = \"code\")])"
            .into(),
    )
}

fn resolve<'a>(shape: &'a Shape, graph: &'a Graph) -> Result<&'a Shape> {
    crate::plan::resolve(shape, graph)
}

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
    /// The application errors declared at this status, as the client decodes
    /// them (problem members removed).
    pub error: Shape,
    /// The complete problem body: `error` with `status` added.
    pub body: Shape,
    pub content_type: &'static str,
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
    let mut declared = BTreeMap::new();
    for (status, mut shapes) in grouped {
        let error = if shapes.len() == 1 {
            shapes.pop().unwrap()
        } else {
            Shape::Union(shapes)
        };
        let body = application_error_schema(&error, graph)?;
        declared.insert(
            status,
            ErrorResponse {
                error,
                body,
                content_type: PROBLEM_CONTENT_TYPE,
            },
        );
    }
    Ok(ErrorResponses::Declared(declared))
}

fn validate_status(status: u16) -> Result<()> {
    if !(400..600).contains(&status) {
        return Err(Error::Binding(format!(
            "application error status must be 4xx or 5xx, got {status}"
        )));
    }
    Ok(())
}
