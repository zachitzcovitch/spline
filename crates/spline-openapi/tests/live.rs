mod support;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde_json::{json, Value};
use spline::http::{axum, reqwest, DeclaredErrors, Fault, HttpFactoryExt};
use spline_openapi::OpenApi;
use support::*;

async fn record(
    request: reqwest::RequestBuilder,
    path: &str,
    method: &str,
    expected: u16,
) -> Value {
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    assert_eq!(status, expected, "Unexpected status for {method} {path}");
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let body = response.text().await.unwrap();
    json!({"path":path,"method":method,"status":status,"headers":headers,"body":body})
}

struct Running(tokio::task::JoinHandle<()>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn exported_schemas_match_real_requests_responses_and_error_status_precedence() {
    let description = sample::http::describe_with(&DeclaredErrors).unwrap();
    let document = OpenApi::new("Live API", "1")
        .export_contract(&description)
        .unwrap();
    let omitted = Arc::new(AtomicBool::new(false));
    let observed = omitted.clone();
    let router = sample::contract()
        .http_with(
            move |context| {
                let observed = observed.clone();
                async move {
                    match context
                        .headers
                        .get("authorization")
                        .and_then(|header| header.to_str().ok())
                    {
                        Some("Bearer hidden") => {
                            return Err(Fault::new(404, "hidden", "not visible"))
                        }
                        Some("Bearer failure") => {
                            return Err(Fault::new(500, "unavailable", "unavailable"))
                        }
                        Some("Bearer valid") => {}
                        _ => return Err(Fault::new(401, "unauthorized", "token required")),
                    }
                    if context.uri.path() == "/profiles/7" && context.uri.query().is_none() {
                        observed.store(true, Ordering::SeqCst);
                    }
                    Ok(Arc::new(Backend))
                }
            },
            DeclaredErrors,
        )
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let _running = Running(tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    }));
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer valid".parse().unwrap());
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let mut responses = vec![
        record(
            http.get(format!("{base}/profiles/7")),
            "/profiles/{id}",
            "get",
            200,
        )
        .await,
    ];
    assert_eq!(responses[0]["status"], 200);
    assert!(omitted.load(Ordering::SeqCst));
    for (path, url, expected) in [
        ("/profiles/{id}", "/profiles/0", 404),
        ("/profiles/{id}", "/profiles/400", 400),
        ("/profiles/{id}", "/profiles/500", 500),
        ("/profiles/{id}", "/profiles/7?choice=unknown", 400),
        ("/profiles/{id}", "/profiles/7?count=1&count=2", 400),
    ] {
        responses.push(record(http.get(format!("{base}{url}")), path, "get", expected).await);
    }
    responses.push(
        record(
            reqwest::Client::new().get(format!("{base}/profiles/7")),
            "/profiles/{id}",
            "get",
            401,
        )
        .await,
    );
    for (token, expected) in [("hidden", 404), ("failure", 500)] {
        responses.push(
            record(
                http.get(format!("{base}/profiles/7"))
                    .header("authorization", format!("Bearer {token}")),
                "/profiles/{id}",
                "get",
                expected,
            )
            .await,
        );
    }
    responses.push(
        record(
            http.post(format!("{base}/profiles")).json(&json!({"id":7})),
            "/profiles",
            "post",
            201,
        )
        .await,
    );
    responses.push(
        record(
            http.post(format!("{base}/profiles")).body("{}"),
            "/profiles",
            "post",
            415,
        )
        .await,
    );
    responses.push(
        record(
            http.post(format!("{base}/profiles"))
                .header("content-type", "application/json")
                .body(vec![b' '; spline::http::MAX_BODY_BYTES + 1]),
            "/profiles",
            "post",
            413,
        )
        .await,
    );
    responses.push(
        record(
            http.post(format!("{base}/nullable")).json(&Value::Null),
            "/nullable",
            "post",
            200,
        )
        .await,
    );
    responses.push(
        record(
            http.post(format!("{base}/nullable"))
                .header("content-type", "application/json"),
            "/nullable",
            "post",
            400,
        )
        .await,
    );
    for body in [json!({}), json!({"note":null}), json!({"note":"updated"})] {
        responses.push(
            record(
                http.patch(format!("{base}/profiles/7")).json(&body),
                "/profiles/{id}",
                "patch",
                200,
            )
            .await,
        );
    }
    responses.push(
        record(
            http.delete(format!("{base}/profiles/7")),
            "/profiles/{id}",
            "delete",
            204,
        )
        .await,
    );
    responses.push(record(http.get(format!("{base}/unit")), "/unit", "get", 200).await);
    responses.push(record(http.get(format!("{base}/nodes")), "/nodes", "get", 200).await);
    responses.push(record(http.get(format!("{base}/pair")), "/pair", "get", 200).await);
    assert!(responses.iter().any(|response| response["status"] == 204));
    assert!(responses.iter().any(|response| response["status"] == 413));
    independent_responses(&document, json!(responses));
}
