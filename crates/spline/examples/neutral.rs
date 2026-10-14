//! Also checked with --no-default-features: optional HTTP annotations and
//! neutral application composition must not require the HTTP runtime.
mod users {
    #[spline::contract]
    pub trait Users {
        #[query]
        #[http(get = "/health", status = 204)]
        async fn health(&self) -> Result<(), ()>;
    }
    pub struct Service;
    impl Users for Service {
        async fn health(&self) -> Result<(), ()> {
            Ok(())
        }
    }
}
spline::api! { mod api { users: self::users } }
use spline::runtime::InProcessExt;
#[tokio::main]
async fn main() {
    assert_eq!(api::describe().unwrap().operations().count(), 1);
    let client = api::shared(users::Service).in_process().unwrap();
    client.users().health().await.unwrap();
}
