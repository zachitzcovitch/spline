#[derive(spline::Schema, spline::http::Error)]
#[http(status = spline::http::StatusCode::BAD_REQUEST)]
enum Failure {
    #[http(status = spline::http::StatusCode::NOT_FOUND)]
    Missing,
}

#[derive(spline::Schema, spline::http::Error)]
enum Duplicate {
    #[http(status = spline::http::StatusCode::NOT_FOUND, status = spline::http::StatusCode::CONFLICT)]
    Missing,
}

fn main() {}
