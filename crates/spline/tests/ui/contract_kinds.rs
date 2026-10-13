#[spline::contract(namespace = "users")]
trait Missing {
    async fn get(&self) -> Result<(), ()>;
}

#[spline::contract(namespace = "users")]
trait Both {
    #[query]
    #[mutation]
    async fn get(&self) -> Result<(), ()>;
}

fn main() {}
