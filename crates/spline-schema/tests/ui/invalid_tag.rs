use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "kind", content = "kind")]
enum InvalidTag {
    Value { value: String },
}

fn main() {}
