mod support;

use spline_schema::*;
use support::*;

#[test]
fn generic_arguments_and_directions_have_distinct_identities() {
    let mut registry = Registry::new();
    let users = registry
        .register::<Page<User>>(Direction::Serialize)
        .unwrap();
    let posts = registry
        .register::<Page<Post>>(Direction::Serialize)
        .unwrap();
    let input = registry
        .register::<Page<User>>(Direction::Deserialize)
        .unwrap();
    assert_ne!(users, posts);
    assert_ne!(users, input);
    assert_eq!(
        users,
        registry
            .register::<Page<User>>(Direction::Serialize)
            .unwrap()
    );
    let graph = registry.finish().unwrap();
    assert!(graph
        .get(Page::<User>::info().name, Direction::Serialize)
        .is_some());
    assert!(graph
        .get(Page::<Post>::info().name, Direction::Serialize)
        .is_some());
    assert_eq!(
        Page::<User>::info().name.to_string(),
        "demo.Page<demo.User>"
    );
    let post_page = serde_json::to_value(Page {
        items: vec![Post {
            title: "A post".into(),
        }],
    })
    .unwrap();
    assert!(matches(&graph, &posts, &post_page));
    assert!(!matches(&graph, &users, &post_page));
}

#[test]
fn finished_graphs_are_equal_across_registration_orders() {
    let mut first = Registry::new();
    first.register::<Page<User>>(Direction::Serialize).unwrap();
    first
        .register::<Page<Post>>(Direction::Deserialize)
        .unwrap();
    first.register::<Node>(Direction::Serialize).unwrap();
    let mut second = Registry::new();
    second.register::<Node>(Direction::Serialize).unwrap();
    second
        .register::<Page<Post>>(Direction::Deserialize)
        .unwrap();
    second.register::<Page<User>>(Direction::Serialize).unwrap();
    assert_eq!(first.finish().unwrap(), second.finish().unwrap());
}

#[test]
fn mutual_recursion_uses_reserved_named_references() {
    let mut registry = Registry::new();
    registry.register::<Left>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    assert_eq!(graph.definitions().len(), 2);
    assert!(graph.get("demo.Left", Direction::Serialize).is_some());
    assert!(graph.get("demo.Right", Direction::Serialize).is_some());
}

#[test]
fn logical_name_collisions_fail_instead_of_renaming_by_visit_order() {
    struct First;
    struct Second;
    impl Schema for First {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Collision")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::String)
        }
    }
    impl Schema for Second {
        fn info() -> TypeInfo {
            First::info()
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::Boolean)
        }
    }
    let mut registry = Registry::new();
    registry.register::<First>(Direction::Serialize).unwrap();
    let before = registry.clone().finish().unwrap();
    assert!(matches!(
        registry.register::<Second>(Direction::Serialize),
        Err(Error::NameCollision { .. })
    ));
    assert_eq!(registry.finish().unwrap(), before);
}

#[test]
fn failed_registration_does_not_leave_nested_definitions_or_names() {
    struct Failing;
    impl Schema for Failing {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Failing")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            cx.schema::<Post>()?;
            Err(Error::Unsupported(
                "test failure after registering a child".into(),
            ))
        }
    }
    let mut registry = Registry::new();
    registry.register::<UserId>(Direction::Serialize).unwrap();
    let before = registry.clone().finish().unwrap();
    assert!(registry.register::<Failing>(Direction::Serialize).is_err());
    assert_eq!(registry.clone().finish().unwrap(), before);
    registry.register::<Post>(Direction::Serialize).unwrap();
    assert_eq!(registry.finish().unwrap().definitions().len(), 2);
}

#[test]
fn swallowing_a_context_error_cannot_commit_a_partial_graph() {
    struct Swallow;
    impl Schema for Swallow {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Swallow")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            let _ = cx.schema::<Post>()?;
            let _ = cx.field::<u64>(
                "count",
                FieldRules {
                    custom_codec: true,
                    ..FieldRules::default()
                },
            );
            Ok(Shape::String)
        }
    }
    let mut registry = Registry::new();
    assert_eq!(
        registry.register::<Swallow>(Direction::Serialize),
        Err(Error::MissingCodec("count".into()))
    );
    assert!(registry.finish().unwrap().definitions().is_empty());
}

#[test]
fn panicking_description_also_leaves_the_original_registry_untouched() {
    struct Panics;
    impl Schema for Panics {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Panics")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            cx.schema::<Post>()?;
            panic!("test panic after registering a child");
        }
    }
    let mut registry = Registry::new();
    registry.register::<UserId>(Direction::Serialize).unwrap();
    let before = registry.clone().finish().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registry.register::<Panics>(Direction::Serialize)
    }));
    assert!(result.is_err());
    assert_eq!(registry.finish().unwrap(), before);
}

#[test]
fn inline_recursion_requires_a_named_definition() {
    struct InlineLoop;
    impl Schema for InlineLoop {
        fn info() -> TypeInfo {
            TypeInfo::inline("demo.InlineLoop")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            cx.schema::<Self>()
        }
    }
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register::<InlineLoop>(Direction::Serialize),
        Err(Error::InlineRecursion(_))
    ));
    assert!(registry.finish().unwrap().definitions().is_empty());
}

#[test]
fn missing_and_cross_direction_references_are_rejected() {
    struct Dangling;
    struct CrossDirection;
    impl Schema for Dangling {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Dangling")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::Ref(DefinitionId {
                name: Name::new("demo.Absent"),
                direction: cx.direction(),
            }))
        }
    }
    impl Schema for CrossDirection {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.CrossDirection")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::Ref(DefinitionId {
                name: UserId::info().name,
                direction: Direction::Deserialize,
            }))
        }
    }
    let mut registry = Registry::new();
    registry.register::<UserId>(Direction::Deserialize).unwrap();
    let before = registry.clone().finish().unwrap();
    assert!(matches!(
        registry.register::<Dangling>(Direction::Serialize),
        Err(Error::UnresolvedReference(_))
    ));
    assert!(matches!(
        registry.register::<CrossDirection>(Direction::Serialize),
        Err(Error::DirectionMismatch(_))
    ));
    assert_eq!(registry.finish().unwrap(), before);
}

#[test]
fn malformed_shapes_and_names_cannot_enter_a_finished_graph() {
    struct EmptyUnion;
    struct Duplicate;
    struct WideInteger;
    struct InvalidName;
    impl Schema for EmptyUnion {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.EmptyUnion")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::Union(vec![]))
        }
    }
    impl Schema for Duplicate {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Duplicate")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(object(vec![
                Field::required("x", Shape::String),
                Field::required("x", Shape::Null),
            ]))
        }
    }
    impl Schema for WideInteger {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.WideInteger")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::Integer {
                signed: false,
                bits: 64,
            })
        }
    }
    impl Schema for InvalidName {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Invalid<Name>")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::String)
        }
    }
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register::<EmptyUnion>(Direction::Serialize),
        Err(Error::InvalidShape(_))
    ));
    assert!(matches!(
        registry.register::<Duplicate>(Direction::Serialize),
        Err(Error::InvalidShape(_))
    ));
    assert!(matches!(
        registry.register::<WideInteger>(Direction::Serialize),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        registry.register::<InvalidName>(Direction::Serialize),
        Err(Error::InvalidName(_))
    ));
    assert!(registry.finish().unwrap().definitions().is_empty());
}

#[test]
fn generic_field_builder_does_not_guess_an_option_projection() {
    struct UnknownOption;
    impl Schema for UnknownOption {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.UnknownOption")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            let rules = FieldRules {
                omission: Omission::IfNone,
                ..FieldRules::default()
            };
            let field = cx.field::<Option<String>>("value", rules)?.unwrap();
            Ok(object(vec![field]))
        }
    }
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register::<UnknownOption>(Direction::Serialize),
        Err(Error::InvalidField(_))
    ));
    assert!(registry.finish().unwrap().definitions().is_empty());
}
