#[spline::contract(namespace = "users")]
trait Users {
    #[query(name = "get")]
    async fn first(&self) -> Result<(), ()>;
    #[query(name = "get")]
    async fn second(&self) -> Result<(), ()>;
}

fn main() {}
