#[derive(spline::Schema, spline::http::Error)]
enum Failure {
    Missing,
}

#[derive(spline::Schema, spline::http::Error)]
struct StructFailure;

fn main() {}
