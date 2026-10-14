use spline_contract::{ApplicationBuilder, ApplicationDescription, OperationKind};
use spline_http_contract::{ApplicationBuilder as HttpBuilder, HttpBinding, HttpMethod};
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

fn application(resources: &[(&str, &str)]) -> ApplicationDescription {
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
                Ok(())
            })
            .unwrap();
    }
    application.finish().unwrap()
}

fn http(application: ApplicationDescription) -> spline_http_contract::ApplicationDescription {
    let mut builder = HttpBuilder::new(application.clone());
    for (group, resource) in application.resources() {
        builder
            .bind(
                &resource.operations()["get"].id,
                HttpBinding {
                    method: HttpMethod::Get,
                    path: format!("/{group}"),
                    body: None,
                    success_status: 200,
                },
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

#[test]
fn grouped_exports_are_deterministic_and_share_one_schema_and_runtime() {
    let exporter = Typescript::new();
    let first = application(&[("users", "v1.users"), ("posts", "v1.posts")]);
    let reversed = application(&[("posts", "v1.posts"), ("users", "v1.users")]);
    assert_eq!(
        exporter.export_application(&first, "Api").unwrap(),
        exporter.export_application(&reversed, "Api").unwrap()
    );
    let artifact = exporter
        .export_http_application(&http(first), "Api")
        .unwrap();
    assert_eq!(
        artifact,
        exporter
            .export_http_application(&http(reversed), "Api")
            .unwrap()
    );
    assert_eq!(artifact.source.matches("export type Shared =").count(), 1);
    assert_eq!(artifact.source.matches("const call = async").count(), 1);
    assert!(artifact.source.contains("\"users\": {"));
    assert!(artifact
        .source
        .contains("\"v1.users.get\": { kind: \"query\""));
    assert!(artifact.source.contains("Resource docs"));
}

#[test]
fn application_symbols_and_root_aliases_are_checked_together() {
    let application = http(application(&[("users", "v1.users"), ("posts", "v1.posts")]));
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
        "ApiResult",
        "ApiClient",
        "__SplineWire",
        "__SplinePlan",
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
    let ambiguous = http(self::application(&[("first", "a.b"), ("second", "a_b")]));
    // Logical identities remain distinct even when legacy underscore flattening
    // would have produced colliding operation aliases.
    let source = Typescript::new()
        .export_http_application(&ambiguous, "Api")
        .unwrap()
        .source;
    assert!(source.contains("\"a.b.get\""));
    assert!(source.contains("\"a_b.get\""));
}

#[test]
fn transport_neutral_applications_export_signatures_but_require_bindings_to_execute() {
    let application = application(&[("people", "v1.users")]);
    assert!(Typescript::new()
        .export_application(&application, "Api")
        .is_ok());
    assert!(HttpBuilder::new(application)
        .finish()
        .unwrap_err()
        .to_string()
        .contains("v1.users.get has no HTTP binding"));
}

#[test]
fn group_names_are_quoted_and_computed_not_interpreted_as_javascript_prototypes() {
    let application = application(&[("__proto__", "v1.users"), ("constructor", "v1.posts")]);
    let source = Typescript::new()
        .export_http_application(&http(application), "Api")
        .unwrap()
        .source;
    assert!(source.contains("[\"__proto__\"]: {"));
    assert!(source.contains("[\"constructor\"]: {"));
}
