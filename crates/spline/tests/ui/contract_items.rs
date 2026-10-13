#[spline::contract(namespace = "users")]
trait Associated {
    type User;
}

#[spline::contract(namespace = "users")]
trait SyncMethod {
    #[query]
    fn get(&self) -> Result<(), ()>;
}

fn main() {}
