#[derive(spline::Schema, spline::http::Error)]
enum Failure {
    #[http(status = 404)]
    Missing,
}

fn main() {}
