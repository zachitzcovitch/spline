#[derive(splinekit::Schema, splinekit::http::Error)]
enum Failure {
    #[http(status = 404)]
    Missing,
}

fn main() {}
