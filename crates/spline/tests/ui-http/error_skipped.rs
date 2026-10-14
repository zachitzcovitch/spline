#[derive(spline::Schema, spline::http::Error)]
enum Failure {
    #[http(status = spline::http::StatusCode::NOT_FOUND)]
    Missing,
    #[serde(skip_serializing)]
    #[http(status = spline::http::StatusCode::CONFLICT)]
    Hidden,
}

fn main() {}
