use spline::Schema;

#[derive(Schema)]
struct Flattened {
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, String>,
}

fn main() {}
