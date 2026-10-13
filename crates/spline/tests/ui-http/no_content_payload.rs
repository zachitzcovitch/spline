#[spline::contract(namespace = "test")]
trait NonUnit {
    #[mutation]
    #[http(delete = "/test", status = 204)]
    async fn delete(&self) -> Result<String, ()>;
}

#[spline::contract(namespace = "nullable")]
trait NullableIsNotUnit {
    #[mutation]
    #[http(delete = "/nullable", status = 204)]
    async fn delete(&self) -> Result<Option<()>, ()>;
}

fn main() {}
