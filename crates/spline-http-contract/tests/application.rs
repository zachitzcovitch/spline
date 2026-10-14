use spline_contract::{ApplicationBuilder as NeutralBuilder, OperationKind};
use spline_http_contract::{ApplicationBuilder, HttpBinding, HttpMethod};
use spline_schema::{Context, Field, Object, Schema, Shape, TypeInfo, UnknownFields};

struct Input;
impl Schema for Input {
    fn info() -> TypeInfo {
        TypeInfo::named("test.Input")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::Object(Object {
            fields: vec![
                Field::required("id", Shape::String),
                Field::required("key", Shape::String),
            ],
            unknown_fields: UnknownFields::Reject,
        }))
    }
}
fn binding(method: HttpMethod, path: &str) -> HttpBinding {
    HttpBinding {
        method,
        path: path.into(),
        body: None,
        success_status: 200,
    }
}

#[test]
fn http_route_conflicts_are_separate_from_neutral_composition_and_recoverable() {
    let mut neutral = NeutralBuilder::new("");
    for resource in ["users", "posts"] {
        neutral
            .resource(resource, resource, "", |r| {
                r.operation::<Input, String, ()>("get", "get", OperationKind::Query, "")
            })
            .unwrap();
    }
    let neutral = neutral.finish().unwrap();
    assert_eq!(neutral.resources().len(), 2);
    let mut http = ApplicationBuilder::new(neutral.clone());
    http.bind("users.get", binding(HttpMethod::Get, "/items/{id}"))
        .unwrap();
    for (method, path) in [
        (HttpMethod::Get, "/items/{id}"),
        (HttpMethod::Post, "/items/{key}"),
    ] {
        let error = http
            .bind("posts.get", binding(method, path))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("users.get") && error.contains("posts.get"),
            "{error}"
        );
    }
    http.bind("posts.get", binding(HttpMethod::Post, "/items/{id}"))
        .unwrap();
    let description = http.finish().unwrap();
    assert_eq!(description.application(), &neutral);
    assert_eq!(description.plans().len(), 2);
}
