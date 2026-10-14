#[derive(spline::Schema, spline::http::Error)]
#[spline(schema = "String")]
enum Failure {
    #[http(status = spline::http::StatusCode::NOT_FOUND)]
    Missing,
}

fn main() {}
