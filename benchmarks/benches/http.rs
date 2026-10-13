use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spline::http::{axum, reqwest, ErrorPolicy};
use spline_example_blog_contracts::{api, NewPost, Post, PostError, Posts, User, UserError, Users};

struct Service(Arc<Post>);

impl Users for Service {
    async fn get(&self, _: String) -> Result<User, UserError> {
        Ok(self.0.author.clone())
    }
}

impl Posts for Service {
    async fn get(&self, _: String) -> Result<Post, PostError> {
        Ok((*self.0).clone())
    }

    async fn create(&self, _: NewPost) -> Result<Post, PostError> {
        Err(PostError::Forbidden)
    }

    async fn publish(&self, _: String) -> Result<Post, PostError> {
        Err(PostError::Forbidden)
    }
}

struct Errors;
impl ErrorPolicy<UserError> for Errors {
    fn status(&self, _: &UserError) -> axum::http::StatusCode {
        axum::http::StatusCode::NOT_FOUND
    }
}
impl ErrorPolicy<PostError> for Errors {
    fn status(&self, _: &PostError) -> axum::http::StatusCode {
        axum::http::StatusCode::FORBIDDEN
    }
}

fn http(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let mut group = c.benchmark_group("http/loopback_get_post");
    for body_bytes in [128, 16 * 1024] {
        let post = Post {
            id: "post_1".into(),
            author: User {
                id: "alice".into(),
                display_name: "Alice".into(),
            },
            title: "Benchmark post".into(),
            body: "x".repeat(body_bytes),
            published: true,
        };
        group.throughput(Throughput::Bytes(
            serde_json::to_vec(&post).unwrap().len() as u64
        ));
        let (base, task) = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let router = api::router(Service(Arc::new(post)), Errors).unwrap();
            let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
            (base, task)
        });
        let transport = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let client = api::Client::with_http(&base, transport.clone()).unwrap();
        let url = format!("{base}/posts/post_1");

        // Check the fixture and establish reusable connections before measurement.
        runtime.block_on(async {
            assert_eq!(
                client
                    .posts()
                    .get("post_1".into())
                    .await
                    .unwrap()
                    .body
                    .len(),
                body_bytes
            );
            assert_eq!(
                transport
                    .get(&url)
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap()
                    .json::<Post>()
                    .await
                    .unwrap()
                    .body
                    .len(),
                body_bytes
            );
        });

        group.bench_function(BenchmarkId::new("spline", body_bytes), |b| {
            b.to_async(&runtime).iter(|| async {
                black_box(
                    client
                        .posts()
                        .get(black_box("post_1").into())
                        .await
                        .unwrap(),
                );
            });
        });
        group.bench_function(BenchmarkId::new("reqwest_serde", body_bytes), |b| {
            b.to_async(&runtime).iter(|| async {
                black_box(
                    transport
                        .get(black_box(&url))
                        .send()
                        .await
                        .unwrap()
                        .error_for_status()
                        .unwrap()
                        .json::<Post>()
                        .await
                        .unwrap(),
                );
            });
        });

        task.abort();
        runtime.block_on(async {
            let _ = task.await;
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = http
}
criterion_main!(benches);
