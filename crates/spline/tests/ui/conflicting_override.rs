use spline::Schema;

#[derive(Schema)]
#[spline(schema = "String", serialize_as = "u32")]
struct Conflicting(u32);

fn main() {}
