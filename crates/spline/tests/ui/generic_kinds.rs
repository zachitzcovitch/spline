use spline::Schema;

#[derive(Schema)]
struct Borrowed<'a> {
    value: &'a str,
}

#[derive(Schema)]
struct Fixed<const N: usize> {
    value: [u8; N],
}

fn main() {}
