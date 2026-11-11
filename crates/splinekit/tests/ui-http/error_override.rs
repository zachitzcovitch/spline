#[derive(splinekit::Schema, splinekit::http::Error)]
#[splinekit(schema = "String")]
enum Failure {
    #[http(status = splinekit::http::StatusCode::NOT_FOUND)]
    Missing,
}

fn main() {}
