use spline::http::{Fault, RequestContext};

/// Two fixed demo tokens identify the seeded users in this local example.
pub fn authenticate(context: &RequestContext) -> Result<&'static str, Fault> {
    match context
        .headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
    {
        Some("Bearer alice-token") => Ok("alice"),
        Some("Bearer bob-token") => Ok("bob"),
        _ => Err(Fault::new(
            401,
            "unauthorized",
            "A valid demo bearer token is required",
        )),
    }
}
