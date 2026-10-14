use std::time::Duration;

use spline::http::{reqwest, CallError};
use spline_example_blog_contracts::{api, NewPost, PostError};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// -------- CLI entry point --------

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let base_url = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:3000/api".into());
    if args.next().is_some() {
        return Err("usage: blog-client [base-url]".into());
    }
    run(&base_url).await
}

// -------- Blog workflow --------

async fn run(base_url: &str) -> Result<()> {
    // -------- Authenticated clients --------

    let alice = create_client(base_url, "alice-token")?;
    let bob = create_client(base_url, "bob-token")?;

    // -------- Read a profile --------

    let user = alice.users().get("alice".into()).await?;
    println!("User: {}", user.display_name);

    // -------- Create and read a draft --------

    let new_post = NewPost {
        title: "Hello from Spline".into(),
        body: "One Rust contract, two typed clients.".into(),
    };
    let draft = alice.posts().create(new_post).await?;
    println!("Created draft: {}", draft.id);

    let fetched = alice.posts().get(draft.id.clone()).await?;
    println!("Read draft: {}", fetched.title);

    // -------- Handle Bob's expected Forbidden error --------

    match bob.posts().publish(draft.id.clone()).await {
        Err(CallError::Application(PostError::Forbidden)) => {
            println!("Bob cannot publish Alice's post (typed Forbidden)");
        }
        other => return Err(format!("expected Forbidden, received {other:?}").into()),
    }

    // -------- Publish as Alice --------

    let published = alice.posts().publish(draft.id).await?;
    println!(
        "Published: {} (published={})",
        published.id, published.published
    );

    Ok(())
}

// -------- Client configuration --------

fn create_client(base_url: &str, token: &str) -> Result<api::http::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse()?,
    );
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()?;
    Ok(api::http::Client::with_http(base_url, http)?)
}
