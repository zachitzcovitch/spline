#[derive(splinekit::Schema, splinekit::http::Error)]
enum Failure {
    #[http(status = splinekit::http::StatusCode::NOT_FOUND)]
    Missing,
    #[serde(skip_serializing)]
    #[http(status = splinekit::http::StatusCode::CONFLICT)]
    Hidden,
}

fn main() {}
