use spline::Schema;

#[derive(Schema)]
#[spline(nmae = "api.Typo")]
struct Typo {
    value: String,
}

fn main() {}
