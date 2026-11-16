use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use bytes::Bytes;
use http::header;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use splinekit_contract::Operation;
use splinekit_http_contract::{
    application_error_codes, problem_type, ApplicationDescription, ContractDescription,
    ErrorResponses, HttpMethod, HttpPlan, Scalar, PROBLEM_MEMBERS,
};
use splinekit_schema::{Graph, Shape};

use crate::transport::{erase, Erased, ReqwestTransport, Transport};
use crate::{json_content_type, response, BuildError, CallError, Fault, ResponsePolicy};

const PATH_COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Clone)]
pub struct Client {
    base: url::Url,
    transport: Arc<dyn Erased>,
    plans: Arc<BTreeMap<String, CallPlan>>,
    schemas: Arc<Graph>,
    response_policy: ResponsePolicy,
    /// Where the service's problem types live, when its contract declares
    /// them: problem details count only when their `type` matches.
    problem_types: Option<Arc<str>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base.as_str())
            .field("operations", &self.plans.keys().collect::<Vec<_>>())
            .field("response_policy", &self.response_policy)
            .finish_non_exhaustive()
    }
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
        Self::with_transport(base, contract, ReqwestTransport::new(http))
    }

    /// Send through any [`Transport`]: a Tower service, typically
    /// [`ReqwestTransport`] wrapped in layers for authentication, retries or
    /// timeouts.
    pub fn with_transport<T: Transport>(
        base: &str,
        contract: &ContractDescription,
        transport: T,
    ) -> Result<Self, BuildError> {
        Self::build(
            base,
            contract.contract().schemas(),
            contract.contract().operations().values(),
            contract.plans(),
            contract.problem_types(),
            erase(transport),
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
        Self::for_application_with_transport(base, application, ReqwestTransport::new(http))
    }

    pub fn for_application_with_transport<T: Transport>(
        base: &str,
        application: &ApplicationDescription,
        transport: T,
    ) -> Result<Self, BuildError> {
        Self::build(
            base,
            application.application().schemas(),
            application.application().operations(),
            application.plans(),
            application.problem_types(),
            erase(transport),
        )
    }

    fn build<'a>(
        base: &str,
        schemas: &Graph,
        operations: impl Iterator<Item = &'a Operation>,
        bindings: &BTreeMap<String, HttpPlan>,
        problem_types: Option<&str>,
        transport: Arc<dyn Erased>,
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
                bindings
                    .get(&operation.id)
                    .map(|http| {
                        (
                            operation.id.clone(),
                            CallPlan {
                                http: http.clone(),
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
            transport,
            plans: Arc::new(plans),
            schemas: Arc::new(schemas.clone()),
            response_policy: ResponsePolicy::default(),
            problem_types: problem_types.map(Arc::from),
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
        let method = match plan.binding.method {
            HttpMethod::Get => http::Method::GET,
            HttpMethod::Post => http::Method::POST,
            HttpMethod::Put => http::Method::PUT,
            HttpMethod::Patch => http::Method::PATCH,
            HttpMethod::Delete => http::Method::DELETE,
        };
        let mut request = http::Request::builder()
            .method(method)
            .uri(url.as_str())
            .header(header::ACCEPT, "application/json");
        let body = match &plan.binding.body {
            Some(body) => {
                let value = values
                    .get(body)
                    .ok_or_else(|| CallError::Encode("missing body argument".into()))?;
                request = request.header(header::CONTENT_TYPE, "application/json");
                Bytes::from(
                    serde_json::to_vec(value)
                        .map_err(|error| CallError::Encode(error.to_string()))?,
                )
            }
            None => Bytes::new(),
        };
        let request = request
            .body(body)
            .map_err(|error| CallError::Encode(error.to_string()))?;
        let response = self
            .transport
            .send(request)
            .await
            .map_err(CallError::Transport)?;
        let status = response.status().as_u16();
        let json = json_content_type(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        );
        if status == plan.binding.success_status {
            if status == 204 {
                if response.headers().contains_key(header::CONTENT_LENGTH)
                    || response.headers().contains_key(header::TRANSFER_ENCODING)
                {
                    return Err(CallError::Protocol("invalid 204 response headers".into()));
                }
                if !response.body().is_empty() {
                    return Err(CallError::Protocol(
                        "204 response must have an empty body".into(),
                    ));
                }
                // The generated contract requires unit. Decode its unit value
                // locally; there is deliberately no JSON body to parse.
                return serde_json::from_value(Value::Null)
                    .map_err(|error| CallError::Protocol(error.to_string()));
            }
            if !json {
                return Err(CallError::Protocol(
                    "invalid success response headers".into(),
                ));
            }
            let value: Value = serde_json::from_slice(response.body())
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
        // Problem details carrying this status and a code (and the service's
        // own `type` for that code, when its contract declares problem types);
        // anything else (a proxy's page, a gateway's JSON) is an unrecognized
        // HTTP failure.
        let problem = json
            .then(|| serde_json::from_slice::<Value>(response.body()).ok())
            .flatten()
            .and_then(|value| match value {
                Value::Object(fields) => Some(fields),
                _ => None,
            })
            .filter(|fields| {
                let Some(code) = fields.get("code").and_then(Value::as_str) else {
                    return false;
                };
                fields.get("status").and_then(Value::as_u64) == Some(u64::from(status))
                    && self.problem_types.as_deref().is_none_or(|base| {
                        fields.get("type").and_then(Value::as_str)
                            == Some(problem_type(base, code).as_str())
                    })
            });
        let Some(mut problem) = problem else {
            return Err(CallError::Http { status });
        };
        let code = problem["code"].as_str().unwrap_or_default().to_owned();
        // A code the operation declares at this status is an application
        // error and must decode as one; any other code is a framework fault.
        let declared = match &plan.errors {
            ErrorResponses::Opaque => Some(&call.error),
            ErrorResponses::Declared(responses) => {
                responses.get(&status).map(|response| &response.error)
            }
        };
        let declares = declared
            .map(|shape| application_error_codes(shape, &self.schemas))
            .transpose()
            .map_err(|error| CallError::Protocol(error.to_string()))?
            .is_some_and(|codes| codes.contains(&code));
        if let (true, Some(shape)) = (declares, declared) {
            for member in PROBLEM_MEMBERS {
                problem.remove(member);
            }
            let value = response::project(
                &Value::Object(problem),
                shape,
                &self.schemas,
                self.response_policy,
            )
            .map_err(CallError::Protocol)?;
            let error = serde_json::from_value(value)
                .map_err(|error| CallError::Protocol(error.to_string()))?;
            return Err(CallError::Application(error));
        }
        Err(CallError::Framework(Fault {
            status,
            code,
            detail: problem
                .get("detail")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }))
    }
}

fn default_http() -> Result<reqwest::Client, BuildError> {
    let builder = reqwest::Client::builder();
    // Redirects are surfaced, not followed. Browsers decide that for fetch,
    // and reqwest's wasm32 client has no redirect policy.
    #[cfg(not(target_arch = "wasm32"))]
    let builder = builder.redirect(reqwest::redirect::Policy::none());
    builder
        .build()
        .map_err(|error| BuildError::Configuration(error.to_string()))
}

fn scalar_text(value: &Value, scalar: Scalar) -> Result<String, String> {
    match scalar {
        Scalar::String => value.as_str().map(str::to_owned),
        Scalar::Boolean => value.as_bool().map(|v| v.to_string()),
        Scalar::Integer { signed, bits } => value
            .as_i64()
            .filter(|v| {
                splinekit_schema::integer_bounds(signed, bits)
                    .is_some_and(|(min, max)| (min..=max).contains(v))
            })
            .map(|v| v.to_string()),
    }
    .ok_or_else(|| "URL argument does not match its declared scalar type".into())
}
