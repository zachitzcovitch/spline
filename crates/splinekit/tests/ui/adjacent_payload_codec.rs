use splinekit::Schema;

#[derive(Schema)]
#[serde(tag = "kind", content = "value")]
enum Adjacent {
    Value(#[serde(with = "codec")] #[splinekit(schema = "String")] Option<String>),
}

fn main() {}
