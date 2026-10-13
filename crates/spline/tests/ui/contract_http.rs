#[spline::contract(namespace = "users")]
trait Users {
    #[query]
    #[http(get = "/users/{missing}")]
    async fn get(&self) -> Result<(), ()>;
}

fn main() {}
