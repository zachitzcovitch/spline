#[splinekit::contract(namespace = "users")]
trait Borrowed {
    #[query]
    async fn get(&self, id: &str) -> Result<(), ()>;
}

#[splinekit::contract(namespace = "users")]
trait SelfType {
    #[query]
    async fn get(&self) -> Result<Self, ()>;
}

fn main() {}
