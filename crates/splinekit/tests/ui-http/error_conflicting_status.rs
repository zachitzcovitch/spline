#[derive(splinekit::Schema, splinekit::http::Error)]
#[http(status = splinekit::http::StatusCode::BAD_REQUEST)]
enum Failure {
    #[http(status = splinekit::http::StatusCode::NOT_FOUND)]
    Missing,
}

#[derive(splinekit::Schema, splinekit::http::Error)]
enum Duplicate {
    #[http(status = splinekit::http::StatusCode::NOT_FOUND, status = splinekit::http::StatusCode::CONFLICT)]
    Missing,
}

fn main() {}
