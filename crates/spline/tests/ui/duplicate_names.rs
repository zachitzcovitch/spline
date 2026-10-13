use spline::Schema;

#[derive(Schema)]
struct Fields {
    #[serde(rename = "same")]
    first: String,
    #[serde(rename = "same")]
    second: String,
}

#[derive(Schema)]
enum Variants {
    #[serde(rename = "same")]
    First,
    #[serde(rename = "same")]
    Second,
}

fn main() {}
