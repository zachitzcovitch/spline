use std::collections::BTreeMap;
use std::sync::Arc;

use reqwest::header;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use spline_contract::{
    http::{HttpMethod, HttpPlan, Scalar},
    ApplicationDescription, ContractDescription, Operation,
};
use spline_schema::{Graph, Shape};

use crate::{
    json_content_type, response, BuildError, CallError, Fault, ResponsePolicy, ERROR_HEADER,
};

const PATH_COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Clone, Debug)]
pub struct Client {
    base: url::Url,
    http: reqwest::Client,
    plans: Arc<BTreeMap<String, CallPlan>>,
    schemas: Arc<Graph>,
    response_policy: ResponsePolicy,
}

#[derive(Clone, Debug)]
struct CallPlan {
    http: HttpPlan,
    output: Shape,
    error: Shape,
}

impl Client {
    pub fn new(base: &str, contract: &ContractDescription) -> Result<Self, BuildError> {
        Self::with_http(base, contract, default_http()?)
    }

    pub fn with_http(
        base: &str,
        contract: &ContractDescription,
        http: reqwest::Client,
    ) -> Result<Self, BuildError> {
        Self::build(
            base,
            contract.schemas(),
            contract.operations().values(),
            http,
        )
    }

    pub fn for_application(
        base: &str,
        application: &ApplicationDescription,
    ) -> Result<Self, BuildError> {
        Self::for_application_with_http(base, application, default_http()?)
    }

    pub fn for_application_with_http(
        base: &str,
        application: &ApplicationDescription,
        http: reqwest::Client,
    ) -> Result<Self, BuildError> {
        Self::build(base, application.schemas(), application.operations(), http)
    }

    fn build<'a>(
        base: &str,
        schemas: &Graph,
        operations: impl Iterator<Item = &'a Operation>,
        http: reqwest::Client,
    ) -> Result<Self, BuildError> {
        let base =
            url::Url::parse(base).map_err(|error| BuildError::Configuration(error.to_string()))?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err(BuildError::Configuration(
                "base URL must be HTTP(S), without credentials, query, or fragment".into(),
            ));
        }
        let plans = operations
            .map(|operation| {
                operation
                    .http
                    .clone()
                    .map(|http| {
                        (
                            operation.id.clone(),
                            CallPlan {
                                http,
                                output: operation.output.clone(),
                                error: operation.error.clone(),
                            },
                        )
                    })
                    .ok_or_else(|| {
                        BuildError::Configuration(format!("{} has no HTTP binding", operation.id))
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            base,
            http,
            plans: Arc::new(plans),
            schemas: Arc::new(schemas.clone()),
            response_policy: ResponsePolicy::default(),
        })
    }

    pub fn with_response_policy(mut self, policy: ResponsePolicy) -> Self {
        self.response_policy = policy;
        self
    }

    pub async fn call<I: Serialize, O: DeserializeOwned, E: DeserializeOwned>(
        &self,
        operation: &str,
        input: I,
    ) -> Result<O, CallError<E>> {
        let call = self
            .plans
            .get(operation)
            .ok_or_else(|| CallError::Protocol("unknown operation".into()))?;
        let plan = &call.http;
        let value =
            serde_json::to_value(input).map_err(|error| CallError::Encode(error.to_string()))?;
        let values = value
            .as_object()
            .ok_or_else(|| CallError::Encode("expected a named input object".into()))?;
        let mut path = plan.binding.path.clone();
        for parameter in &plan.path_parameters {
            let value = values
                .get(&parameter.name)
                .ok_or_else(|| CallError::Encode("missing path argument".into()))?;
            let text = scalar_text(value, parameter.scalar).map_err(CallError::Encode)?;
            if text.is_empty() || matches!(text.as_str(), "." | "..") {
                return Err(CallError::Encode(
                    "path arguments cannot be empty or dot segments".into(),
                ));
            }
            let encoded = percent_encoding::utf8_percent_encode(&text, PATH_COMPONENT).to_string();
            path = path.replace(&format!("{{{}}}", parameter.name), &encoded);
        }
        let mut url = self.base.clone();
        url.set_path(&format!(
            "{}{}",
            self.base.path().trim_end_matches('/'),
            path
        ));
        for parameter in &plan.query_parameters {
            let value = values.get(&parameter.name).unwrap_or(&Value::Null);
            if value.is_null() && parameter.optional {
                continue;
            }
            let text = scalar_text(value, parameter.scalar).map_err(CallError::Encode)?;
            url.query_pairs_mut().append_pair(&parameter.name, &text);
        }
        let mut request = match plan.binding.method {
            HttpMethod::Get => self.http.get(url),
            HttpMethod::Post => self.http.post(url),
            HttpMethod::Put => self.http.put(url),
            HttpMethod::Patch => self.http.patch(url),
            HttpMethod::Delete => self.http.delete(url),
        }
        .header(header::ACCEPT, "application/json");
        if let Some(body) = &plan.binding.body {
            let value = values
                .get(body)
                .ok_or_else(|| CallError::Encode("missing body argument".into()))?;
            request = request.json(value);
        }
        let response = request.send().await.map_err(CallError::Transport)?;
        let status = response.status().as_u16();
        let error_kind = response
            .headers()
            .get(ERROR_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let json = json_content_type(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        );
        if status == plan.binding.success_status {
            if status == 204 {
                if response.headers().contains_key(ERROR_HEADER)
                    || response.headers().contains_key(header::CONTENT_LENGTH)
                    || response.headers().contains_key(header::TRANSFER_ENCODING)
                {
                    return Err(CallError::Protocol("invalid 204 response headers".into()));
                }
                let bytes = response.bytes().await.map_err(CallError::Transport)?;
                if !bytes.is_empty() {
                    return Err(CallError::Protocol(
                        "204 response must have an empty body".into(),
                    ));
                }
                // The generated contract requires unit. Decode its unit value
                // locally; there is deliberately no JSON body to parse.
                return serde_json::from_value(Value::Null)
                    .map_err(|error| CallError::Protocol(error.to_string()));
            }
            if !json || response.headers().contains_key(ERROR_HEADER) {
                return Err(CallError::Protocol(
                    "invalid success response headers".into(),
                ));
            }
            let bytes = response.bytes().await.map_err(CallError::Transport)?;
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| CallError::Protocol(error.to_string()))?;
            let value =
                response::project(&value, &call.output, &self.schemas, self.response_policy)
                    .map_err(CallError::Protocol)?;
            return serde_json::from_value(value)
                .map_err(|error| CallError::Protocol(error.to_string()));
        }
        if status < 400 {
            return Err(CallError::Protocol(format!(
                "unexpected success/redirect status {status}"
            )));
        }
        if !json || error_kind.is_none() {
            return Err(CallError::Http { status });
        }
        let bytes = response.bytes().await.map_err(CallError::Transport)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| CallError::Protocol(error.to_string()))?;
        match (
            error_kind.as_deref(),
            value.get("kind").and_then(Value::as_str),
        ) {
            (Some("application"), Some("application")) => {
                let value = value
                    .get("value")
                    .ok_or_else(|| CallError::Protocol("application error has no value".into()))?;
                let value =
                    response::project(value, &call.error, &self.schemas, self.response_policy)
                        .map_err(CallError::Protocol)?;
                let error = serde_json::from_value(value)
                    .map_err(|error| CallError::Protocol(error.to_string()))?;
                Err(CallError::Application(error))
            }
            (Some("framework"), Some("framework")) => {
                let code = value.get("code").and_then(Value::as_str);
                let message = value.get("message").and_then(Value::as_str);
                match (code, message) {
                    (Some(code), Some(message)) => {
                        Err(CallError::Framework(Fault::new(status, code, message)))
                    }
                    _ => Err(CallError::Protocol("malformed framework error".into())),
                }
            }
            _ => Err(CallError::Protocol("unrecognized error envelope".into())),
        }
    }
}

fn default_http() -> Result<reqwest::Client, BuildError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| BuildError::Configuration(error.to_string()))
}

fn scalar_text(value: &Value, scalar: Scalar) -> Result<String, String> {
    match scalar {
        Scalar::String => value.as_str().map(str::to_owned),
        Scalar::Boolean => value.as_bool().map(|v| v.to_string()),
        Scalar::Integer { signed: true, bits } => value
            .as_i64()
            .filter(|v| {
                let bound = 1_i64 << (bits - 1);
                (-bound..bound).contains(v)
            })
            .map(|v| v.to_string()),
        Scalar::Integer {
            signed: false,
            bits,
        } => value
            .as_u64()
            .filter(|v| *v < 1_u64 << bits)
            .map(|v| v.to_string()),
    }
    .ok_or_else(|| "URL argument does not match its declared scalar type".into())
}
