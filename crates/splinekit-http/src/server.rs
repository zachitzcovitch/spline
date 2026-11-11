use std::collections::{BTreeMap, BTreeSet};

use axum::body::{to_bytes, Body};
use axum::extract::{FromRequestParts, Path, Request};
use axum::http::{header, Extensions, HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};
use splinekit_http_contract::{HttpPlan, Scalar};

use crate::{json_content_type, ErrorPolicy, Fault, MAX_BODY_BYTES, PROBLEM_CONTENT_TYPE};
use splinekit_http_contract::PROBLEM_MEMBERS;

/// Request-owned context, suitable for an asynchronous per-request service factory.
#[derive(Clone, Debug)]
pub struct RequestContext {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub extensions: Extensions,
}

impl RequestContext {
    pub fn from_request(request: &Request) -> Self {
        Self {
            method: request.method().clone(),
            uri: request.uri().clone(),
            headers: request.headers().clone(),
            extensions: request.extensions().clone(),
        }
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status)
            .ok()
            .filter(|status| status.is_client_error() || status.is_server_error());
        let Some(status) = status else {
            return Fault::internal().into_response();
        };
        let mut body = Map::new();
        body.insert("status".into(), status.as_u16().into());
        body.insert("code".into(), self.code.into());
        if let Some(detail) = self.detail {
            body.insert("detail".into(), detail.into());
        }
        problem_response(status, serde_json::to_vec(&body).unwrap())
    }
}

pub fn respond<T: Serialize, E: Serialize, P: ErrorPolicy<E>>(
    result: Result<T, E>,
    success: u16,
    policy: &P,
) -> Response {
    match result {
        Ok(value) if success == 204 => match serde_json::to_value(value) {
            Ok(Value::Null) => {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = StatusCode::NO_CONTENT;
                response
            }
            _ => Fault::internal().into_response(),
        },
        Ok(value) if matches!(success, 200 | 201) => match serde_json::to_vec(&value) {
            Ok(body) => json_response(StatusCode::from_u16(success).unwrap(), body),
            Err(_) => Fault::internal().into_response(),
        },
        Ok(_) => Fault::internal().into_response(),
        Err(error) => {
            let status = policy.status(&error);
            if !status.is_client_error() && !status.is_server_error() {
                return Fault::internal().into_response();
            }
            // The binding checked the error type: an object tagged by `code`
            // with no problem-detail members of its own. Add the status.
            let Ok(Value::Object(fields)) = serde_json::to_value(error) else {
                return Fault::internal().into_response();
            };
            if !fields.get("code").is_some_and(Value::is_string)
                || PROBLEM_MEMBERS
                    .iter()
                    .any(|member| fields.contains_key(*member))
            {
                return Fault::internal().into_response();
            }
            let mut body = Map::new();
            body.insert("status".into(), status.as_u16().into());
            body.extend(fields);
            match serde_json::to_vec(&body) {
                Ok(body) => problem_response(status, body),
                Err(_) => Fault::internal().into_response(),
            }
        }
    }
}

/// Translate a neutral Tower invocation into this binding's response envelopes.
pub fn respond_call<T: Serialize, E: Serialize, P: ErrorPolicy<E>>(
    result: Result<T, splinekit_runtime::CallError<E>>,
    success: u16,
    policy: &P,
) -> Response {
    match result {
        Ok(value) => respond(Ok::<_, E>(value), success, policy),
        Err(splinekit_runtime::CallError::Application(error)) => {
            respond(Err::<T, _>(error), success, policy)
        }
        Err(splinekit_runtime::CallError::Service(_)) => {
            Fault::new(500, "service_failure", "operation service failed").into_response()
        }
    }
}

fn json_response(status: StatusCode, body: Vec<u8>) -> Response {
    typed_response(status, body, "application/json")
}

fn problem_response(status: StatusCode, body: Vec<u8>) -> Response {
    typed_response(status, body, PROBLEM_CONTENT_TYPE)
}

fn typed_response(status: StatusCode, body: Vec<u8>, content_type: &'static str) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type),
    );
    response
}

pub async fn decode<I: DeserializeOwned>(request: Request, plan: &HttpPlan) -> Result<I, Fault> {
    let (mut parts, body) = request.into_parts();
    let mut values = Map::new();
    if !plan.path_parameters.is_empty() {
        let Path(parameters) =
            Path::<BTreeMap<String, String>>::from_request_parts(&mut parts, &())
                .await
                .map_err(|error| Fault::invalid(error.to_string()))?;
        for parameter in &plan.path_parameters {
            let value = parameters
                .get(&parameter.name)
                .ok_or_else(|| Fault::invalid("missing path parameter"))?;
            values.insert(
                parameter.name.clone(),
                parse_scalar(value, parameter.scalar)?,
            );
        }
    }
    let mut seen = BTreeSet::new();
    for (name, value) in url::form_urlencoded::parse(parts.uri.query().unwrap_or("").as_bytes()) {
        if !seen.insert(name.to_string()) {
            return Err(Fault::invalid(format!("duplicate query argument {name}")));
        }
        let parameter = plan
            .query_parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .ok_or_else(|| Fault::invalid(format!("unknown query argument {name}")))?;
        values.insert(
            parameter.name.clone(),
            parse_scalar(&value, parameter.scalar)?,
        );
    }
    for parameter in &plan.query_parameters {
        if !parameter.optional && !values.contains_key(&parameter.name) {
            return Err(Fault::invalid(format!(
                "missing query argument {}",
                parameter.name
            )));
        }
    }
    let bytes = to_bytes(body, MAX_BODY_BYTES).await.map_err(|_| {
        Fault::new(
            413,
            "body_too_large",
            "request body exceeds the binding limit",
        )
    })?;
    if let Some(argument) = &plan.binding.body {
        if !json_content_type(
            parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        ) {
            return Err(Fault::new(
                415,
                "unsupported_media_type",
                "expected application/json",
            ));
        }
        let body =
            serde_json::from_slice(&bytes).map_err(|error| Fault::invalid(error.to_string()))?;
        values.insert(argument.clone(), body);
    } else if !bytes.is_empty() {
        return Err(Fault::invalid(
            "this operation does not accept a request body",
        ));
    }
    serde_json::from_value(Value::Object(values)).map_err(|error| Fault::invalid(error.to_string()))
}

fn parse_scalar(value: &str, scalar: Scalar) -> Result<Value, Fault> {
    match scalar {
        Scalar::String => Ok(Value::String(value.into())),
        Scalar::Boolean => match value {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(Fault::invalid("expected true or false")),
        },
        Scalar::Integer { signed, bits } => {
            let number: i64 = value.parse().map_err(|_| {
                Fault::invalid(if signed {
                    "expected an integer"
                } else {
                    "expected an unsigned integer"
                })
            })?;
            let (min, max) = splinekit_schema::integer_bounds(signed, bits)
                .ok_or_else(|| Fault::invalid("unsupported integer width"))?;
            if !(min..=max).contains(&number) {
                return Err(Fault::invalid("integer outside its declared range"));
            }
            Ok(number.into())
        }
    }
}
