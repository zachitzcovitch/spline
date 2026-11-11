use splinekit::Schema;

#[derive(Schema)]
struct MissingCodec {
    #[serde(with = "decimal")]
    value: u64,
}

fn main() {}
