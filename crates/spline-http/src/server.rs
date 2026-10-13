use std::collections::{BTreeMap, BTreeSet};

use axum::body::{to_bytes, Body};
use axum::extract::{FromRequestParts, Path, Request};
use axum::http::{header, Extensions, HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};
use spline_contract::http::{HttpPlan, Scalar};

use crate::{json_content_type, ERROR_HEADER, MAX_BODY_BYTES};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl Fault {
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(400, "invalid_request", message)
    }
    fn internal() -> Self {
        Self::new(500, "internal", "response encoding or error policy failed")
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
        let body =
            serde_json::json!({ "kind": "framework", "code": self.code, "message": self.message });
        json_response(
            status,
            serde_json::to_vec(&body).unwrap(),
            Some("framework"),
        )
    }
}

pub trait ErrorPolicy<E>: Send + Sync + 'static {
    fn status(&self, error: &E) -> StatusCode;
}
impl<E, F> ErrorPolicy<E> for F
where
    F: Fn(&E) -> StatusCode + Send + Sync + 'static,
{
    fn status(&self, error: &E) -> StatusCode {
        self(error)
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
            Ok(body) => json_response(StatusCode::from_u16(success).unwrap(), body, None),
            Err(_) => Fault::internal().into_response(),
        },
        Ok(_) => Fault::internal().into_response(),
        Err(error) => {
            let status = policy.status(&error);
            if !status.is_client_error() && !status.is_server_error() {
                return Fault::internal().into_response();
            }
            #[derive(Serialize)]
            struct Envelope<E> {
                kind: &'static str,
                value: E,
            }
            match serde_json::to_vec(&Envelope {
                kind: "application",
                value: error,
            }) {
                Ok(body) => json_response(status, body, Some("application")),
                Err(_) => Fault::internal().into_response(),
            }
        }
    }
}

fn json_response(status: StatusCode, body: Vec<u8>, error: Option<&'static str>) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    if let Some(error) = error {
        response
            .headers_mut()
            .insert(ERROR_HEADER, header::HeaderValue::from_static(error));
    }
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
        Scalar::Integer { signed: true, bits } => {
            let number: i64 = value
                .parse()
                .map_err(|_| Fault::invalid("expected an integer"))?;
            let limit = 1_i64 << (bits - 1);
            if !(-limit..limit).contains(&number) {
                return Err(Fault::invalid("integer outside its declared range"));
            }
            Ok(number.into())
        }
        Scalar::Integer {
            signed: false,
            bits,
        } => {
            let number: u64 = value
                .parse()
                .map_err(|_| Fault::invalid("expected an unsigned integer"))?;
            if number >= 1_u64 << bits {
                return Err(Fault::invalid("integer outside its declared range"));
            }
            Ok(number.into())
        }
    }
}
