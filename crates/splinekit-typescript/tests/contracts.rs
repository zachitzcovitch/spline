use splinekit_contract::{ContractBuilder, OperationKind};
use splinekit_http_contract::{ContractBuilder as HttpBuilder, HttpBinding, HttpMethod};
use splinekit_schema::{Context, DefinitionId, Direction, Name, Schema, Shape, TypeInfo};
use splinekit_typescript::{Error, Typescript};

/// A minimal HTTP application error: `{"code": "failed"}`.
struct AppError;
impl splinekit_schema::Schema for AppError {
    fn info() -> splinekit_schema::TypeInfo {
        splinekit_schema::TypeInfo::named("test.AppError")
    }
    fn describe(
        _: &mut splinekit_schema::Context<'_>,
    ) -> splinekit_schema::Result<splinekit_schema::Shape> {
        Ok(splinekit_schema::Shape::Object(splinekit_schema::Object {
            fields: vec![splinekit_schema::Field::required(
                "code",
                splinekit_schema::Shape::Literal("failed".into()),
            )],
            unknown_fields: splinekit_schema::UnknownFields::Reject,
        }))
    }
}

struct Value;
impl Schema for Value {
    fn info() -> TypeInfo {
        TypeInfo::named("api.Value")
    }
    fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
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
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: vec![
                    splinekit_schema::Field::required("zebra", Shape::String),
                    splinekit_schema::Field::required("alpha", Shape::Boolean),
                ],
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("ordered", "").unwrap();
    builder
        .operation::<Input, String, AppError>("inspect", "inspect", OperationKind::Query, "")
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
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: ["call", "arg0", "default", "arg2"]
                    .into_iter()
                    .map(|name| splinekit_schema::Field::required(name, Shape::String))
                    .collect(),
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("safe", "").unwrap();
    builder
        .operation::<Input, (), AppError>("inspect", "inspect", OperationKind::Query, "")
        .unwrap();
    let mut builder = HttpBuilder::new(builder.finish().unwrap());
    builder
        .bind(
            "safe.inspect",
            HttpBinding {
                method: HttpMethod::Get,
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
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: vec![splinekit_schema::Field::required(
                    "value",
                    Shape::Literal("/*__METHODS__*/".into()),
                )],
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("marker", "").unwrap();
    builder
        .operation::<Input, String, AppError>("send", "send", OperationKind::Mutation, "")
        .unwrap();
    let mut builder = HttpBuilder::new(builder.finish().unwrap());
    builder
        .bind(
            "marker.send",
            HttpBinding {
                method: HttpMethod::Post,
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

fn contract(reverse: bool) -> splinekit_contract::ContractDescription {
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
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: vec![],
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("empty", "").unwrap();
    builder
        .operation::<Empty, (), AppError>("delete", "delete", OperationKind::Mutation, "")
        .unwrap();
    builder
        .operation::<Empty, (), AppError>("json", "json", OperationKind::Query, "")
        .unwrap();
    let neutral = builder.finish().unwrap();
    assert_eq!(neutral.operations()["delete"].output, Shape::Null);
    let neutral_source = Typescript::new()
        .export_contract(&neutral, "Empty")
        .unwrap()
        .source;
    assert!(neutral_source.contains("delete(): Promise<EmptyResult<null, AppError, Failure>>"));
    let mut builder = HttpBuilder::new(neutral);
    builder
        .bind(
            "empty.delete",
            HttpBinding {
                method: HttpMethod::Delete,
                path: "/empty".into(),
                body: None,
                success_status: 204,
            },
        )
        .unwrap();
    builder
        .bind(
            "empty.json",
            HttpBinding {
                method: HttpMethod::Get,
                path: "/json".into(),
                body: None,
                success_status: 200,
            },
        )
        .unwrap();
    let description = builder.finish().unwrap();
    let source = Typescript::new()
        .export_http_client(&description, "Empty")
        .unwrap()
        .source;
    assert!(source.contains("delete(): Promise<EmptyResult<void, AppError, Failure>>"));
    assert!(source.contains("json(): Promise<EmptyResult<null, AppError, Failure>>"));
    // Generated result utility names participate in the same checks as DTOs.
    let key = DefinitionId {
        name: Name::new("test.AppError"),
        direction: Direction::Serialize,
    };
    let source = Typescript::new().name(key, "EmptyResult");
    assert!(matches!(
        source.export_http_client(&description, "Empty"),
        Err(Error::SymbolCollision(_))
    ));
}

#[test]
fn api_exports_use_camel_case_rust_names_while_type_exports_keep_the_wire() {
    #[derive(splinekit::Schema)]
    #[splinekit(name = "api.Account")]
    #[allow(dead_code)]
    struct Account {
        display_name: String,
        #[serde(rename = "type")]
        kind: String,
        _private_flag: bool,
        r#ref: String,
    }
    let mut builder = ContractBuilder::new("accounts", "").unwrap();
    builder
        .operation::<Input, Account, AppError>("get", "get", OperationKind::Query, "")
        .unwrap();
    let contract = builder.finish().unwrap();
    let api = Typescript::new()
        .export_contract(&contract, "Accounts")
        .unwrap()
        .source;
    for property in ["\"displayName\"", "\"kind\"", "\"_privateFlag\"", "\"ref\""] {
        assert!(api.contains(property), "missing {property}:\n{api}");
    }
    assert!(!api.contains("\"display_name\""), "{api}");

    let mut registry = splinekit_schema::Registry::new();
    let root = registry.register::<Account>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    let types = Typescript::new()
        .export(
            &graph,
            &[splinekit_typescript::Root::new(
                "Root",
                Direction::Serialize,
                root,
            )],
        )
        .unwrap()
        .source;
    for property in [
        "\"display_name\"",
        "\"type\"",
        "\"_private_flag\"",
        "\"ref\"",
    ] {
        assert!(types.contains(property), "missing {property}:\n{types}");
    }

    struct Input;
    impl Schema for Input {
        fn info() -> TypeInfo {
            TypeInfo::named("api.Empty")
        }
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: vec![],
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
}

#[test]
fn fields_that_would_share_a_typescript_name_are_refused_in_api_exports() {
    #[derive(splinekit::Schema)]
    #[splinekit(name = "api.Clash")]
    #[allow(dead_code, non_snake_case)]
    struct Clash {
        user_id: String,
        #[serde(rename = "userId2")]
        userId: String,
    }
    struct Empty;
    impl Schema for Empty {
        fn info() -> TypeInfo {
            TypeInfo::named("api.Nothing")
        }
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: vec![],
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("clash", "").unwrap();
    builder
        .operation::<Empty, Clash, AppError>("get", "get", OperationKind::Query, "")
        .unwrap();
    assert!(matches!(
        Typescript::new().export_contract(&builder.finish().unwrap(), "Clash"),
        Err(Error::InvalidShape(message)) if message.contains("userId")
    ));
}

#[test]
fn operations_groups_and_parameters_are_camel_case_and_must_stay_distinct() {
    struct Args;
    impl Schema for Args {
        fn info() -> TypeInfo {
            TypeInfo::named("api.LookupArgs")
        }
        fn describe(_: &mut Context<'_>) -> splinekit_schema::Result<Shape> {
            Ok(Shape::Object(splinekit_schema::Object {
                fields: vec![splinekit_schema::Field::required("user_id", Shape::String)],
                unknown_fields: splinekit_schema::UnknownFields::Reject,
            }))
        }
    }
    let mut builder = ContractBuilder::new("lookups", "").unwrap();
    builder
        .operation::<Args, String, AppError>("find_user", "find_user", OperationKind::Query, "")
        .unwrap();
    let source = Typescript::new()
        .export_contract(&builder.finish().unwrap(), "Lookups")
        .unwrap()
        .source;
    assert!(source.contains("findUser(userId: string)"), "{source}");
    assert!(!source.contains("find_user("), "{source}");

    let mut builder = ContractBuilder::new("clash", "").unwrap();
    builder
        .operation::<Args, String, AppError>("get_user", "get_user", OperationKind::Query, "")
        .unwrap();
    builder
        .operation::<Args, String, AppError>("getUser", "getUser", OperationKind::Query, "")
        .unwrap();
    assert!(matches!(
        Typescript::new().export_contract(&builder.finish().unwrap(), "Clash"),
        Err(Error::SymbolCollision(name)) if name == "getUser"
    ));
}
