use spline::Schema;

#[derive(Schema)]
#[spline(name = "api.invalid-name")]
struct Invalid;

fn main() {}
