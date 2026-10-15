use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use spline_contract::Operation;
use spline_http_contract::{
    application_error_schema, ErrorResponses, HttpMethod, HttpPlan, ERROR_HEADER, MAX_BODY_BYTES,
};

use crate::{schema::Schemas, Result};

pub(crate) const FRAMEWORK_SCHEMA: &str = "ServiceError";
fn framework_ref() -> Value {
    json!({"$ref":format!("#/components/schemas/{FRAMEWORK_SCHEMA}")})
}

pub(crate) fn framework_response() -> Value {
    error_response(
        "Request or service failure.",
        framework_ref(),
        BTreeMap::new(),
    )
}

pub(crate) fn method(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
    }
}

pub(crate) fn render(
    op: &Operation,
    plan: &HttpPlan,
    group: &str,
    schemas: &Schemas<'_>,
) -> Result<Value> {
    let mut operation = json!({"operationId":op.id,"tags":[group]});
    if let Some(summary) = op
        .documentation
        .lines()
        .next()
        .filter(|line| !line.trim().is_empty())
    {
        operation["summary"] = summary.into();
    }
    let wire = format!(
        "Query parameters use URL form encoding. Duplicate or unknown query parameters are rejected. Request bodies are limited to {MAX_BODY_BYTES} bytes."
    );
    operation["description"] = if op.documentation.is_empty() {
        wire
    } else {
        format!("{}\n\n{wire}", op.documentation)
    }
    .into();
    let mut parameters = Vec::new();
    for (location, values) in [
        ("path", &plan.path_parameters),
        ("query", &plan.query_parameters),
    ] {
        for parameter in values {
            let argument = op
                .arguments
                .iter()
                .find(|a| a.name == parameter.name)
                .expect("validated HTTP argument");
            let schema = if location == "query" && parameter.optional {
                schemas
                    .present(&argument.shape)?
                    .expect("validated scalar query")
            } else {
                schemas.render(&argument.shape)?
            };
            let mut value = json!({"name":parameter.name,"in":location,"required":location=="path"||!parameter.optional,
                "schema":schema,"style":if location=="path"{"simple"}else{"form"},"explode":location=="query"});
            let mut docs = schemas.argument_documentation(op, &parameter.name);
            if location == "query" && parameter.optional {
                if !docs.is_empty() {
                    docs.push_str("\n\n");
                }
                docs.push_str("Omit this optional parameter when no value is supplied.");
            }
            if !docs.is_empty() {
                value["description"] = docs.into();
            }
            parameters.push(value);
        }
    }
    if !parameters.is_empty() {
        operation["parameters"] = json!(parameters);
    }
    if let Some(body) = &plan.binding.body {
        let argument = op
            .arguments
            .iter()
            .find(|a| &a.name == body)
            .expect("validated body argument");
        let mut description = schemas.argument_documentation(op, body);
        if !description.is_empty() {
            description.push_str("\n\n");
        }
        description.push_str("A JSON body is required, even when its value may be null. application/json and application subtypes ending in +json are accepted, including content-type parameters.");
        operation["requestBody"] = json!({"required":true,"description":description,
            "content":{"application/json":{"schema":schemas.render(&argument.shape)?}}});
    }
    let mut responses = Map::new();
    let success = if plan.binding.success_status == 204 {
        json!({"description":"Success with no response body."})
    } else {
        json!({"description":"Success.","content":{"application/json":{"schema":schemas.render(&op.output)?}}})
    };
    responses.insert(plan.binding.success_status.to_string(), success);
    let fallback = match &plan.errors {
        ErrorResponses::Opaque => {
            error_response("Application error or request/service failure. See the response body for error details.",
                json!({"anyOf":[schemas.render(&application_error_schema(op.error.clone()))?,framework_ref()]}),
                BTreeMap::new())
        }
        ErrorResponses::Declared(_) => {
            json!({"$ref":"#/components/responses/ServiceErrorResponse"})
        }
    };
    for status in [400, 413, 500]
        .into_iter()
        .chain(plan.binding.body.is_some().then_some(415))
    {
        responses.insert(status.to_string(), fallback.clone());
    }
    responses.insert("4XX".into(), fallback.clone());
    responses.insert("5XX".into(), fallback);
    if let ErrorResponses::Declared(declared) = &plan.errors {
        for (status, response) in declared {
            let mut headers = BTreeMap::new();
            for (name, value) in &response.headers {
                if !name.eq_ignore_ascii_case(ERROR_HEADER) {
                    headers.insert(name.clone(),json!({"description":"Application-error response header.","schema":{"type":"string","const":value}}));
                }
            }
            responses.insert(
                status.to_string(),
                error_response(
                    "Application error or request/service failure.",
                    json!({"anyOf":[schemas.render(&response.body)?,framework_ref()]}),
                    headers,
                ),
            );
        }
    }
    operation["responses"] = Value::Object(responses);
    Ok(operation)
}

fn error_response(description: &str, body: Value, headers: BTreeMap<String, Value>) -> Value {
    let mut response =
        json!({"description":description,"content":{"application/json":{"schema":body}}});
    if !headers.is_empty() {
        response["headers"] = json!(headers);
    }
    response
}
