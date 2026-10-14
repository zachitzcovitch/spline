use spline_contract::{ApplicationBuilder, ContractBuilder, Error, OperationKind};
use spline_schema::{Context, Field, Object, Schema, Shape, TypeInfo, UnknownFields};

struct Shared;
impl Schema for Shared {
    fn info() -> TypeInfo {
        TypeInfo::named("app.Shared")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::String)
    }
}
struct Impostor;
impl Schema for Impostor {
    fn info() -> TypeInfo {
        Shared::info()
    }
    fn describe(cx: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Shared::describe(cx)
    }
}
struct Input;
impl Schema for Input {
    fn info() -> TypeInfo {
        TypeInfo::named("app.Input")
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

fn operation(builder: &mut ContractBuilder) -> spline_contract::Result<()> {
    builder.operation::<Input, Shared, ()>("get", "lookup", OperationKind::Query, "Operation docs")
}

#[test]
fn resources_share_one_registry_and_are_order_independent() {
    let build = |groups: &[&str]| {
        let mut builder = ApplicationBuilder::new("Application docs");
        for group in groups {
            builder
                .resource(group, &format!("v1.{group}"), "Resource docs", operation)
                .unwrap();
        }
        builder.finish().unwrap()
    };
    let application = build(&["users", "posts"]);
    assert_eq!(application, build(&["posts", "users"]));
    assert_eq!(application.schemas().definitions().len(), 2);
    assert_eq!(application.documentation(), "Application docs");
    assert_eq!(
        application.resources()["users"].documentation(),
        "Resource docs"
    );
    assert_eq!(
        application
            .operations()
            .map(|op| op.id.as_str())
            .collect::<Vec<_>>(),
        ["v1.posts.get", "v1.users.get"]
    );
    assert_eq!(
        application.resources()["users"].operations()["get"].rust_name,
        "lookup"
    );
}

#[test]
fn equal_shapes_do_not_hide_type_identity_collisions_and_failure_is_atomic() {
    let mut builder = ApplicationBuilder::new("");
    builder.resource("users", "users", "", operation).unwrap();
    let error = builder.resource("posts", "posts", "", |resource| {
        resource.operation::<Input, Vec<Impostor>, ()>("get", "get", OperationKind::Query, "")
    });
    assert!(matches!(
        error,
        Err(Error::Schema(spline_schema::Error::NameCollision { .. }))
    ));
    // A failed resource did not reserve its group, namespace, or schema owners.
    builder.resource("posts", "posts", "", operation).unwrap();
    let application = builder.finish().unwrap();
    assert_eq!(application.resources().len(), 2);
    assert_eq!(application.schemas().definitions().len(), 2);
}

#[test]
fn invalid_empty_and_duplicate_resources_are_rejected() {
    assert!(matches!(
        ApplicationBuilder::new("").finish(),
        Err(Error::EmptyApplication)
    ));
    let mut builder = ApplicationBuilder::new("");
    assert!(matches!(
        builder.resource("bad-name", "users", "", operation),
        Err(Error::InvalidIdentifier(_))
    ));
    assert!(matches!(
        builder.resource("users", "bad-name", "", operation),
        Err(Error::InvalidIdentifier(_))
    ));
    assert!(matches!(
        builder.resource("users", "users", "", |_| Ok(())),
        Err(Error::EmptyContract)
    ));
    builder
        .resource("users", "v1.users", "", operation)
        .unwrap();
    assert!(matches!(
        builder.resource("users", "posts", "", operation),
        Err(Error::DuplicateResource(_))
    ));
    assert!(matches!(
        builder.resource("people", "v1.users", "", operation),
        Err(Error::DuplicateNamespace(_))
    ));
    assert_eq!(builder.finish().unwrap().resources().len(), 1);
}

#[test]
fn a_late_resource_failure_does_not_commit_earlier_operations() {
    let mut builder = ApplicationBuilder::new("");
    assert!(builder
        .resource("failed", "failed", "", |resource| {
            operation(resource)?;
            resource.operation::<(), u64, ()>("bad", "bad", OperationKind::Query, "")
        })
        .is_err());
    builder
        .resource("good", "good", "", |r| {
            r.operation::<(), (), ()>("health", "health", OperationKind::Query, "")
        })
        .unwrap();
    assert!(builder.finish().unwrap().schemas().definitions().is_empty());
}
