#[path = "../fixtures/application.rs"]
#[allow(dead_code)]
mod application;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use application::{api, Database, Errors, NewPost, PostError, Session, User, UserError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use spline::{
    http::{axum, reqwest, CallError, Fault, ResponsePolicy},
    Schema,
};

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
    let base = format!("http://{}/api", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, axum::Router::new().nest("/api", router))
            .await
            .unwrap();
    });
    Running { base, task }
}

fn authenticated_client(base: &str, principal: &str) -> api::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {principal}").parse().unwrap(),
    );
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    api::Client::with_http(base, http).unwrap()
}

#[tokio::test]
async fn both_clients_share_authentication_state_configuration_and_typed_resource_errors() {
    #[derive(Clone)]
    struct Trace(&'static str);
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let database = Database::default();
    let router = api::router_with(
        move |context| {
            let database = database.clone();
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                assert_eq!(
                    context.extensions.get::<Trace>().unwrap().0,
                    "shared middleware"
                );
                let principal = match context
                    .headers
                    .get("authorization")
                    .and_then(|h| h.to_str().ok())
                {
                    Some("Bearer alice") => "alice",
                    Some("Bearer bob") => "bob",
                    _ => {
                        return Err(Fault::new(
                            401,
                            "unauthorized",
                            "fixture authentication required",
                        ))
                    }
                };
                Ok(Arc::new(Session {
                    database,
                    principal: principal.into(),
                }))
            }
        },
        Errors,
    )
    .unwrap()
    .layer(axum::Extension(Trace("shared middleware")));
    let server = serve(router).await;
    let anonymous = api::Client::new(&server.base).unwrap();
    assert!(matches!(
        anonymous.users().get("alice".into()).await,
        Err(CallError::Framework(Fault { status: 401, .. }))
    ));
    assert!(matches!(
        anonymous
            .posts()
            .create(NewPost { title: "no".into() })
            .await,
        Err(CallError::Framework(Fault { status: 401, .. }))
    ));
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "factory is called once per request"
    );
    // Authentication is shared and runs before body decoding for every resource.
    let response = reqwest::Client::new()
        .post(format!("{}/posts", server.base))
        .header("content-type", "application/json")
        .body("{")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);

    let alice = authenticated_client(&server.base, "alice");
    let bob = authenticated_client(&server.base, "bob");
    assert_eq!(alice.users().get("alice".into()).await.unwrap().id, "alice");
    let created = alice
        .posts()
        .create(NewPost {
            title: "Shared application".into(),
        })
        .await
        .unwrap();
    assert_eq!(created.author.id, "alice");
    let renamed = alice.users().rename("Alice renamed".into()).await.unwrap();
    assert_eq!(
        bob.posts().get(created.id.clone()).await.unwrap().author,
        renamed
    );
    assert!(matches!(
        bob.posts().publish(created.id.clone()).await,
        Err(CallError::Application(PostError::Forbidden))
    ));
    assert!(
        alice
            .clone()
            .posts()
            .publish(created.id.clone())
            .await
            .unwrap()
            .published
    );
    assert!(matches!(
        alice.users().get("missing".into()).await,
        Err(CallError::Application(UserError::NotFound))
    ));
    assert!(matches!(
        alice.posts().create(NewPost { title: " ".into() }).await,
        Err(CallError::Application(PostError::InvalidTitle))
    ));
    alice.posts().delete(created.id.clone()).await.unwrap();
    assert!(matches!(
        alice.posts().get(created.id).await,
        Err(CallError::Application(PostError::NotFound))
    ));

    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new("node")
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("typescript/application.mjs"),
            )
            .arg(&server.base)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "TypeScript application conformance failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn one_response_policy_applies_to_every_native_resource() {
    let user = json!({ "id": "alice", "displayName": "Alice", "added": true });
    let post = json!({ "id": "post_1", "author": user.clone(), "title": "Post", "published": false, "added": true });
    let router = axum::Router::new()
        .route(
            "/users/{id}",
            axum::routing::get(move || async move { axum::Json(user) }),
        )
        .route(
            "/posts/{id}",
            axum::routing::get(move || async move { axum::Json(post) }),
        );
    let server = serve(router).await;
    let compatible = api::Client::new(&server.base).unwrap();
    assert!(compatible.users().get("alice".into()).await.is_ok());
    assert!(compatible.posts().get("post_1".into()).await.is_ok());
    let strict = compatible.with_response_policy(ResponsePolicy::Strict);
    assert!(matches!(
        strict.users().get("alice".into()).await,
        Err(CallError::Protocol(_))
    ));
    assert!(matches!(
        strict.posts().get("post_1".into()).await,
        Err(CallError::Protocol(_))
    ));
}

#[spline::contract(namespace = "collision.routes", module = "routes")]
pub trait Routes {
    #[query]
    #[http(get = "/users/{other}")]
    async fn get(&self, other: String) -> Result<User, UserError>;
}
impl Routes for Session {
    async fn get(&self, other: String) -> Result<User, UserError> {
        application::Users::get(self, other).await
    }
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "composition.User")]
pub struct Impostor {
    pub id: String,
    pub display_name: String,
}

#[spline::contract(namespace = "collision.schemas", module = "schemas")]
pub trait Schemas {
    #[query]
    #[http(get = "/impostor")]
    async fn get(&self) -> Result<Impostor, UserError>;
}
impl Schemas for Session {
    async fn get(&self) -> Result<Impostor, UserError> {
        Err(UserError::NotFound)
    }
}

spline::api! { mod bad_routes { users: application::users, other: self::routes } }
spline::api! { mod bad_schemas { users: application::users, other: self::schemas } }
spline::api! { mod bad_namespaces { users: application::users, people: application::users } }

#[test]
fn application_collisions_fail_before_router_merge_or_client_construction() {
    let session = || Session {
        database: Database::default(),
        principal: "alice".into(),
    };
    for error in [
        bad_routes::describe().unwrap_err().to_string(),
        bad_routes::router(session(), Errors)
            .unwrap_err()
            .to_string(),
        bad_routes::Client::new("http://localhost")
            .unwrap_err()
            .to_string(),
    ] {
        assert!(
            error.contains("app.users.get") && error.contains("collision.routes.get"),
            "{error}"
        );
    }
    assert!(matches!(
        bad_schemas::describe(),
        Err(spline::contract::Error::Schema(
            spline::Error::NameCollision { .. }
        ))
    ));
    assert!(bad_schemas::router(session(), Errors).is_err());
    assert!(bad_schemas::Client::new("http://localhost").is_err());
    assert!(matches!(
        bad_namespaces::describe(),
        Err(spline::contract::Error::DuplicateNamespace(_))
    ));
    assert!(bad_namespaces::router(session(), Errors).is_err());
    assert!(bad_namespaces::Client::new("http://localhost").is_err());
}

mod relative_paths {
    use spline as renamed;
    renamed::api! {
        crate = "renamed";
        pub mod grouped {
            people: super::application::users,
            articles: super::application::posts,
        }
    }
}

#[tokio::test]
async fn renamed_groups_relative_paths_and_shared_service_constructor_work() {
    let session = Session {
        database: Database::default(),
        principal: "alice".into(),
    };
    let server = serve(relative_paths::grouped::router(session, Errors).unwrap()).await;
    let client = relative_paths::grouped::Client::new(&server.base).unwrap();
    assert!(client.people().get("alice".into()).await.is_ok());
    assert!(client
        .articles()
        .create(NewPost {
            title: "Renamed group".into()
        })
        .await
        .is_ok());
    let description = relative_paths::grouped::describe().unwrap();
    assert_eq!(description.resources()["people"].namespace(), "app.users");
    assert_eq!(
        description.resources()["articles"].operations()["publish"].id,
        "app.posts.publish"
    );
    assert_eq!(description.schemas(), api::describe().unwrap().schemas());
}

#[test]
fn application_artifact_is_current_and_shared_dtos_are_emitted_once() {
    let source = application::typescript().unwrap();
    assert_eq!(
        source,
        include_str!("../typescript/application.generated.ts")
    );
    assert_eq!(source.matches("export type User =").count(), 1);
    assert_eq!(
        source.matches("export function createApiClient(").count(),
        1
    );
}

mod checks {
    use super::{Session, User, UserError};

    #[spline::contract]
    pub trait Checks {
        #[query(name = "get")]
        #[http(get = "/checks/{id}")]
        async fn lookup(&self, id: String) -> Result<User, UserError>;
    }

    impl Checks for Session {
        async fn lookup(&self, id: String) -> Result<User, UserError> {
            super::application::Users::get(self, id).await
        }
    }
}

spline::api! {
    mod mixed {
        users: application::users,
        checks: self::checks,
    }
}

#[tokio::test]
async fn same_module_http_resources_work_directly_and_compose_with_explicit_companions() {
    let session = Session {
        database: Database::default(),
        principal: "alice".into(),
    };
    let server = serve(mixed::router(session, Errors).unwrap()).await;
    let direct = checks::Client::new(&server.base).unwrap();
    assert_eq!(direct.lookup("alice".into()).await.unwrap().id, "alice");
    assert!(matches!(
        direct.lookup("missing".into()).await,
        Err(CallError::Application(UserError::NotFound))
    ));
    let client = mixed::Client::new(&server.base).unwrap();
    assert_eq!(
        client.checks().lookup("bob".into()).await.unwrap().id,
        "bob"
    );
    assert_eq!(client.users().get("bob".into()).await.unwrap().id, "bob");
    let description = checks::describe().unwrap();
    assert_eq!(description.operations()["get"].id, "checks.get");
    let source = spline_typescript::Typescript::new()
        .export_http_application(&mixed::describe().unwrap(), "Mixed")
        .unwrap()
        .source;
    assert!(source.contains("\"checks.get\""));
    assert!(source.contains("\"app.users.get\""));
}
