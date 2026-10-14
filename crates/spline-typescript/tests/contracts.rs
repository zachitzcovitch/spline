use spline_contract::{ContractBuilder, OperationKind};
use spline_schema::{Context, DefinitionId, Direction, Name, Schema, Shape, TypeInfo};
use spline_typescript::{Error, Typescript};

struct Value;
impl Schema for Value {
    fn info() -> TypeInfo {
        TypeInfo::named("api.Value")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::String)
    }
}

#[test]
fn positional_signatures_follow_argument_order_not_alphabetical_order() {
    struct Input;
    impl Schema for Input {
        fn info() -> TypeInfo {
            TypeInfo::named("api.Arguments")
        }
        fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
            Ok(Shape::Object(spline_schema::Object {
                fields: vec![
                    spline_schema::Field::required("zebra", Shape::String),
                    spline_schema::Field::required("alpha", Shape::Boolean),
                ],
                unknown_fields: spline_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("ordered", "").unwrap();
    builder
        .operation::<Input, String, String>("inspect", "inspect", OperationKind::Query, "")
        .unwrap();
    let artifact = Typescript::new()
        .export_contract(&builder.finish().unwrap(), "Ordered")
        .unwrap();
    assert!(artifact
        .source
        .contains("inspect(zebra: string, alpha: boolean)"));
}

#[test]
fn keyword_and_runtime_shadowing_parameters_receive_local_safe_names() {
    struct Input;
    impl Schema for Input {
        fn info() -> TypeInfo {
            TypeInfo::inline("test.Input")
        }
        fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
            Ok(Shape::Object(spline_schema::Object {
                fields: ["call", "arg0", "default", "arg2"]
                    .into_iter()
                    .map(|name| spline_schema::Field::required(name, Shape::String))
                    .collect(),
                unknown_fields: spline_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("safe", "").unwrap();
    builder
        .operation::<Input, (), ()>("inspect", "inspect", OperationKind::Query, "")
        .unwrap();
    builder
        .http(
            "inspect",
            spline_contract::http::HttpBinding {
                method: spline_contract::http::HttpMethod::Get,
                path: "/safe".into(),
                body: None,
                success_status: 200,
            },
        )
        .unwrap();
    let source = Typescript::new()
        .export_http_client(&builder.finish().unwrap(), "Safe")
        .unwrap()
        .source;
    assert!(source.contains("inspect(arg0_: string, arg0: string, arg2_: string, arg2: string)"));
    assert!(source.contains("[\"call\"]: arg0_"));
    assert!(source.contains("[\"default\"]: arg2_"));
}

#[test]
fn http_runtime_insertion_does_not_rewrite_user_literals_as_template_markers() {
    struct Input;
    impl Schema for Input {
        fn info() -> TypeInfo {
            TypeInfo::named("api.Marker")
        }
        fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
            Ok(Shape::Object(spline_schema::Object {
                fields: vec![spline_schema::Field::required(
                    "value",
                    Shape::Literal("/*__METHODS__*/".into()),
                )],
                unknown_fields: spline_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("marker", "").unwrap();
    builder
        .operation::<Input, String, String>("send", "send", OperationKind::Mutation, "")
        .unwrap();
    builder
        .http(
            "send",
            spline_contract::http::HttpBinding {
                method: spline_contract::http::HttpMethod::Post,
                path: "/marker".into(),
                body: Some("value".into()),
                success_status: 200,
            },
        )
        .unwrap();
    let artifact = Typescript::new()
        .export_http_client(&builder.finish().unwrap(), "Marker")
        .unwrap();
    assert!(artifact
        .source
        .contains("[\"literal\", \"/*__METHODS__*/\"]"));
}

fn contract(reverse: bool) -> spline_contract::ContractDescription {
    let mut builder = ContractBuilder::new("users", "User operations").unwrap();
    for name in if reverse {
        ["create", "get"]
    } else {
        ["get", "create"]
    } {
        builder
            .operation::<Value, Value, Value>(
                name,
                name,
                if name == "get" {
                    OperationKind::Query
                } else {
                    OperationKind::Mutation
                },
                "docs */ stay in a comment",
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

#[test]
fn operation_exports_are_deterministic_and_preserve_kind_and_types() {
    let exporter = Typescript::new();
    let first = exporter.export_contract(&contract(false), "Users").unwrap();
    assert_eq!(
        first,
        exporter.export_contract(&contract(true), "Users").unwrap()
    );
    assert!(first.source.contains("\"users.get\": { kind: \"query\""));
    assert!(first
        .source
        .contains("\"users.create\": { kind: \"mutation\""));
    assert!(first
        .source
        .contains("input: Value; output: Value; error: Value"));
    assert!(first
        .source
        .contains("get(input: Value): Promise<UsersResult<Value, Value, Failure>>"));
    assert!(first
        .source
        .contains("export type UsersCalls<Failure = never>"));
}

#[test]
fn schema_overrides_cannot_shadow_call_surface_or_type_parameters() {
    let contract = contract(false);
    let id = DefinitionId {
        name: Name::new("api.Value"),
        direction: Direction::Serialize,
    };
    for name in [
        "Promise",
        "Failure",
        "UsersCalls",
        "UsersOperations",
        "UsersResult",
    ] {
        assert!(matches!(
            Typescript::new()
                .name(id.clone(), name)
                .export_contract(&contract, "Users"),
            Err(Error::SymbolCollision(_))
        ));
    }
    assert!(matches!(
        Typescript::new().export_contract(&contract, "not-a-symbol"),
        Err(Error::InvalidSymbol(_))
    ));
}

#[test]
fn no_content_signatures_are_void_without_changing_the_json_unit_shape() {
    struct Empty;
    impl Schema for Empty {
        fn info() -> TypeInfo {
            TypeInfo::inline("api.EmptyArguments")
        }
        fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
            Ok(Shape::Object(spline_schema::Object {
                fields: vec![],
                unknown_fields: spline_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("empty", "").unwrap();
    builder
        .operation::<Empty, (), Value>("delete", "delete", OperationKind::Mutation, "")
        .unwrap();
    builder
        .http(
            "delete",
            spline_contract::http::HttpBinding {
                method: spline_contract::http::HttpMethod::Delete,
                path: "/empty".into(),
                body: None,
                success_status: 204,
            },
        )
        .unwrap();
    builder
        .operation::<Empty, (), String>("json", "json", OperationKind::Query, "")
        .unwrap();
    builder
        .http(
            "json",
            spline_contract::http::HttpBinding {
                method: spline_contract::http::HttpMethod::Get,
                path: "/json".into(),
                body: None,
                success_status: 200,
            },
        )
        .unwrap();
    let description = builder.finish().unwrap();
    assert_eq!(description.operations()["delete"].output, Shape::Null);
    let source = Typescript::new()
        .export_http_client(&description, "Empty")
        .unwrap()
        .source;
    assert!(source.contains("delete(): Promise<EmptyResult<void, Value, Failure>>"));
    assert!(source.contains("json(): Promise<EmptyResult<null, string, Failure>>"));
    // Generated result utility names participate in the same checks as DTOs.
    let key = DefinitionId {
        name: Name::new("api.Value"),
        direction: Direction::Serialize,
    };
    let source = Typescript::new().name(key, "EmptyResult");
    assert!(matches!(
        source.export_http_client(&description, "Empty"),
        Err(Error::SymbolCollision(_))
    ));
}
