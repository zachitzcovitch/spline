#![cfg(feature = "http")]

mod support;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use splinekit::http::{
    axum,
    contract::{ErrorMapping, ErrorResponses},
    reqwest, CallError, DeclaredErrors, ErrorPolicy, HttpExt, StatusCode,
};
use splinekit::{Direction, Schema, Shape};
use support::{body, field, graph, matches};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Details {
    message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Schema, splinekit::http::Error)]
#[serde(
    tag = "code",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ApiError<T> {
    #[http(status = StatusCode::NOT_FOUND)]
    #[serde(rename = "absent")]
    NotFound,
    #[http(status = StatusCode::UNPROCESSABLE_ENTITY)]
    Invalid {
        #[serde(rename = "fieldName")]
        field_name: String,
    },
    #[http(status = StatusCode::UNPROCESSABLE_ENTITY)]
    Rejected(T),
    #[http(status = StatusCode::FORBIDDEN)]
    Forbidden,
}

pub type AppError = ApiError<Details>;

#[derive(Debug, Serialize, Deserialize, Schema, splinekit::http::Error)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Problem {
    #[http(status = StatusCode::CONFLICT)]
    Unavailable { message: String },
}

pub mod items {
    use super::AppError;

    #[splinekit::contract]
    pub trait Items {
        #[query]
        #[http(get = "/items/{kind}")]
        async fn get(&self, kind: u8) -> Result<(), AppError>;
    }
}

pub mod health {
    use super::Problem;

    #[splinekit::contract]
    pub trait Health {
        #[query]
        #[http(get = "/health")]
        async fn get(&self) -> Result<(), Problem>;
    }
}

splinekit::api! {
    pub mod api {
        items: crate::items,
        health: crate::health,
    }
}

struct Backend;
impl items::Items for Backend {
    async fn get(&self, kind: u8) -> Result<(), AppError> {
        match kind {
            0 => Err(ApiError::NotFound),
            1 => Err(ApiError::Invalid {
                field_name: "title".into(),
            }),
            2 => Err(ApiError::Rejected(Details {
                message: "invalid".into(),
            })),
            3 => Err(ApiError::Forbidden),
            _ => Ok(()),
        }
    }
}
impl health::Health for Backend {
    async fn get(&self) -> Result<(), Problem> {
        Err(Problem::Unavailable {
            message: "unavailable".into(),
        })
    }
}

struct Running {
    base: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn serve(router: axum::Router) -> Running {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Running { base, task }
}

fn declared(
    errors: &ErrorResponses,
) -> &std::collections::BTreeMap<u16, splinekit::http::contract::ErrorResponse> {
    let ErrorResponses::Declared(responses) = errors else {
        panic!("expected declared errors")
    };
    responses
}

#[tokio::test]
async fn derived_statuses_and_serialized_metadata_agree_with_real_http_and_typed_clients() {
    let description = api::http::describe_with(&DeclaredErrors).unwrap();
    let responses = declared(&description.plans()["items.get"].errors);
    assert_eq!(
        responses.keys().copied().collect::<Vec<_>>(),
        [403, 404, 422]
    );
    assert_eq!(declared(&description.plans()["health.get"].errors).len(), 1);
    let server = serve(api::shared(Backend).http(DeclaredErrors).unwrap()).await;
    let native = api::http::Client::new(&server.base).unwrap();
    let http = reqwest::Client::new();
    for (kind, status, payload) in [
        (0, 404, json!({"code": "absent"})),
        (
            1,
            422,
            json!({"code": "invalid", "payload": {"fieldName": "title"}}),
        ),
        (
            2,
            422,
            json!({"code": "rejected", "payload": {"message": "invalid"}}),
        ),
        (3, 403, json!({"code": "forbidden"})),
    ] {
        let response = http
            .get(format!("{}/items/{kind}", server.base))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
        // Nothing on the wire names the framework.
        assert!(response
            .headers()
            .keys()
            .all(|name| !name.as_str().contains("splinekit")));
        let actual: Value = response.json().await.unwrap();
        let mut expected = payload.clone();
        expected["status"] = json!(status);
        assert_eq!(actual, expected);
        let documented = &responses[&status];
        assert_eq!(documented.content_type, "application/problem+json");
        assert!(matches(
            description.application().schemas(),
            &documented.body,
            &actual
        ));
        for (other_status, other) in responses {
            assert_eq!(
                matches(description.application().schemas(), &other.body, &actual),
                *other_status == status
            );
        }
        let Err(CallError::Application(error)) = native.items().get(kind).await else {
            panic!("typed error")
        };
        assert_eq!(serde_json::to_value(error).unwrap(), payload);
    }
    let Err(CallError::Application(Problem::Unavailable { message })) = native.health().get().await
    else {
        panic!("typed error")
    };
    assert_eq!(message, "unavailable");
    let response = http
        .get(format!("{}/health", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    assert!(matches(
        description.application().schemas(),
        &declared(&description.plans()["health.get"].errors)[&409].body,
        &response.json::<Value>().await.unwrap()
    ));
    native.items().get(4).await.unwrap();
    // The configured-operation entry point validates and uses the same declarations.
    let _router = items::http::get::router(
        items::get::Service::new(std::sync::Arc::new(Backend)),
        DeclaredErrors,
    )
    .unwrap();
}

#[test]
fn response_groups_are_deterministic_and_match_the_selected_serialization_direction() {
    let description = items::http::describe_with(&DeclaredErrors).unwrap();
    let errors = &description.plans()["items.get"].errors;
    let mut reversed = <DeclaredErrors as ErrorPolicy<AppError>>::mapping(&DeclaredErrors);
    if let ErrorMapping::Variants(rules) = &mut reversed {
        rules.reverse();
    }
    let mut other = items::http::describe().unwrap();
    assert_eq!(other.plans()["items.get"].errors, ErrorResponses::Opaque);
    other.errors("items.get", reversed).unwrap();
    assert_eq!(&other.plans()["items.get"].errors, errors);
    let (root, schemas) = graph::<AppError>(Direction::Serialize);
    let Shape::Union(variants) = body(&root, &schemas) else {
        panic!("enum")
    };
    assert_eq!(
        field(&variants[0], "code").shape,
        Shape::Literal("absent".into())
    );
    let Shape::Union(alternatives) = &declared(errors)[&422].body else {
        panic!("shared status")
    };
    assert_eq!(alternatives.len(), 2);
    assert!(alternatives
        .iter()
        .all(|alternative| matches!(field(alternative, "status").shape, Shape::Integer { .. })));
    // The wire code is the serialized name, not the Rust variant name.
    assert!(!matches(
        description.contract().schemas(),
        &declared(errors)[&404].body,
        &json!({"status": 404, "code": "not_found"})
    ));
    assert!(matches(
        description.contract().schemas(),
        &declared(errors)[&404].body,
        &json!({"status": 404, "code": "absent"})
    ));
}

struct Rules(Vec<(String, u16)>);
impl ErrorPolicy<AppError> for Rules {
    fn status(&self, _: &AppError) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
    fn mapping(&self) -> ErrorMapping {
        ErrorMapping::Variants(self.0.clone())
    }
}

#[test]
fn incomplete_duplicate_unknown_and_invalid_mappings_fail_transactionally_before_routing() {
    let mut description = items::http::describe_with(&DeclaredErrors).unwrap();
    let before = description.clone();
    for (rules, message) in [
        (vec![], "missing status"),
        (
            vec![("NoSuchVariant".into(), 404)],
            "unknown serialized error variant",
        ),
        (
            vec![("NotFound".into(), 404), ("NotFound".into(), 404)],
            "duplicate error variant",
        ),
        (vec![("NotFound".into(), 200)], "4xx or 5xx"),
    ] {
        let policy = Rules(rules);
        assert!(items::http::describe_with(&policy)
            .unwrap_err()
            .to_string()
            .contains(message));
        assert!(description.errors("items.get", policy.mapping()).is_err());
        assert_eq!(description, before);
        assert!(items::shared(Backend).http(policy).is_err());
    }
    assert!(items::http::get::router(
        items::get::Service::new(std::sync::Arc::new(Backend)),
        Rules(vec![])
    )
    .is_err());
    assert!(description
        .errors("missing", ErrorMapping::Status(404))
        .is_err());
    assert_eq!(description, before);
    let mut app = api::http::describe_with(&DeclaredErrors).unwrap();
    let before = app.clone();
    assert!(app
        .errors(
            "health.get",
            ErrorMapping::Variants(vec![("x".into(), 409)])
        )
        .is_err());
    assert!(app.errors("items.get", ErrorMapping::Status(700)).is_err());
    assert_eq!(app, before);
    // Neutral descriptions do not consult HTTP policies.
    api::describe().unwrap();
}

#[tokio::test]
async fn custom_policies_override_defaults_and_remain_opaque_without_a_declaration() {
    let policy = |_: &AppError| StatusCode::IM_A_TEAPOT;
    assert_eq!(
        items::http::describe_with(&policy).unwrap().plans()["items.get"].errors,
        ErrorResponses::Opaque
    );
    let server = serve(items::shared(Backend).http(policy).unwrap()).await;
    let response = reqwest::get(format!("{}/items/0", server.base))
        .await
        .unwrap();
    assert_eq!(response.status(), 418);
    let client = items::http::Client::new(&server.base).unwrap();
    assert!(matches!(
        client.get(0).await,
        Err(CallError::Application(ApiError::NotFound))
    ));

    struct Override;
    impl ErrorPolicy<AppError> for Override {
        fn status(&self, _: &AppError) -> StatusCode {
            StatusCode::CONFLICT
        }
        fn mapping(&self) -> ErrorMapping {
            ErrorMapping::Status(409)
        }
    }
    let description = items::http::describe_with(&Override).unwrap();
    assert_eq!(
        declared(&description.plans()["items.get"].errors)
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [409]
    );
}

#[test]
fn aliases_recursive_external_errors_and_whole_type_overrides_have_explicit_metadata() {
    #[derive(Serialize, Schema, splinekit::http::Error)]
    #[splinekit(only = "serialize")]
    #[serde(tag = "code", content = "value", rename_all = "camelCase")]
    enum Recursive {
        #[http(status = StatusCode::NOT_FOUND)]
        Missing,
        #[http(status = StatusCode::CONFLICT)]
        Nested(Box<Recursive>),
    }
    #[derive(Schema)]
    struct Args {}
    let mut builder = splinekit::contract::ContractBuilder::new("recursive", "").unwrap();
    builder
        .operation::<Args, (), Recursive>(
            "get",
            "get",
            splinekit::contract::OperationKind::Query,
            "",
        )
        .unwrap();
    let mut builder = splinekit::http::contract::ContractBuilder::new(builder.finish().unwrap());
    builder
        .bind(
            "recursive.get",
            splinekit::http::contract::HttpBinding {
                method: splinekit::http::contract::HttpMethod::Get,
                path: "/recursive".into(),
                body: None,
                success_status: 200,
            },
        )
        .unwrap();
    let mut description = builder.finish().unwrap();
    description
        .errors(
            "recursive.get",
            <Recursive as splinekit::http::Error>::mapping(),
        )
        .unwrap();
    let mut actual = serde_json::to_value(Recursive::Nested(Box::new(Recursive::Missing))).unwrap();
    actual["status"] = json!(409);
    assert!(matches(
        description.contract().schemas(),
        &declared(&description.plans()["recursive.get"].errors)[&409].body,
        &actual
    ));

    #[derive(splinekit::Schema, splinekit::http::Error)]
    #[splinekit(schema = "String")]
    #[http(status = StatusCode::BAD_REQUEST)]
    struct CustomCodec;
    assert_eq!(
        <CustomCodec as splinekit::http::Error>::mapping(),
        ErrorMapping::Status(400)
    );

    use splinekit as renamed;
    #[derive(renamed::Schema, renamed::http::Error)]
    #[splinekit(crate = "renamed")]
    enum Keyword {
        #[http(status = StatusCode::FORBIDDEN)]
        #[allow(non_camel_case_types)]
        r#type,
    }
    assert_eq!(
        <Keyword as renamed::http::Error>::mapping(),
        ErrorMapping::Variants(vec![("type".into(), 403)])
    );
    assert_eq!(
        <Keyword as renamed::http::Error>::status(&Keyword::r#type),
        StatusCode::FORBIDDEN
    );
}

fn bind_with_error<E: Schema>() -> Result<(), String> {
    #[derive(Schema)]
    struct Args {}
    let mut builder = splinekit::contract::ContractBuilder::new("errors", "").unwrap();
    builder
        .operation::<Args, (), E>("get", "get", splinekit::contract::OperationKind::Query, "")
        .unwrap();
    let mut builder = splinekit::http::contract::ContractBuilder::new(builder.finish().unwrap());
    builder
        .bind(
            "errors.get",
            splinekit::http::contract::HttpBinding {
                method: splinekit::http::contract::HttpMethod::Get,
                path: "/errors".into(),
                body: None,
                success_status: 200,
            },
        )
        .map_err(|error| error.to_string())
}

#[test]
fn application_errors_must_be_code_tagged_objects_that_cannot_pass_for_problem_members() {
    #[allow(dead_code)]
    #[derive(Serialize, Schema)]
    #[splinekit(only = "serialize")]
    struct Untagged {
        message: String,
    }
    #[allow(dead_code)]
    #[derive(Serialize, Schema)]
    #[splinekit(only = "serialize")]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum WrongTag {
        Gone,
    }
    #[allow(dead_code)]
    #[derive(Serialize, Schema)]
    #[splinekit(only = "serialize")]
    #[serde(tag = "code", rename_all = "snake_case")]
    enum Reserved {
        InvalidRequest,
    }
    #[allow(dead_code)]
    #[derive(Serialize, Schema)]
    #[splinekit(only = "serialize")]
    #[serde(tag = "code", rename_all = "snake_case")]
    enum Member {
        Gone { detail: String },
    }
    #[allow(dead_code)]
    #[derive(Serialize, Schema)]
    #[splinekit(only = "serialize")]
    #[serde(tag = "code", rename_all = "snake_case")]
    enum Fine {
        Gone { at: String },
        Busy,
    }
    for (result, message) in [
        (bind_with_error::<Untagged>(), "tagged by `code`"),
        (bind_with_error::<WrongTag>(), "tagged by `code`"),
        (bind_with_error::<()>(), "tagged by `code`"),
        (
            bind_with_error::<Reserved>(),
            "`invalid_request` is reserved for framework faults",
        ),
        (
            bind_with_error::<Member>(),
            "field named `detail`, which problem details reserve",
        ),
    ] {
        let error = result.unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    bind_with_error::<Fine>().unwrap();
}
