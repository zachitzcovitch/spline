pub mod users {
    #[spline::contract]
    pub trait Users {
        #[query]
        #[http(get = "/health")]
        async fn health(&self) -> Result<(), ()>;
    }
}

use spline::http::HttpFactoryExt;

fn main() {
    let _ = users::contract().http_with(
        |_| async { Ok(std::sync::Arc::new(())) },
        |_: &()| spline::http::axum::http::StatusCode::BAD_REQUEST,
    );
}
