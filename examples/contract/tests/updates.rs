use std::io::{Read, Write};
use std::time::Duration;

use serde_json::json;
use spline::{
    http::{self, axum, reqwest, CallError},
    Patch,
};
use spline_example_contract::{
    error_status, users, DemoUsers, NewUser, ReplaceUser, UserId, UserPatch,
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

async fn server() -> Running {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = users::router(DemoUsers::default(), error_status).unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Running { base, task }
}

#[tokio::test]
async fn native_client_replaces_updates_clears_and_deletes_a_resource() {
    let server = server().await;
    let client = users::Client::new(&server.base).unwrap();
    let user = client
        .create(NewUser {
            display_name: "First".into(),
            tags: vec![],
        })
        .await
        .unwrap();
    let id = user.id;
    let replacement = client
        .replace(
            id.clone(),
            ReplaceUser {
                display_name: "Replacement".into(),
                bio: Some("baseline".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        client
            .replace(
                id.clone(),
                ReplaceUser {
                    display_name: "Replacement".into(),
                    bio: Some("baseline".into())
                }
            )
            .await
            .unwrap(),
        replacement
    );
    assert_eq!(
        client
            .update(id.clone(), UserPatch::default())
            .await
            .unwrap(),
        replacement
    );
    let changed = client
        .update(
            id.clone(),
            UserPatch {
                display_name: Patch::Value("Partial".into()),
                bio: Patch::Missing,
            },
        )
        .await
        .unwrap();
    assert_eq!(changed.display_name, "Partial");
    assert_eq!(changed.bio.as_deref(), Some("baseline"));
    let changed = client
        .update(
            id.clone(),
            UserPatch {
                bio: Patch::Value(Some("new bio".into())),
                ..UserPatch::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(changed.bio.as_deref(), Some("new bio"));
    let cleared = client
        .update(
            id.clone(),
            UserPatch {
                bio: Patch::Value(None),
                ..UserPatch::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(cleared.display_name, "Partial");
    assert_eq!(cleared.bio, None);
    // Application validation is atomic: an invalid name must not apply the bio.
    assert!(matches!(
        client
            .update(
                id.clone(),
                UserPatch {
                    display_name: Patch::Value(" ".into()),
                    bio: Patch::Value(Some("must not apply".into()))
                }
            )
            .await,
        Err(CallError::Application(_))
    ));
    assert_eq!(client.get(id.clone()).await.unwrap(), cleared);
    client.delete(id.clone()).await.unwrap();
    assert!(matches!(
        client.get(id.clone()).await,
        Err(CallError::Application(_))
    ));
    assert!(matches!(
        client.delete(id).await,
        Err(CallError::Application(_))
    ));
}

#[tokio::test]
async fn raw_http_observes_json_updates_and_an_actually_empty_204() {
    let server = server().await;
    let client = reqwest::Client::new();
    let resource = format!("{}/users/user_1", server.base);
    let response = client
        .put(&resource)
        .json(&json!({ "displayName": "Replacement", "bio": "before" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["bio"],
        "before"
    );
    for (body, name, bio) in [
        (json!({}), "Replacement", json!("before")),
        (
            json!({ "displayName": "Partial" }),
            "Partial",
            json!("before"),
        ),
        (json!({ "bio": null }), "Partial", json!(null)),
    ] {
        let response = client.patch(&resource).json(&body).send().await.unwrap();
        assert_eq!(response.status(), 200);
        let user: serde_json::Value = response.json().await.unwrap();
        assert_eq!(user["displayName"], name);
        assert_eq!(user["bio"], bio);
    }
    for body in [
        json!(null),
        json!({ "displayName": null }),
        json!({ "bio": 7 }),
        json!({ "unknown": true }),
    ] {
        let response = client.patch(&resource).json(&body).send().await.unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(response.headers()[http::ERROR_HEADER], "framework");
    }
    assert_eq!(
        client
            .put(&resource)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .patch(&resource)
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        415
    );
    assert_eq!(
        client
            .delete(&resource)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let response = client.delete(&resource).send().await.unwrap();
    assert_eq!(response.status(), 204);
    assert!(!response.headers().contains_key("content-type"));
    assert!(!response.headers().contains_key("content-length"));
    assert!(!response.headers().contains_key("transfer-encoding"));
    assert!(!response.headers().contains_key(http::ERROR_HEADER));
    assert!(response.bytes().await.unwrap().is_empty());
    let missing = client.delete(&resource).send().await.unwrap();
    assert_eq!(missing.status(), 404);
    assert_eq!(missing.headers()[http::ERROR_HEADER], "application");
}

#[tokio::test]
async fn native_client_rejects_wrong_status_and_noncompliant_no_content_framing() {
    let responses: &[&[u8]] = &[
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Type: application/json\r\nConnection: close\r\n\r\nnull",
        b"HTTP/1.1 204 No Content\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnull",
        b"HTTP/1.1 204 No Content\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n0\r\n\r\n",
        b"HTTP/1.1 204 No Content\r\nx-spline-error: application\r\nConnection: close\r\n\r\n",
        b"HTTP/1.1 204 No Content\r\nx-spline-error: \xff\r\nConnection: close\r\n\r\n",
    ];
    for &raw in responses {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::task::spawn_blocking(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            let _ = socket.read(&mut bytes).unwrap();
            socket.write_all(raw).unwrap();
        });
        let transport = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let client = users::Client::with_http(&base, transport).unwrap();
        assert!(
            matches!(
                client.delete(UserId("x".into())).await,
                Err(CallError::Protocol(_))
            ),
            "{raw:?}"
        );
        task.await.unwrap();
    }
}

#[tokio::test]
async fn response_helper_never_serializes_a_json_null_body_for_204() {
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    let policy = |_: &()| StatusCode::BAD_REQUEST;
    let response = http::server::respond::<(), (), _>(Ok(()), 204, &policy);
    assert_eq!(response.status(), 204);
    assert!(!response.headers().contains_key("content-type"));
    assert!(to_bytes(response.into_body(), 100)
        .await
        .unwrap()
        .is_empty());
    let response = http::server::respond::<(), (), _>(Ok(()), 200, &policy);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"null"
    );
    // Direct helper misuse must not silently discard a non-null success payload.
    let response = http::server::respond::<_, (), _>(Ok("payload"), 204, &policy);
    assert_eq!(response.status(), 500);
}
