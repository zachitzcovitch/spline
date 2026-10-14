#[path = "../fixtures/api.rs"]
mod api;

use spline_schema::{
    Context, DefinitionId, Direction, Field, Graph, Name, Object, Registry, Schema, Shape,
    TypeInfo, UnknownFields,
};
use spline_typescript::{Error, Root, Typescript};

fn id(name: &str, direction: Direction) -> DefinitionId {
    DefinitionId {
        name: Name::new(name),
        direction,
    }
}

#[test]
fn generated_types_and_serde_samples_match_checked_in_artifacts() {
    assert_eq!(
        api::artifact(false).unwrap().source,
        include_str!("../typescript/generated.ts")
    );
    assert_eq!(
        api::samples().unwrap(),
        include_str!("../typescript/wire-samples.ts")
    );
}

#[test]
fn registration_and_root_order_do_not_change_source_or_symbol_mapping() {
    assert_eq!(api::artifact(false).unwrap(), api::artifact(true).unwrap());
}

#[test]
fn adding_an_unrelated_type_keeps_existing_symbols_stable() {
    let mut registry = Registry::new();
    registry
        .register::<api::User>(Direction::Serialize)
        .unwrap();
    let before = Typescript::new()
        .export(&registry.clone().finish().unwrap(), &[])
        .unwrap();
    registry
        .register::<api::ApiError>(Direction::Serialize)
        .unwrap();
    let after = Typescript::new()
        .export(&registry.finish().unwrap(), &[])
        .unwrap();
    for (id, symbol) in before.symbols {
        assert_eq!(after.symbols[&id], symbol);
    }
}

#[test]
fn explicit_names_update_references_as_well_as_declarations() {
    let (graph, roots) = api::build(false).unwrap();
    let key = id("api.User", Direction::Serialize);
    let artifact = Typescript::new()
        .name(key.clone(), "UserOutput")
        .export(&graph, &roots)
        .unwrap();
    assert_eq!(artifact.symbols[&key], "UserOutput");
    assert!(artifact.source.contains("export type UserOutput ="));
    assert!(artifact.source.contains("\"items\": UserOutput[];"));
    assert!(!artifact.source.contains("export type UserOutput = User"));
}

#[test]
fn default_name_collisions_require_explicit_overrides() {
    #[derive(spline::Schema)]
    #[spline(name = "api.User")]
    struct Qualified;
    #[derive(spline::Schema)]
    #[spline(name = "other.User")]
    struct Flattened;
    let mut registry = Registry::new();
    registry
        .register::<Qualified>(Direction::Serialize)
        .unwrap();
    registry
        .register::<Flattened>(Direction::Serialize)
        .unwrap();
    let graph = registry.finish().unwrap();
    assert_eq!(
        Typescript::new().export(&graph, &[]),
        Err(Error::SymbolCollision("User".into()))
    );
    let artifact = Typescript::new()
        .name(id("other.User", Direction::Serialize), "FlattenedOutput")
        .export(&graph, &[])
        .unwrap();
    assert!(artifact
        .source
        .contains("export type FlattenedOutput = null;"));
}

#[test]
fn names_are_checked_across_overrides_definitions_and_roots() {
    let (graph, _) = api::build(false).unwrap();
    let user = id("api.User", Direction::Serialize);
    assert!(matches!(
        Typescript::new()
            .name(user.clone(), "string")
            .export(&graph, &[]),
        Err(Error::InvalidSymbol(_))
    ));
    assert!(matches!(
        Typescript::new().name(user, "UserId").export(&graph, &[]),
        Err(Error::SymbolCollision(_))
    ));
    assert!(matches!(
        Typescript::new()
            .name(id("api.Missing", Direction::Serialize), "Missing")
            .export(&graph, &[]),
        Err(Error::UnknownOverride(_))
    ));
    let root = Root::new("UserOutput", Direction::Serialize, Shape::String);
    assert!(matches!(
        Typescript::new().export(&graph, &[root]),
        Err(Error::SymbolCollision(_))
    ));
    for symbol in [
        "",
        "9name",
        "bad-name",
        "export",
        "unknown",
        "x; export type Bad",
    ] {
        let root = Root::new(symbol, Direction::Serialize, Shape::String);
        assert!(matches!(
            Typescript::new().export(&Graph::default(), &[root]),
            Err(Error::InvalidSymbol(_))
        ));
    }
    let roots = [
        Root::new("Same", Direction::Serialize, Shape::String),
        Root::new("Same", Direction::Deserialize, Shape::Boolean),
    ];
    assert_eq!(
        Typescript::new().export(&Graph::default(), &roots),
        Err(Error::SymbolCollision("Same".into()))
    );
}

#[test]
fn root_references_must_exist_and_match_their_declared_direction() {
    let (graph, _) = api::build(false).unwrap();
    let missing = Root::new(
        "Missing",
        Direction::Serialize,
        Shape::Ref(id("absent.Type", Direction::Serialize)),
    );
    assert!(matches!(
        Typescript::new().export(&graph, &[missing]),
        Err(Error::MissingReference(_))
    ));
    let wrong = Root::new(
        "Wrong",
        Direction::Deserialize,
        Shape::Ref(id("api.User", Direction::Serialize)),
    );
    assert!(matches!(
        Typescript::new().export(&graph, &[wrong]),
        Err(Error::WrongDirection(_))
    ));
}

#[test]
fn unvalidated_inline_roots_cannot_bypass_the_json_profile() {
    let shapes = [
        Shape::Integer {
            signed: false,
            bits: 64,
        },
        Shape::Union(vec![]),
        Shape::Object(Object {
            fields: vec![
                Field::required("x", Shape::String),
                Field::required("x", Shape::Boolean),
            ],
            unknown_fields: UnknownFields::Reject,
        }),
    ];
    for shape in shapes {
        assert!(matches!(
            Typescript::new().export(
                &Graph::default(),
                &[Root::new("Invalid", Direction::Serialize, shape)]
            ),
            Err(Error::InvalidShape(_))
        ));
    }
}

struct AliasLoop;
impl Schema for AliasLoop {
    fn info() -> TypeInfo {
        TypeInfo::named("cycle.Loop")
    }
    fn describe(cx: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::Union(vec![Shape::Null, cx.schema::<Self>()?]))
    }
}

#[test]
fn unguarded_alias_recursion_is_rejected_before_producing_typescript() {
    let mut registry = Registry::new();
    registry
        .register::<AliasLoop>(Direction::Serialize)
        .unwrap();
    let graph = registry.finish().unwrap();
    let Error::UnguardedRecursion(cycle) = Typescript::new().export(&graph, &[]).unwrap_err()
    else {
        panic!("expected cycle error")
    };
    assert_eq!(
        cycle,
        [
            id("cycle.Loop", Direction::Serialize),
            id("cycle.Loop", Direction::Serialize)
        ]
    );
}

#[test]
fn structural_recursion_is_allowed_and_uses_names_not_expansion() {
    let artifact = api::artifact(false).unwrap();
    assert!(artifact.source.contains("export type Tree = Tree[];"));
    assert!(artifact.source.contains("\"children\": NodeOfString[];"));
    assert!(artifact.source.contains("\"right\": RightOutput | null;"));
}

#[test]
fn literal_escaping_preserves_all_json_controls_and_unicode_separators() {
    let controls: String = (0..32).map(char::from).collect();
    let value = format!("{controls}\"\\🌱\u{2028}\u{2029}");
    let artifact = Typescript::new()
        .export(
            &Graph::default(),
            &[Root::new(
                "Literal",
                Direction::Serialize,
                Shape::Literal(value.clone()),
            )],
        )
        .unwrap();
    let literal = artifact
        .source
        .split("export type Literal = ")
        .nth(1)
        .unwrap()
        .trim()
        .strip_suffix(';')
        .unwrap();
    assert_eq!(serde_json::from_str::<String>(literal).unwrap(), value);
    assert!(literal.contains("\\u2028\\u2029"));
    assert!(!literal.contains('\u{2028}'));
}

#[test]
fn object_and_array_rendering_keep_important_typescript_distinctions() {
    let roots = [
        Root::new(
            "Empty",
            Direction::Serialize,
            Shape::Object(Object {
                fields: vec![],
                unknown_fields: UnknownFields::Reject,
            }),
        ),
        Root::new(
            "Open",
            Direction::Deserialize,
            Shape::Object(Object {
                fields: vec![],
                unknown_fields: UnknownFields::Ignore,
            }),
        ),
        Root::new(
            "NullableElements",
            Direction::Serialize,
            Shape::Array(Box::new(Shape::Union(vec![Shape::String, Shape::Null]))),
        ),
        Root::new(
            "NullableArray",
            Direction::Serialize,
            Shape::Union(vec![Shape::Array(Box::new(Shape::String)), Shape::Null]),
        ),
    ];
    let source = Typescript::new()
        .export(&Graph::default(), &roots)
        .unwrap()
        .source;
    assert!(source.contains("export type Empty = { [key: string]: never };"));
    assert!(source.contains("export type Open = { [key: string]: unknown };"));
    assert!(source.contains("export type NullableElements = (string | null)[];"));
    assert!(source.contains("export type NullableArray = string[] | null;"));
}

#[test]
fn documentation_cannot_terminate_its_comment_and_inject_a_declaration() {
    let mut root = Root::new("Documented", Direction::Serialize, Shape::String);
    root.documentation = "first\r\n*/\rexport type Injected = any;".into();
    let source = Typescript::new()
        .export(&Graph::default(), &[root])
        .unwrap()
        .source;
    assert!(source.contains(" * *\\/\n * export type Injected = any;\n */\nexport type Documented"));
    assert!(!source.contains('\r'));
}

#[test]
fn empty_exports_are_still_modules() {
    assert!(Typescript::new()
        .export(&Graph::default(), &[])
        .unwrap()
        .source
        .ends_with("export {};\n"));
}
