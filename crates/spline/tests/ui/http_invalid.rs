#[spline::contract(namespace = "test")]
trait GetBody {
    #[query]
    #[http(get = "/test", body = "input")]
    async fn read(&self, input: String) -> Result<String, ()>;
}

#[spline::contract(namespace = "test")]
trait MutatingGet {
    #[mutation]
    #[http(get = "/test")]
    async fn update(&self) -> Result<String, ()>;
}

#[spline::contract(namespace = "test")]
trait Partial {
    #[query]
    #[http(get = "/test")]
    async fn read(&self) -> Result<String, ()>;
    #[mutation]
    async fn update(&self) -> Result<String, ()>;
}

fn main() {}
