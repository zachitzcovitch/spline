use spline_contract::{
    http::{HttpBinding, HttpMethod},
    ApplicationBuilder, ApplicationDescription, OperationKind,
};
use spline_schema::{
    Context, DefinitionId, Direction, Object, Schema, Shape, TypeInfo, UnknownFields,
};
use spline_typescript::{Error, Typescript};

struct Shared;
impl Schema for Shared {
    fn info() -> TypeInfo {
        TypeInfo::named("test.Shared")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::String)
    }
}
struct Input;
impl Schema for Input {
    fn info() -> TypeInfo {
        TypeInfo::inline("test.Empty")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::Object(Object {
            fields: vec![],
            unknown_fields: UnknownFields::Reject,
        }))
    }
}

fn application(resources: &[(&str, &str)], bound: bool) -> ApplicationDescription {
    let mut application = ApplicationBuilder::new("Application docs */ remain escaped");
    for (group, namespace) in resources {
        application
            .resource(group, namespace, "Resource docs", |resource| {
                resource.operation::<Input, Shared, ()>(
                    "get",
                    "get",
                    OperationKind::Query,
                    "Operation docs",
                )?;
                if bound {
                    resource.http(
                        "get",
                        HttpBinding {
                            method: HttpMethod::Get,
                            path: format!("/{group}"),
                            body: None,
                            success_status: 200,
                        },
                    )?;
                }
                Ok(())
            })
            .unwrap();
    }
    application.finish().unwrap()
}

#[test]
fn grouped_exports_are_deterministic_and_share_one_schema_and_runtime() {
    let exporter = Typescript::new();
    let first = application(&[("users", "v1.users"), ("posts", "v1.posts")], true);
    let reversed = application(&[("posts", "v1.posts"), ("users", "v1.users")], true);
    assert_eq!(
        exporter.export_application(&first, "Api").unwrap(),
        exporter.export_application(&reversed, "Api").unwrap()
    );
    let artifact = exporter.export_http_application(&first, "Api").unwrap();
    assert_eq!(
        artifact,
        exporter.export_http_application(&reversed, "Api").unwrap()
    );
    assert_eq!(
        artifact
            .source
            .matches("export type test_Shared_Output =")
            .count(),
        1
    );
    assert_eq!(artifact.source.matches("const call = async").count(), 1);
    assert!(artifact.source.contains("\"users\": {"));
    assert!(artifact
        .source
        .contains("\"v1.users.get\": { kind: \"query\""));
    assert!(artifact.source.contains("Resource docs"));
}

#[test]
fn application_symbols_and_root_aliases_are_checked_together() {
    let application = application(&[("users", "v1.users"), ("posts", "v1.posts")], true);
    let id = DefinitionId {
        name: "test.Shared".into(),
        direction: Direction::Serialize,
    };
    for symbol in [
        "ApiCalls",
        "ApiOperations",
        "ApiHttpFailure",
        "ApiHttpOptions",
        "createApiClient",
        "Promise",
        "Failure",
        "v1_posts_get_Input",
    ] {
        assert!(
            matches!(
                Typescript::new()
                    .name(id.clone(), symbol)
                    .export_http_application(&application, "Api"),
                Err(Error::SymbolCollision(_))
            ),
            "{symbol}"
        );
    }
    let ambiguous = self::application(&[("first", "a.b"), ("second", "a_b")], true);
    assert!(matches!(
        Typescript::new().export_http_application(&ambiguous, "Api"),
        Err(Error::SymbolCollision(_))
    ));
}

#[test]
fn transport_neutral_applications_export_signatures_but_require_bindings_to_execute() {
    let application = application(&[("people", "v1.users")], false);
    assert!(Typescript::new()
        .export_application(&application, "Api")
        .is_ok());
    assert!(
        matches!(Typescript::new().export_http_application(&application, "Api"), Err(Error::MissingHttpBinding(id)) if id == "v1.users.get")
    );
}

#[test]
fn group_names_are_quoted_and_computed_not_interpreted_as_javascript_prototypes() {
    let application = application(
        &[("__proto__", "v1.users"), ("constructor", "v1.posts")],
        true,
    );
    let source = Typescript::new()
        .export_http_application(&application, "Api")
        .unwrap()
        .source;
    assert!(source.contains("[\"__proto__\"]: {"));
    assert!(source.contains("[\"constructor\"]: {"));
}
