use splinekit::Schema;

#[derive(Schema)]
#[serde(untagged)]
enum Untagged {
    Text(String),
    Flag(bool),
}

fn main() {}
