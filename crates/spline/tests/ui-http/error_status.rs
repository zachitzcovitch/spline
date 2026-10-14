use spline::http::StatusCode;

#[derive(spline::Schema, spline::http::Error)]
enum Failure {
    #[http(status = StatusCode::OK)]
    Invalid,
}

fn main() {}
