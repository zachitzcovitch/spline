#[spline::contract(namespace = "users")]
trait Generic<T> {
    #[query]
    async fn get(&self) -> Result<T, ()>;
}

#[spline::contract(namespace = "users")]
trait Mutable {
    #[query]
    async fn get(&mut self) -> Result<(), ()>;
}

#[spline::contract(namespace = "users")]
trait Infallible {
    #[query]
    async fn get(&self) -> String;
}

fn main() {}
