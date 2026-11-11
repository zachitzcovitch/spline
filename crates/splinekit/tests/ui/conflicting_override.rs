use splinekit::Schema;

#[derive(Schema)]
#[splinekit(schema = "String", serialize_as = "u32")]
struct Conflicting(u32);

fn main() {}
