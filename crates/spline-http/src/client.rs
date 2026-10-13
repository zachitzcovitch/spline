use std::collections::BTreeMap;

use reqwest::header;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use spline_contract::{
    http::{HttpMethod, HttpPlan, Scalar},
    ContractDescription,
};

use crate::{json_content_type, BuildError, CallError, Fault, ERROR_HEADER};

const PATH_COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Clone, Debug)]
pub struct Client {
    base: url::Url,
    http: reqwest::Client,
    plans: BTreeMap<String, HttpPlan>,
}

impl Client {
    pub fn new(base: &str, contract: &ContractDescription) -> Result<Self, BuildError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| BuildError::Configuration(error.to_string()))?;
        Self::with_http(base, contract, client)
    }

    pub fn with_http(
        base: &str,
        contract: &ContractDescription,
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
        let plans = contract
            .operations()
            .values()
            .map(|operation| {
                operation
                    .http
                    .clone()
                    .map(|plan| (operation.id.clone(), plan))
                    .ok_or_else(|| {
                        BuildError::Configuration(format!("{} has no HTTP binding", operation.id))
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { base, http, plans })
    }

    pub async fn call<I: Serialize, O: DeserializeOwned, E: DeserializeOwned>(
        &self,
        operation: &str,
        input: I,
    ) -> Result<O, CallError<E>> {
        let plan = self
            .plans
            .get(operation)
            .ok_or_else(|| CallError::Protocol("unknown operation".into()))?;
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
            if !json || error_kind.is_some() {
                return Err(CallError::Protocol(
                    "invalid success response headers".into(),
                ));
            }
            let bytes = response.bytes().await.map_err(CallError::Transport)?;
            return serde_json::from_slice(&bytes)
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
                let error = serde_json::from_value(value.clone())
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
