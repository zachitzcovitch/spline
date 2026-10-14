pub mod users {
    #[spline::contract]
    pub trait Users {
        #[query]
        #[http(get = "/health")]
        async fn health(&self) -> Result<(), ()>;
    }
    pub struct Implementation;
    impl Users for Implementation {
        async fn health(&self) -> Result<(), ()> { Ok(()) }
    }
}

use spline::http::HttpExt;

fn main() {
    let _ = users::shared(users::Implementation).http(());
}
