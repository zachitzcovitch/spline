#[derive(splinekit::Schema)]
struct Opaque;

#[splinekit::contract(namespace = "users")]
trait Users {
    #[query]
    async fn get(&self) -> Result<Opaque, ()>;
}

fn main() {}
