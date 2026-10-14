#[path = "../fixtures/evolution.rs"]
mod fixtures;

use std::time::Duration;

use axum::http::StatusCode;
use fixtures::{required_input, v1, v2};
use serde_json::{json, Value};
use spline::http::{self, axum, reqwest, CallError, ResponsePolicy};

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

fn old_user(name: String) -> v1::User {
    let profile = v1::Profile { display_name: name };
    v1::User {
        id: "user_1".into(),
        profile: profile.clone(),
        history: vec![profile.clone()],
        profiles: [("primary".into(), profile)].into_iter().collect(),
        counters: [("visits".into(), 7)].into_iter().collect(),
        nickname: None,
        state: v1::State::Active,
    }
}

fn new_user(name: String) -> v2::User {
    let profile = v2::Profile {
        display_name: name,
        locale: "en".into(),
    };
    v2::User {
        id: "user_1".into(),
        profile: profile.clone(),
        history: vec![profile.clone()],
        profiles: [
            ("primary".into(), profile.clone()),
            ("added_key".into(), profile),
        ]
        .into_iter()
        .collect(),
        counters: [("visits".into(), 7), ("added_key".into(), 9)]
            .into_iter()
            .collect(),
        nickname: None,
        state: v2::State::Active,
        revision: 2,
    }
}

struct Old;
impl v1::Users for Old {
    async fn get(&self, _: String) -> Result<v1::User, v1::ApiError> {
        Ok(old_user("Old".into()))
    }
    async fn create(&self, input: v1::Create) -> Result<v1::User, v1::ApiError> {
        Ok(old_user(input.display_name))
    }
    async fn event(&self, id: String) -> Result<v1::Event, v1::ApiError> {
        Ok(v1::Event::Item { id })
    }
}

struct New;
impl v2::Users for New {
    async fn get(&self, id: String, audit: Option<String>) -> Result<v2::User, v2::ApiError> {
        match id.as_str() {
            "existing-error" => {
                return Err(v2::ApiError::Missing {
                    message: "missing".into(),
                    retryable: false,
                })
            }
            "new-error" => return Err(v2::ApiError::Busy { retry_after: 5 }),
            _ => {}
        }
        let mut user = new_user("New".into());
        user.counters
            .insert("audit".into(), u32::from(audit.is_some()));
        if id == "new-state" {
            user.state = v2::State::Suspended;
        }
        Ok(user)
    }
    async fn create(&self, input: v2::Create) -> Result<v2::User, v2::ApiError> {
        let mut user = new_user(input.display_name);
        user.state = input.state;
        user.counters
            .insert("tags".into(), input.tags.len().try_into().unwrap());
        Ok(user)
    }
    async fn event(&self, id: String) -> Result<v2::Event, v2::ApiError> {
        if id == "new-variant" {
            Ok(v2::Event::Removed { id })
        } else {
            Ok(v2::Event::Item { id, revision: 2 })
        }
    }
}

struct RequiresApproval;
impl required_input::Users for RequiresApproval {
    async fn create(&self, input: required_input::Create) -> Result<v2::User, v2::ApiError> {
        Ok(new_user(input.display_name))
    }
}

fn new_router() -> axum::Router {
    let router = v2::users::http::router(New, |_: &v2::ApiError| StatusCode::NOT_FOUND).unwrap();
    let router = router.route(
        "/evolution/users/bad/{case}",
        axum::routing::get(
            |axum::extract::Path(case): axum::extract::Path<String>| async move {
                let mut value = serde_json::to_value(new_user("New".into())).unwrap();
                match case.as_str() {
                    "missing" => {
                        value.as_object_mut().unwrap().remove("nickname");
                    }
                    "type" => value["profile"]["displayName"] = json!(7),
                    "null" => value["profile"] = Value::Null,
                    "array" => value["history"][0]["displayName"] = json!(false),
                    "map" => value["counters"]["visits"] = json!("seven"),
                    "range" => value["counters"]["visits"] = json!(u64::from(u32::MAX) + 1),
                    _ => unreachable!(),
                }
                axum::Json(value)
            },
        ),
    );
    router.route(
        "/evolution/events/ambiguous",
        axum::routing::get(|| async {
            axum::Json(json!({ "item": { "id": "x" }, "other": { "id": "x" } }))
        }),
    )
}

#[tokio::test]
async fn old_native_client_accepts_additive_responses_and_optional_inputs() {
    let server = serve(new_router()).await;
    let client = v1::users::http::Client::new(&server.base).unwrap();
    let user = client.get("existing".into()).await.unwrap();
    assert_eq!(user.profile.display_name, "New");
    assert_eq!(user.history[0].display_name, "New");
    assert_eq!(user.profiles["added_key"].display_name, "New");
    assert_eq!(user.counters["added_key"], 9);
    assert_eq!(user.counters["audit"], 0); // v1 sent no newly optional query argument
    let user = client
        .create(v1::Create {
            display_name: "Created by v1".into(),
            state: v1::State::Inactive,
        })
        .await
        .unwrap();
    assert_eq!(user.profile.display_name, "Created by v1");
    assert_eq!(user.counters["tags"], 0); // v2's new request field defaulted
    assert_eq!(user.state, v1::State::Inactive);
    assert_eq!(
        client.event("known".into()).await.unwrap(),
        v1::Event::Item { id: "known".into() }
    );
    assert!(matches!(client.get("existing-error".into()).await,
        Err(CallError::Application(v1::ApiError::Missing { message })) if message == "missing"));
}

#[tokio::test]
async fn strict_policy_rejects_additions_but_still_accepts_the_original_server() {
    let new = serve(new_router()).await;
    let strict = v1::users::http::Client::new(&new.base)
        .unwrap()
        .with_response_policy(ResponsePolicy::Strict);
    for id in ["existing", "existing-error"] {
        assert!(matches!(
            strict.get(id.into()).await,
            Err(CallError::Protocol(_))
        ));
    }
    assert!(matches!(
        strict.event("known".into()).await,
        Err(CallError::Protocol(_))
    ));
    let old =
        serve(v1::users::http::router(Old, |_: &v1::ApiError| StatusCode::NOT_FOUND).unwrap())
            .await;
    let strict = v1::users::http::Client::new(&old.base)
        .unwrap()
        .with_response_policy(ResponsePolicy::Strict);
    assert_eq!(
        strict.get("existing".into()).await.unwrap(),
        old_user("Old".into())
    );
}

#[tokio::test]
async fn new_variants_and_invalid_known_fields_remain_protocol_failures() {
    let server = serve(new_router()).await;
    let client = v1::users::http::Client::new(&server.base).unwrap();
    for id in ["new-state", "new-error"] {
        assert!(matches!(
            client.get(id.into()).await,
            Err(CallError::Protocol(_))
        ));
    }
    for id in ["new-variant", "ambiguous"] {
        assert!(matches!(
            client.event(id.into()).await,
            Err(CallError::Protocol(_))
        ));
    }
    // Bind a low-level client to deliberately malformed server responses while
    // preserving the original response/error schemas and route success status.
    for case in ["missing", "type", "null", "array", "map", "range"] {
        let url = format!("{}/evolution/users/bad/{case}", server.base);
        let response = reqwest::get(url)
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        let bad = axum::Router::new().route(
            "/evolution/users/{id}",
            axum::routing::get(move || {
                let response = response.clone();
                async move { axum::Json(response) }
            }),
        );
        let bad = serve(bad).await;
        let client = v1::users::http::Client::new(&bad.base).unwrap();
        assert!(
            matches!(client.get("test".into()).await, Err(CallError::Protocol(_))),
            "{case}"
        );
    }
}

#[tokio::test]
async fn required_input_additions_break_old_requests_and_unknown_inputs_stay_rejected() {
    let required = serve(
        required_input::users::http::router(RequiresApproval, |_: &v2::ApiError| {
            StatusCode::BAD_REQUEST
        })
        .unwrap(),
    )
    .await;
    let client = v1::users::http::Client::new(&required.base).unwrap();
    assert!(matches!(
        client
            .create(v1::Create {
                display_name: "v1".into(),
                state: v1::State::Active
            })
            .await,
        Err(CallError::Framework(http::Fault { status: 400, .. }))
    ));
    let new = serve(new_router()).await;
    let client = reqwest::Client::new();
    let unknown = client
        .post(format!("{}/evolution/users", new.base))
        .json(&json!({ "displayName": "x", "state": "active", "typo": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 400);
    let new_variant = client
        .post(format!("{}/evolution/users", new.base))
        .json(&json!({ "displayName": "x", "state": "suspended" }))
        .send()
        .await
        .unwrap();
    assert_eq!(new_variant.status(), 201); // expanding the server's accepted enum is valid
}

#[tokio::test]
async fn generated_v1_typescript_client_uses_the_same_evolution_policy() {
    let new = serve(new_router()).await;
    let old =
        serve(v1::users::http::router(Old, |_: &v1::ApiError| StatusCode::NOT_FOUND).unwrap())
            .await;
    let required = serve(
        required_input::users::http::router(RequiresApproval, |_: &v2::ApiError| {
            StatusCode::BAD_REQUEST
        })
        .unwrap(),
    )
    .await;
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/typescript/evolution.mjs"
            ))
            .arg(&new.base)
            .arg(&old.base)
            .arg(&required.base)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn v1_typescript_artifact_is_built_only_from_the_v1_contract() {
    let source = fixtures::typescript().unwrap();
    assert_eq!(
        source,
        include_str!("../typescript/evolution-v1.generated.ts")
    );
    assert!(!source.contains("retryable"));
    assert!(!source.contains("suspended"));
    let operations = v1::users::describe().unwrap();
    assert_eq!(operations.operations()["get"].arguments.len(), 1);
    assert_eq!(
        v2::users::describe().unwrap().operations()["get"]
            .arguments
            .len(),
        2
    );
}
