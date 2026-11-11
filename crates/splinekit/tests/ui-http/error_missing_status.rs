#[derive(splinekit::Schema, splinekit::http::Error)]
enum Failure {
    Missing,
}

#[derive(splinekit::Schema, splinekit::http::Error)]
struct StructFailure;

fn main() {}
