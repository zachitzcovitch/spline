use spline::Schema;

#[derive(Schema)]
#[serde(tag = "kind", content = "value")]
enum Adjacent {
    Value(#[serde(with = "codec")] #[spline(schema = "String")] Option<String>),
}

fn main() {}
