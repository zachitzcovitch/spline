pub mod users {
    #[splinekit::contract]
    pub trait Users {
        #[query]
        #[http(get = "/health")]
        async fn health(&self) -> Result<(), ()>;
    }
}

use splinekit::http::HttpFactoryExt;

fn main() {
    let _ = users::contract().http_with(
        |_| async { Ok(std::sync::Arc::new(())) },
        |_: &()| splinekit::http::axum::http::StatusCode::BAD_REQUEST,
    );
}
