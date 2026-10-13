use serde::Deserialize;

#[derive(Deserialize)]
struct Borrowed<'de> {
    value: &'de str,
}

fn main() {}
