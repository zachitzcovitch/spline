//! A contract that declares where its RFC 9457 problem types live: the server
//! stamps `type` on every error, and both clients accept only problem details
//! carrying the service's own type for their code, so a proxy's or gateway's
//! JSON can't pass as the service's answer.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use splinekit::http::{
    axum, reqwest, CallError, DeclaredErrors, Fault, RequestContext, StatusCode,
};
use splinekit::Schema;

const BASE: &str = "https://example.test/errors";

#[derive(Debug, PartialEq, Serialize, Deserialize, Schema, splinekit::http::Error)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum OrderError {
    #[http(status = StatusCode::NOT_FOUND)]
    OrderMissing,
}

pub mod orders {
    use super::OrderError;

    #[splinekit::contract]
    #[http(problem_types = "https://example.test/errors")]
    pub trait Orders {
        #[query]
        #[http(get = "/orders/{id}")]
        async fn get(&self, id: u8) -> Result<(), OrderError>;
    }
}

pub mod plain {
    use super::OrderError;

    #[splinekit::contract]
    pub trait Plain {
        #[query]
        #[http(get = "/plain/{id}")]
        async fn get(&self, id: u8) -> Result<(), OrderError>;
    }
}

splinekit::api! {
    pub mod mixed {
        orders: crate::orders,
        plain: crate::plain,
    }
}

struct Backend;
impl orders::Orders for Backend {
    async fn get(&self, id: u8) -> Result<(), OrderError> {
        if id == 0 {
            Err(OrderError::OrderMissing)
        } else {
            Ok(())
        }
    }
}
impl plain::Plain for Backend {
    async fn get(&self, _id: u8) -> Result<(), OrderError> {
        Err(OrderError::OrderMissing)
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

async fn get(http: &reqwest::Client, url: String, deny: bool) -> (u16, Value) {
    let mut request = http.get(url);
    if deny {
        request = request.header("x-deny", "1");
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap())
}

#[tokio::test]
async fn the_server_stamps_the_problem_type_on_every_error() {
    let router = orders::http::router_with(
        |context: RequestContext| async move {
            if context.headers.contains_key("x-deny") {
                Err(Fault::new(403, "access_denied", "not yours"))
            } else {
                Ok(Arc::new(Backend))
            }
        },
        DeclaredErrors,
    )
    .unwrap()
    .merge(plain::http::router(Backend, DeclaredErrors).unwrap());
    let server = serve(router).await;
    let http = reqwest::Client::new();
    let base = &server.base;

    // An application error, a fault from decoding the input, and a fault from
    // the service factory all carry the type for their code.
    assert_eq!(
        get(&http, format!("{base}/orders/0"), false).await,
        (
            404,
            json!({ "type": format!("{BASE}/order-missing"), "status": 404, "code": "order_missing" })
        )
    );
    let (status, invalid) = get(&http, format!("{base}/orders/nine"), false).await;
    assert_eq!(status, 400);
    assert_eq!(invalid["type"], format!("{BASE}/invalid-request"));
    assert_eq!(invalid["code"], "invalid_request");
    assert_eq!(
        get(&http, format!("{base}/orders/1"), true).await,
        (
            403,
            json!({
                "type": format!("{BASE}/access-denied"),
                "status": 403,
                "code": "access_denied",
                "detail": "not yours",
            })
        )
    );

    // A contract that declares no problem types sends none.
    assert_eq!(
        get(&http, format!("{base}/plain/1"), false).await,
        (404, json!({ "status": 404, "code": "order_missing" }))
    );

    // The generated clients read the service's own answers as before.
    let client = orders::http::Client::new(base).unwrap();
    assert!(matches!(
        client.get(0).await,
        Err(CallError::Application(OrderError::OrderMissing))
    ));
    client.get(1).await.unwrap();
}

/// A stand-in for whatever answers at the service's address: its own
/// problem details, or a proxy's.
fn answering(body: Value, status: u16) -> axum::Router {
    axum::Router::new().route(
        "/orders/{id}",
        axum::routing::get(move || {
            let body = body.clone();
            async move {
                axum::response::Response::builder()
                    .status(status)
                    .header("content-type", "application/problem+json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap()
            }
        }),
    )
}

#[tokio::test]
async fn the_rust_client_accepts_only_the_services_problem_types() {
    let genuine_fault = json!({
        "type": format!("{BASE}/access-denied"), "status": 403, "code": "access_denied",
    });
    let server = serve(answering(genuine_fault, 403)).await;
    let client = orders::http::Client::new(&server.base).unwrap();
    let Err(CallError::Framework(fault)) = client.get(1).await else {
        panic!("expected the service's fault")
    };
    assert_eq!(fault.code, "access_denied");

    let genuine_error = json!({
        "type": format!("{BASE}/order-missing"), "status": 404, "code": "order_missing",
    });
    let server = serve(answering(genuine_error, 404)).await;
    let client = orders::http::Client::new(&server.base).unwrap();
    assert!(matches!(
        client.get(1).await,
        Err(CallError::Application(OrderError::OrderMissing))
    ));

    // A gateway's JSON in the same shape, with no type, a type of its own,
    // or the service's type for a different code.
    for body in [
        json!({ "status": 502, "code": "bad_gateway", "detail": "upstream 10.0.3.4 timed out" }),
        json!({ "type": "about:blank", "status": 502, "code": "bad_gateway" }),
        json!({ "type": format!("{BASE}/order-missing"), "status": 502, "code": "bad_gateway" }),
    ] {
        let server = serve(answering(body.clone(), 502)).await;
        let client = orders::http::Client::new(&server.base).unwrap();
        assert!(
            matches!(client.get(1).await, Err(CallError::Http { status: 502 })),
            "{body} passed as the service's answer"
        );
    }
}

#[test]
fn descriptions_carry_the_base_and_an_application_must_agree_on_it() {
    assert_eq!(
        orders::http::describe().unwrap().problem_types(),
        Some(BASE)
    );
    assert_eq!(plain::http::describe().unwrap().problem_types(), None);
    // One client serves a whole application, so its contracts must agree.
    let error = mixed::http::describe().unwrap_err().to_string();
    assert!(error.contains("same problem types"), "{error}");
}

#[tokio::test]
async fn the_typescript_client_accepts_only_the_services_problem_types() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/problem-types")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
    let generated = splinekit_typescript::Typescript::new()
        .export_http_client(&orders::http::describe().unwrap(), "Orders")
        .unwrap()
        .source;
    std::fs::write(directory.join("orders.ts"), generated).unwrap();
    std::fs::write(
        directory.join("check.mjs"),
        r#"import assert from "node:assert/strict";
import { createOrdersClient } from "./orders.ts";

const BASE = "https://example.test/errors";
const answering = (body, status) => createOrdersClient("http://127.0.0.1:9", {
  fetch: async () => new Response(JSON.stringify(body), {
    status, headers: { "content-type": "application/problem+json" },
  }),
});

assert.deepEqual(
  await answering({ type: `${BASE}/order-missing`, status: 404, code: "order_missing" }, 404).get(1),
  { ok: false, error: { kind: "application", status: 404, value: { code: "order_missing" } } },
);
assert.deepEqual(
  await answering({ type: `${BASE}/access-denied`, status: 403, code: "access_denied", detail: "no" }, 403).get(1),
  { ok: false, error: { kind: "framework", status: 403, code: "access_denied", detail: "no" } },
);
for (const body of [
  { status: 502, code: "bad_gateway", detail: "upstream 10.0.3.4 timed out" },
  { type: "about:blank", status: 502, code: "bad_gateway" },
  { type: `${BASE}/order-missing`, status: 502, code: "bad_gateway" },
]) {
  assert.deepEqual(await answering(body, 502).get(1), { ok: false, error: { kind: "http", status: 502 } },
    `${JSON.stringify(body)} passed as the service's answer`);
}
"#,
    )
    .unwrap();
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new("node")
            .arg(directory.join("check.mjs"))
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(
        output.status.success(),
        "TypeScript problem types check failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
