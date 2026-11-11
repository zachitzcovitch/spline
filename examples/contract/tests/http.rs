use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use splinekit::{
    http::{self, axum, reqwest, CallError},
    Schema,
};
use splinekit_example_contract::users::http as users;
use splinekit_example_contract::{error_status, ApiError, DemoUsers, NewUser, UserId};

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

#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "conformance.Echo")]
#[serde(deny_unknown_fields)]
pub struct EchoValue {
    pub value: String,
    pub count: i32,
    pub flag: bool,
    pub note: Option<String>,
}

/// Snake-case wire names and an explicit rename: TypeScript sees camelCase
/// names of the Rust fields, the wire keeps Serde's names.
#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "conformance.Named")]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub display_name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub nested_items: Vec<NamedItem>,
    pub by_key: std::collections::BTreeMap<String, NamedItem>,
    pub maybe_value: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "conformance.NamedItem")]
#[serde(deny_unknown_fields)]
pub struct NamedItem {
    pub item_count: i32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "conformance.Status")]
#[serde(rename_all = "snake_case")]
pub enum EchoStatus {
    Active,
    PastDue,
}

/// Every formatted wire value, round-tripped through both generated clients.
#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "conformance.Formats")]
#[serde(deny_unknown_fields)]
pub struct Formats {
    #[splinekit(schema = "splinekit::wire::Uuid")]
    pub id: String,
    #[splinekit(schema = "splinekit::wire::DateTime")]
    pub at: String,
    #[splinekit(schema = "splinekit::wire::Date")]
    pub day: String,
    #[splinekit(schema = "splinekit::wire::Uri")]
    pub link: String,
    #[splinekit(schema = "splinekit::wire::Decimal")]
    pub amount: String,
    #[splinekit(schema = "splinekit::wire::IntegerString<u64>")]
    pub big: String,
    #[splinekit(schema = "splinekit::wire::SafeInteger<i64>")]
    pub safe: i64,
    pub ratio: f64,
    #[serde(with = "splinekit::codec::decimal_string")]
    pub total: u64,
    #[serde(with = "splinekit::codec::safe_number::option")]
    pub limit: Option<i64>,
    pub status: splinekit::Open<EchoStatus>,
}

#[splinekit::contract(namespace = "echo", module = "echo")]
pub trait Echo {
    #[query]
    #[http(get = "/health")]
    async fn health(&self) -> Result<(), ()>;

    #[query]
    #[http(get = "/no-content", status = 204)]
    async fn no_content(&self) -> Result<Empty, ()>;

    #[mutation]
    #[http(post = "/command", body = "fail", status = 204)]
    async fn command(&self, fail: bool) -> Result<(), ()>;

    #[query]
    #[http(get = "/echo/{value}")]
    async fn inspect(
        &self,
        value: String,
        count: i32,
        flag: bool,
        note: Option<String>,
    ) -> Result<EchoValue, ()>;

    #[mutation]
    #[http(post = "/formats", body = "input")]
    async fn formats(&self, input: Formats) -> Result<Formats, ()>;

    #[mutation]
    #[http(post = "/named", body = "input")]
    async fn named(&self, input: Named) -> Result<Named, ()>;
}
pub type Empty = ();
struct EchoService;
impl Echo for EchoService {
    async fn health(&self) -> Result<(), ()> {
        Ok(())
    }

    async fn no_content(&self) -> Result<Empty, ()> {
        Ok(())
    }

    async fn command(&self, fail: bool) -> Result<(), ()> {
        if fail {
            Err(())
        } else {
            Ok(())
        }
    }

    async fn inspect(
        &self,
        value: String,
        count: i32,
        flag: bool,
        note: Option<String>,
    ) -> Result<EchoValue, ()> {
        Ok(EchoValue {
            value,
            count,
            flag,
            note,
        })
    }

    async fn formats(&self, input: Formats) -> Result<Formats, ()> {
        Ok(input)
    }

    async fn named(&self, input: Named) -> Result<Named, ()> {
        Ok(input)
    }
}

#[tokio::test]
async fn rust_and_generated_typescript_clients_call_the_same_server() {
    let router = users::router(DemoUsers::default(), error_status)
        .unwrap()
        .merge(
            echo::http::router(EchoService, |_: &()| axum::http::StatusCode::BAD_REQUEST).unwrap(),
        );
    let server = serve(router).await;
    let client = users::Client::new(&server.base).unwrap();
    assert_eq!(
        client
            .get(UserId("user_1".into()))
            .await
            .unwrap()
            .display_name,
        "Zach"
    );
    let created = client
        .create(NewUser {
            display_name: "Rust".into(),
            tags: vec![],
        })
        .await
        .unwrap();
    assert_eq!(client.get(created.id.clone()).await.unwrap(), created);
    assert!(matches!(
        client.get(UserId("missing".into())).await,
        Err(CallError::Application(ApiError::NotFound))
    ));
    assert!(matches!(
        client
            .create(NewUser {
                display_name: " ".into(),
                tags: vec![]
            })
            .await,
        Err(CallError::Application(ApiError::InvalidName { .. }))
    ));

    let formats = Formats {
        id: "0191f1b2-7c4e-7a3b-9f1e-2a3b4c5d6e7f".into(),
        at: "2026-11-11T05:06:53.123Z".into(),
        day: "2026-11-11".into(),
        link: "https://example.test/a?b=c".into(),
        amount: "-12.50".into(),
        big: u64::MAX.to_string(),
        safe: -9_007_199_254_740_991,
        ratio: 0.25,
        total: u64::MAX,
        limit: None,
        status: splinekit::Open::Unknown("paused".into()),
    };
    let echo_client = echo::http::Client::new(&server.base).unwrap();
    let echoed = echo_client.formats(formats).await.unwrap();
    assert_eq!(echoed.big, "18446744073709551615");
    assert_eq!(echoed.safe, -9_007_199_254_740_991);
    assert_eq!((echoed.total, echoed.limit), (u64::MAX, None));
    assert_eq!(echoed.status, splinekit::Open::Unknown("paused".into()));

    let native = tokio::time::timeout(
        Duration::from_secs(20),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_rust-client"))
            .arg(&server.base)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        native.status.success(),
        "Rust client example failed: {}",
        String::from_utf8_lossy(&native.stderr)
    );

    // Exercise query mapping in the generated TS runtime too, using a test-only
    // artifact rather than adding a third operation to the public user example.
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/http-conformance")
        .join(server.base.rsplit(':').next().unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
    let generated = splinekit_typescript::Typescript::new()
        .export_http_client(&echo::http::describe().unwrap(), "Echo")
        .unwrap()
        .source;
    let echo_path = directory.join("echo.ts");
    std::fs::write(&echo_path, generated).unwrap();
    for script in ["client.mjs", "conformance.mjs"] {
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            tokio::process::Command::new("node")
                .arg(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("typescript")
                        .join(script),
                )
                .arg(&server.base)
                .arg(&echo_path)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            output.status.success(),
            "TypeScript {script} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn resource_routes_have_plain_json_bodies_and_declared_statuses() {
    let server = serve(users::router(DemoUsers::default(), error_status).unwrap()).await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/users", server.base))
        .json(&json!({ "displayName": "Direct HTTP" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    assert!(response.headers().get(http::ERROR_HEADER).is_none());
    let user: serde_json::Value = response.json().await.unwrap();
    assert_eq!(user["displayName"], "Direct HTTP");
    let error = client
        .get(format!("{}/users/missing", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(error.status(), 404);
    assert_eq!(error.headers()[http::ERROR_HEADER], "application");
    assert_eq!(
        error.json::<serde_json::Value>().await.unwrap(),
        json!({ "kind": "application", "value": { "kind": "not_found" } })
    );
}

#[tokio::test]
async fn malformed_requests_are_structured_framework_errors() {
    let server = serve(users::router(DemoUsers::default(), error_status).unwrap()).await;
    let client = reqwest::Client::new();
    let requests = [
        (
            client
                .post(format!("{}/users", server.base))
                .header("content-type", "application/json")
                .body("{"),
            400,
        ),
        (
            client
                .post(format!("{}/users", server.base))
                .json(&json!({ "input": { "displayName": "wrong wrapper" } })),
            400,
        ),
        (
            client
                .post(format!("{}/users", server.base))
                .body("not JSON"),
            415,
        ),
        (
            client.get(format!("{}/users/user_1?unexpected=true", server.base)),
            400,
        ),
        (
            client
                .get(format!("{}/users/user_1", server.base))
                .body("unexpected body"),
            400,
        ),
        (
            client
                .post(format!("{}/users", server.base))
                .json(&json!({ "displayName": "x".repeat(http::MAX_BODY_BYTES) })),
            413,
        ),
    ];
    for (request, status) in requests {
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()[http::ERROR_HEADER], "framework");
        let value: serde_json::Value = response.json().await.unwrap();
        assert_eq!(value["kind"], "framework");
        assert!(value["message"].is_string());
    }
}

#[tokio::test]
async fn encoded_paths_scalar_queries_and_base_prefixes_round_trip() {
    let router =
        echo::http::router(EchoService, |_: &()| axum::http::StatusCode::BAD_REQUEST).unwrap();
    let server = serve(axum::Router::new().nest("/v1", router)).await;
    let client = echo::http::Client::new(&format!("{}/v1/", server.base)).unwrap();
    assert!(client.health().await.is_ok());
    assert!(client.no_content().await.is_ok());
    assert!(client.command(false).await.is_ok());
    assert!(matches!(
        client.command(true).await,
        Err(CallError::Application(()))
    ));
    let value = "slash/space ?#%ü";
    let note = "query &+?/ü";
    assert_eq!(
        client
            .inspect(value.into(), -7, true, Some(note.into()))
            .await
            .unwrap(),
        EchoValue {
            value: value.into(),
            count: -7,
            flag: true,
            note: Some(note.into())
        }
    );
    assert_eq!(
        client
            .inspect("plain".into(), 0, false, None)
            .await
            .unwrap()
            .note,
        None
    );
    for invalid in ["", ".", ".."] {
        assert!(matches!(
            client.inspect(invalid.into(), 0, false, None).await,
            Err(CallError::Encode(_))
        ));
    }
    for query in [
        "count=1&count=2&flag=true",
        "count=1&flag=1",
        "flag=true",
        "count=2147483648&flag=true",
    ] {
        let response = reqwest::get(format!("{}/v1/echo/value?{query}", server.base))
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
}

#[tokio::test]
async fn request_scoped_factory_receives_headers_and_middleware_extensions() {
    #[derive(Clone)]
    struct Principal(&'static str);
    let service = Arc::new(DemoUsers::default());
    let seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = seen.clone();
    let router = users::router_with(
        move |context| {
            let service = service.clone();
            let observed = observed.clone();
            async move {
                if context
                    .headers
                    .get("x-test-token")
                    .and_then(|v| v.to_str().ok())
                    != Some("secret")
                {
                    return Err(http::Fault::new(401, "unauthorized", "missing test token"));
                }
                assert_eq!(context.extensions.get::<Principal>().unwrap().0, "member");
                assert_eq!(context.method, axum::http::Method::GET);
                assert_eq!(context.uri.path(), "/users/user_1");
                observed.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(service)
            }
        },
        error_status,
    )
    .unwrap()
    .layer(axum::Extension(Principal("member")));
    let server = serve(router).await;
    let client = users::Client::new(&server.base).unwrap();
    assert!(matches!(
        client.get(UserId("user_1".into())).await,
        Err(CallError::Framework(http::Fault { status: 401, .. }))
    ));
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-test-token", "secret".parse().unwrap());
    let transport = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let client = users::Client::with_http(&server.base, transport).unwrap();
    assert!(client.get(UserId("user_1".into())).await.is_ok());
    assert!(seen.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn client_distinguishes_protocol_http_and_transport_failures() {
    use axum::response::IntoResponse;
    let router = axum::Router::new().route(
        "/users/{case}",
        axum::routing::get(
            |axum::extract::Path(case): axum::extract::Path<String>| async move {
                match case.as_str() {
                    "shape" => axum::Json(json!({ "id": 3 })).into_response(),
                    "json" => ([("content-type", "application/json")], "{").into_response(),
                    "status" => (
                        axum::http::StatusCode::CREATED,
                        axum::Json(json!({ "id": "x", "displayName": "x" })),
                    )
                        .into_response(),
                    "error" => (
                        axum::http::StatusCode::BAD_REQUEST,
                        [(http::ERROR_HEADER, "application")],
                        axum::Json(json!({ "kind": "application", "value": 3 })),
                    )
                        .into_response(),
                    _ => (axum::http::StatusCode::SERVICE_UNAVAILABLE, "proxy failure")
                        .into_response(),
                }
            },
        ),
    );
    let server = serve(router).await;
    let client = users::Client::new(&server.base).unwrap();
    for case in ["shape", "json", "status", "error"] {
        assert!(matches!(
            client.get(UserId(case.into())).await,
            Err(CallError::Protocol(_))
        ));
    }
    assert!(matches!(
        client.get(UserId("proxy".into())).await,
        Err(CallError::Http { status: 503 })
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unavailable = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let transport = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let client = users::Client::with_http(&unavailable, transport).unwrap();
    assert!(matches!(
        client.get(UserId("x".into())).await,
        Err(CallError::Transport(_))
    ));
}

#[derive(Serialize, Deserialize, Schema)]
pub struct Asymmetric {
    #[serde(rename(serialize = "out", deserialize = "in"))]
    value: String,
}

#[splinekit::contract(namespace = "broken", module = "broken")]
pub trait Broken {
    #[mutation]
    #[http(post = "/broken", body = "input")]
    async fn put(&self, input: Asymmetric) -> Result<Asymmetric, ()>;
}

#[test]
fn native_client_checks_shared_dto_compatibility_and_base_url() {
    assert!(broken::http::Client::new("http://localhost")
        .unwrap_err()
        .to_string()
        .contains("incompatible"));
    for base in [
        "relative",
        "ftp://localhost",
        "http://localhost?query=x",
        "http://localhost#fragment",
        "http://localhost?",
        "http://localhost#",
        "http://user:password@localhost",
    ] {
        assert!(users::Client::new(base).is_err());
    }
}

#[tokio::test]
async fn http_accepts_the_same_typed_operation_middleware_and_maps_service_failures() {
    use splinekit::runtime::tower::{self, ServiceExt};
    let service =
        splinekit_example_contract::users::get::Service::new(Arc::new(DemoUsers::default()))
            .map_request(|mut input: splinekit_example_contract::users::get::Input| {
                input.id = UserId("user_1".into());
                input
            });
    let server = serve(users::get::router(service, error_status).unwrap()).await;
    assert_eq!(
        users::Client::new(&server.base)
            .unwrap()
            .get(UserId("missing".into()))
            .await
            .unwrap()
            .display_name,
        "Zach"
    );

    let failed = tower::service_fn(|_: splinekit_example_contract::users::get::Input| async {
        Err::<Result<splinekit_example_contract::User, ApiError>, _>(std::io::Error::other(
            "middleware failed",
        ))
    });
    let server = serve(users::get::router(failed, error_status).unwrap()).await;
    assert!(
        matches!(users::Client::new(&server.base).unwrap().get(UserId("user_1".into())).await,
        Err(CallError::Framework(http::Fault { status: 500, code, .. })) if code == "service_failure")
    );
}

/// A transport layer that sends the current session token and, when the server
/// answers 401, refreshes it once and re-sends the same request.
#[derive(Clone)]
struct RefreshOnce<S> {
    inner: S,
    token: Arc<std::sync::Mutex<String>>,
    refreshes: Arc<std::sync::atomic::AtomicUsize>,
}

impl<S> splinekit::runtime::tower::Service<http::HttpRequest> for RefreshOnce<S>
where
    S: splinekit::runtime::tower::Service<
            http::HttpRequest,
            Response = http::HttpResponse,
            Error = reqwest::Error,
        > + Clone
        + Send
        + Sync
        + 'static,
    S::Future: Send,
{
    type Response = http::HttpResponse;
    type Error = splinekit::runtime::BoxError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::HttpRequest) -> Self::Future {
        use splinekit::runtime::tower::ServiceExt;
        let inner = self.inner.clone();
        let token = Arc::clone(&self.token);
        let refreshes = Arc::clone(&self.refreshes);
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let send = |token: String| {
                let mut request = ::http::Request::new(body.clone());
                *request.method_mut() = parts.method.clone();
                *request.uri_mut() = parts.uri.clone();
                *request.headers_mut() = parts.headers.clone();
                request.headers_mut().insert(
                    ::http::header::AUTHORIZATION,
                    format!("Bearer {token}").parse().unwrap(),
                );
                inner.clone().oneshot(request)
            };
            let current = token.lock().unwrap().clone();
            let response = send(current).await?;
            if response.status() != ::http::StatusCode::UNAUTHORIZED {
                return Ok(response);
            }
            refreshes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let fresh = String::from("fresh");
            *token.lock().unwrap() = fresh.clone();
            Ok(send(fresh).await?)
        })
    }
}

#[tokio::test]
async fn a_transport_layer_refreshes_a_session_once_and_resends_the_request() {
    use axum::response::IntoResponse;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let seen = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&seen);
    let router = echo::http::router(EchoService, |_: &()| axum::http::StatusCode::BAD_REQUEST)
        .unwrap()
        .layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| {
                let counted = Arc::clone(&counted);
                async move {
                    counted.fetch_add(1, Ordering::SeqCst);
                    let authorized = request
                        .headers()
                        .get(axum::http::header::AUTHORIZATION)
                        .is_some_and(|value| value == "Bearer fresh");
                    if authorized {
                        next.run(request).await
                    } else {
                        axum::http::StatusCode::UNAUTHORIZED.into_response()
                    }
                }
            },
        ));
    let server = serve(router).await;
    let refreshes = Arc::new(AtomicUsize::new(0));
    let transport = RefreshOnce {
        inner: http::ReqwestTransport::new(reqwest::Client::new()),
        token: Arc::new(std::sync::Mutex::new("stale".into())),
        refreshes: Arc::clone(&refreshes),
    };
    let client = echo::http::Client::with_transport(&server.base, transport).unwrap();
    let value = client
        .inspect("body".into(), 3, true, Some("note".into()))
        .await
        .unwrap();
    assert_eq!(value.count, 3);
    assert_eq!(
        seen.load(Ordering::SeqCst),
        2,
        "one 401, then the same request again"
    );
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    client.health().await.unwrap();
    assert_eq!(
        seen.load(Ordering::SeqCst),
        3,
        "the refreshed token is reused"
    );
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
}
