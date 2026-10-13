use serde::Deserialize;

#[derive(Deserialize)]
struct MissingCodec {
    #[serde(deserialize_with = "missing")]
    value: String,
}

fn main() {}
