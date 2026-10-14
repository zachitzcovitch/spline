fn describe() {}

#[spline::contract]
trait Resource {
    #[query]
    async fn health(&self) -> Result<(), ()>;
}

fn main() {}
