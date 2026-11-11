pub mod users {
    #[splinekit::contract]
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

use splinekit::http::HttpExt;

fn main() {
    let _ = users::shared(users::Implementation).http(());
}
