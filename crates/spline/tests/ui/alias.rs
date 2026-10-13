use spline::Schema;

#[derive(Schema)]
struct Alias {
    #[serde(alias = "old_name")]
    name: String,
}

fn main() {}
