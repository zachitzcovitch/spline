mod auth;
mod errors;
mod service;

use std::sync::Arc;

use spline::http::axum;
use spline_example_blog_contracts::api;
use tokio::sync::RwLock;

use service::{Session, Store};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let address = args.next().unwrap_or_else(|| "127.0.0.1:3000".into());
    if args.next().is_some() {
        return Err("usage: blog-server [bind-address]".into());
    }

    let store = Arc::new(RwLock::new(Store::seeded()));
    let routes = api::router_with(
        move |context| {
            let store = Arc::clone(&store);
            async move {
                let user_id = auth::authenticate(&context)?;
                Ok(Arc::new(Session { store, user_id }))
            }
        },
        errors::ErrorStatuses,
    )?;

    // Both resource groups use the same /api prefix and request-scoped session.
    let router = axum::Router::new().nest("/api", routes);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Blog API: http://{}/api", listener.local_addr()?);
    axum::serve(listener, router).await?;
    Ok(())
}
