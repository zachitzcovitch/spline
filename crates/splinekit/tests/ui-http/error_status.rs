use splinekit::http::StatusCode;

#[derive(splinekit::Schema, splinekit::http::Error)]
enum Failure {
    #[http(status = StatusCode::OK)]
    Invalid,
}

fn main() {}
