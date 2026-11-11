use splinekit::Schema;

#[derive(Schema)]
#[splinekit(nmae = "api.Typo")]
struct Typo {
    value: String,
}

fn main() {}
