use spline_contract::{
    http::{HttpBinding, HttpMethod, Scalar},
    ContractBuilder, Error, OperationKind,
};
use spline_schema::{Context, Field, Object, Presence, Schema, Shape, TypeInfo, UnknownFields};

struct Input;
impl Schema for Input {
    fn info() -> TypeInfo {
        TypeInfo::named("test.Request")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::Object(Object {
            fields: vec![
                Field::required("id", Shape::String),
                Field::required(
                    "count",
                    Shape::Integer {
                        signed: false,
                        bits: 32,
                    },
                ),
                Field {
                    name: "note".into(),
                    shape: Shape::Union(vec![Shape::String, Shape::Null]),
                    presence: Presence::Optional,
                    documentation: vec![],
                },
                Field::required(
                    "input",
                    Shape::Object(Object {
                        fields: vec![],
                        unknown_fields: UnknownFields::Reject,
                    }),
                ),
            ],
            unknown_fields: UnknownFields::Reject,
        }))
    }
}

fn binding(path: &str) -> HttpBinding {
    HttpBinding {
        method: HttpMethod::Post,
        path: path.into(),
        body: Some("input".into()),
        success_status: 201,
    }
}

fn builder() -> ContractBuilder {
    let mut builder = ContractBuilder::new("test", "").unwrap();
    builder
        .operation::<Input, String, String>("first", "first", OperationKind::Mutation, "")
        .unwrap();
    builder
}

#[test]
fn binding_plans_preserve_authored_order_and_classify_url_arguments() {
    let mut builder = builder();
    builder.http("first", binding("/items/{id}")).unwrap();
    let description = builder.finish().unwrap();
    let operation = &description.operations()["first"];
    assert_eq!(
        operation
            .arguments
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "count", "note", "input"]
    );
    let plan = operation.http.as_ref().unwrap();
    assert_eq!(plan.path_parameters[0].name, "id");
    assert_eq!(plan.path_parameters[0].scalar, Scalar::String);
    assert_eq!(plan.query_parameters[0].name, "count");
    assert_eq!(
        plan.query_parameters[0].scalar,
        Scalar::Integer {
            signed: false,
            bits: 32
        }
    );
    assert!(plan.query_parameters[1].optional);
}

#[test]
fn invalid_bindings_fail_without_mutating_the_descriptor() {
    let mut builder = builder();
    for path in [
        "relative",
        "/items/{missing}",
        "/items/{id}/{id}",
        "/items/*rest",
        "/items/../x",
        "/items?x=1",
    ] {
        assert!(matches!(
            builder.http("first", binding(path)),
            Err(Error::Http(_))
        ));
    }
    let mut wrong_body = binding("/items/{id}");
    wrong_body.body = Some("id".into());
    assert!(builder.http("first", wrong_body).is_err());
    let mut unmapped = binding("/items/{id}");
    unmapped.body = None;
    assert!(builder.http("first", unmapped).is_err()); // object-valued URL argument
    let mut get_mutation = binding("/items/{id}");
    get_mutation.method = HttpMethod::Get;
    get_mutation.body = None;
    assert!(builder.http("first", get_mutation).is_err());
    builder.http("first", binding("/items/{id}")).unwrap();
    assert!(builder.http("first", binding("/other/{id}")).is_err());
}

#[test]
fn ambiguous_dynamic_routes_are_rejected_before_axum_registration() {
    let mut builder = builder();
    builder
        .operation::<Input, String, String>("second", "second", OperationKind::Mutation, "")
        .unwrap();
    builder.http("first", binding("/items/{id}")).unwrap();
    assert!(builder.http("second", binding("/items/{count}")).is_err());
    assert!(builder.http("second", binding("/items/{id}")).is_err());
}

#[test]
fn new_verbs_share_a_route_and_validate_their_request_body_profile() {
    let mut builder = ContractBuilder::new("test", "").unwrap();
    for (name, method) in [("replace", HttpMethod::Put), ("update", HttpMethod::Patch)] {
        builder
            .operation::<Input, String, String>(name, name, OperationKind::Mutation, "")
            .unwrap();
        let mut plan = binding("/items/{id}");
        plan.method = method;
        let mut missing_body = plan.clone();
        missing_body.body = None;
        assert!(builder.http(name, missing_body).is_err());
        builder.http(name, plan).unwrap();
    }
    let description = builder.finish().unwrap();
    assert_eq!(
        description.operations()["replace"]
            .http
            .as_ref()
            .unwrap()
            .binding
            .method,
        HttpMethod::Put
    );
    assert_eq!(
        description.operations()["update"]
            .http
            .as_ref()
            .unwrap()
            .binding
            .method,
        HttpMethod::Patch
    );
}

#[test]
fn no_content_requires_a_null_output_and_delete_is_bodyless() {
    struct Id;
    impl Schema for Id {
        fn info() -> TypeInfo {
            TypeInfo::named("test.IdArgument")
        }
        fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
            Ok(Shape::Object(Object {
                fields: vec![Field::required("id", Shape::String)],
                unknown_fields: UnknownFields::Reject,
            }))
        }
    }
    let binding = HttpBinding {
        method: HttpMethod::Delete,
        path: "/items/{id}".into(),
        body: None,
        success_status: 204,
    };
    let mut builder = ContractBuilder::new("test", "").unwrap();
    builder
        .operation::<Id, String, String>("invalid", "invalid", OperationKind::Mutation, "")
        .unwrap();
    assert!(builder
        .http("invalid", binding.clone())
        .unwrap_err()
        .to_string()
        .contains("204 requires"));
    builder
        .operation::<Id, (), String>("delete", "delete", OperationKind::Mutation, "")
        .unwrap();
    let mut body = binding.clone();
    body.body = Some("id".into());
    assert!(builder.http("delete", body).is_err());
    builder.http("delete", binding.clone()).unwrap();
    builder
        .operation::<Id, (), String>("read", "read", OperationKind::Query, "")
        .unwrap();
    assert!(builder.http("read", binding).is_err());
}
