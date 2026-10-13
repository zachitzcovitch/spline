#[derive(spline::Schema)]
struct Opaque;

#[spline::contract(namespace = "users")]
trait Users {
    #[query]
    async fn get(&self) -> Result<Opaque, ()>;
}

fn main() {}
