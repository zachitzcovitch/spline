#[spline::contract(namespace = "test", module = "non_unit")]
trait NonUnit {
    #[mutation]
    #[http(delete = "/test", status = 204)]
    async fn delete(&self) -> Result<String, ()>;
}

#[spline::contract(namespace = "nullable", module = "nullable")]
trait NullableIsNotUnit {
    #[mutation]
    #[http(delete = "/nullable", status = 204)]
    async fn delete(&self) -> Result<Option<()>, ()>;
}

fn main() {}
