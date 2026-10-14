#![cfg(feature = "http")]

mod support;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spline::http::{
    axum,
    contract::{ErrorMapping, ErrorResponses},
    reqwest, CallError, DeclaredErrors, ErrorPolicy, HttpExt, StatusCode, ERROR_HEADER,
};
use spline::{Direction, Schema, Shape};
use support::{body, field, graph, matches};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Details {
    message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Schema, spline::http::Error)]
#[serde(
    tag = "type",
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

#[derive(Debug, Serialize, Deserialize, Schema, spline::http::Error)]
#[http(status = StatusCode::CONFLICT)]
pub struct Problem {
    message: String,
}

pub mod items {
    use super::AppError;

    #[spline::contract]
    pub trait Items {
        #[query]
        #[http(get = "/items/{kind}")]
        async fn get(&self, kind: u8) -> Result<(), AppError>;
    }
}

pub mod health {
    use super::Problem;

    #[spline::contract]
    pub trait Health {
        #[query]
        #[http(get = "/health")]
        async fn get(&self) -> Result<(), Problem>;
    }
}

spline::api! {
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
        Err(Problem {
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
) -> &std::collections::BTreeMap<u16, spline::http::contract::ErrorResponse> {
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
        (0, 404, json!({"type": "absent"})),
        (
            1,
            422,
            json!({"type": "invalid", "payload": {"fieldName": "title"}}),
        ),
        (
            2,
            422,
            json!({"type": "rejected", "payload": {"message": "invalid"}}),
        ),
        (3, 403, json!({"type": "forbidden"})),
    ] {
        let response = http
            .get(format!("{}/items/{kind}", server.base))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        assert_eq!(response.headers()[ERROR_HEADER], "application");
        let actual: Value = response.json().await.unwrap();
        assert_eq!(actual, json!({"kind": "application", "value": payload}));
        let documented = &responses[&status];
        assert_eq!(documented.content_type, "application/json");
        assert_eq!(documented.headers[ERROR_HEADER], "application");
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
    let Err(CallError::Application(problem)) = native.health().get().await else {
        panic!("typed struct error")
    };
    assert_eq!(problem.message, "unavailable");
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
        field(&variants[0], "type").shape,
        Shape::Literal("absent".into())
    );
    let value = &field(&declared(errors)[&422].body, "value").shape;
    let Shape::Union(alternatives) = value else {
        panic!("shared status")
    };
    assert_eq!(alternatives.len(), 2);
    assert!(!matches(
        description.contract().schemas(),
        &declared(errors)[&404].body,
        &json!({"kind":"application", "value":{"type":"not_found"}})
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
    #[derive(Serialize, Schema, spline::http::Error)]
    #[spline(only = "serialize")]
    #[serde(rename_all = "camelCase")]
    enum Recursive {
        #[http(status = StatusCode::NOT_FOUND)]
        Missing,
        #[http(status = StatusCode::CONFLICT)]
        Nested(Box<Recursive>),
    }
    #[derive(Schema)]
    struct Args {}
    let mut builder = spline::contract::ContractBuilder::new("recursive", "").unwrap();
    builder
        .operation::<Args, (), Recursive>("get", "get", spline::contract::OperationKind::Query, "")
        .unwrap();
    let mut builder = spline::http::contract::ContractBuilder::new(builder.finish().unwrap());
    builder
        .bind(
            "recursive.get",
            spline::http::contract::HttpBinding {
                method: spline::http::contract::HttpMethod::Get,
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
            <Recursive as spline::http::Error>::mapping(),
        )
        .unwrap();
    let actual = json!({"kind":"application", "value":serde_json::to_value(Recursive::Nested(Box::new(Recursive::Missing))).unwrap()});
    assert!(matches(
        description.contract().schemas(),
        &declared(&description.plans()["recursive.get"].errors)[&409].body,
        &actual
    ));

    #[derive(spline::Schema, spline::http::Error)]
    #[spline(schema = "String")]
    #[http(status = StatusCode::BAD_REQUEST)]
    struct CustomCodec;
    assert_eq!(
        <CustomCodec as spline::http::Error>::mapping(),
        ErrorMapping::Status(400)
    );

    use spline as renamed;
    #[derive(renamed::Schema, renamed::http::Error)]
    #[spline(crate = "renamed")]
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
