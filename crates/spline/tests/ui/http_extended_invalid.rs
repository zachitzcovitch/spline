#[spline::contract(namespace = "test")]
trait QueryPut {
    #[query]
    #[http(put = "/test", body = "input")]
    async fn replace(&self, input: String) -> Result<(), ()>;
}

#[spline::contract(namespace = "test")]
trait MissingPatchBody {
    #[mutation]
    #[http(patch = "/test")]
    async fn update(&self) -> Result<(), ()>;
}

#[spline::contract(namespace = "test")]
trait DeleteBody {
    #[mutation]
    #[http(delete = "/test", body = "input")]
    async fn delete(&self, input: String) -> Result<(), ()>;
}

#[spline::contract(namespace = "test")]
trait UnsupportedStatus {
    #[mutation]
    #[http(delete = "/test", status = 202)]
    async fn delete(&self) -> Result<(), ()>;
}

fn main() {}
