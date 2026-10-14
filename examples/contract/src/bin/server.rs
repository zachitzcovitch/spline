use spline::http::axum;
use spline_example_contract::{error_status, users, DemoUsers};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    let router = users::http::router(DemoUsers::default(), error_status)?;
    println!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, router).await?;
    Ok(())
}
