#![allow(dead_code)] // Fixtures describe types without constructing their values.

use serde::{Deserialize, Serialize};
use spline::{DefinitionId, Direction, Registry, Schema};
use spline_typescript::{Error, Typescript};

fn id<T: Schema>(direction: Direction) -> DefinitionId {
    DefinitionId {
        name: T::info().name,
        direction,
    }
}

#[derive(Schema)]
#[spline(name = "domain.User")]
#[serde(deny_unknown_fields)]
struct User {
    name: String,
}

#[derive(Schema)]
#[spline(name = "domain.Page")]
#[serde(deny_unknown_fields)]
struct Page<T> {
    items: Vec<T>,
}

#[derive(Schema)]
#[spline(name = "domain.Tree")]
#[serde(deny_unknown_fields)]
struct Tree {
    children: Vec<Tree>,
}

#[allow(dead_code)]
#[derive(Schema)]
#[spline(name = "domain.Left")]
#[serde(deny_unknown_fields)]
struct Left {
    right: Vec<Right>,
}
#[allow(dead_code)]
#[derive(Schema)]
#[spline(name = "domain.Right")]
#[serde(deny_unknown_fields)]
struct Right {
    left: Vec<Left>,
}

fn both<T: Schema>(registry: &mut Registry) {
    registry.register::<T>(Direction::Deserialize).unwrap();
    registry.register::<T>(Direction::Serialize).unwrap();
}

#[test]
fn equivalent_directions_generics_and_recursion_share_readable_declarations() {
    let mut registry = Registry::new();
    both::<Page<User>>(&mut registry);
    both::<Tree>(&mut registry);
    both::<Left>(&mut registry);
    let artifact = Typescript::new()
        .export(&registry.finish().unwrap(), &[])
        .unwrap();
    for (input, output, symbol) in [
        (
            id::<User>(Direction::Deserialize),
            id::<User>(Direction::Serialize),
            "User",
        ),
        (
            id::<Page<User>>(Direction::Deserialize),
            id::<Page<User>>(Direction::Serialize),
            "PageOfUser",
        ),
        (
            id::<Tree>(Direction::Deserialize),
            id::<Tree>(Direction::Serialize),
            "Tree",
        ),
        (
            id::<Left>(Direction::Deserialize),
            id::<Left>(Direction::Serialize),
            "Left",
        ),
    ] {
        assert_eq!(artifact.symbols[&input], symbol);
        assert_eq!(artifact.symbols[&output], symbol);
        assert_eq!(
            artifact
                .source
                .matches(&format!("export type {symbol} ="))
                .count(),
            1
        );
    }
    assert!(artifact.source.contains("\"items\": User[]"));
    assert!(artifact.source.contains("\"children\": Tree[]"));
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "domain.Profile")]
#[serde(deny_unknown_fields)]
struct Profile {
    #[serde(rename(serialize = "displayName", deserialize = "name"))]
    name: String,
}

#[allow(dead_code)]
#[derive(Schema)]
#[spline(name = "domain.Envelope")]
#[serde(deny_unknown_fields)]
struct Envelope {
    profile: Profile,
}

#[test]
fn nested_directional_differences_are_preserved_and_explicit_unification_is_rejected() {
    let mut registry = Registry::new();
    both::<Envelope>(&mut registry);
    let graph = registry.finish().unwrap();
    let artifact = Typescript::new().export(&graph, &[]).unwrap();
    assert_eq!(
        artifact.symbols[&id::<Profile>(Direction::Deserialize)],
        "ProfileInput"
    );
    assert_eq!(
        artifact.symbols[&id::<Profile>(Direction::Serialize)],
        "ProfileOutput"
    );
    assert!(artifact.source.contains("export type EnvelopeInput"));
    assert!(artifact.source.contains("\"profile\": ProfileInput"));
    assert!(artifact.source.contains("\"profile\": ProfileOutput"));
    assert!(matches!(Typescript::new()
        .name(id::<Profile>(Direction::Deserialize), "Profile")
        .name(id::<Profile>(Direction::Serialize), "Profile")
        .export(&graph, &[]), Err(Error::SymbolCollision(name)) if name == "Profile"));
}

#[test]
fn presence_and_unknown_field_rules_keep_directions_distinct() {
    #[derive(Schema)]
    #[serde(deny_unknown_fields)]
    struct Optional {
        value: Option<String>,
    }
    #[derive(Schema)]
    struct Open {
        value: String,
    }
    let mut registry = Registry::new();
    both::<Optional>(&mut registry);
    both::<Open>(&mut registry);
    let artifact = Typescript::new()
        .export(&registry.finish().unwrap(), &[])
        .unwrap();
    assert_eq!(
        artifact.symbols[&id::<Optional>(Direction::Deserialize)],
        "OptionalInput"
    );
    assert_eq!(
        artifact.symbols[&id::<Optional>(Direction::Serialize)],
        "OptionalOutput"
    );
    assert_eq!(
        artifact.symbols[&id::<Open>(Direction::Deserialize)],
        "OpenInput"
    );
    assert_eq!(
        artifact.symbols[&id::<Open>(Direction::Serialize)],
        "OpenOutput"
    );
}

#[test]
fn one_direction_has_a_plain_name_and_collisions_are_order_independent() {
    #[derive(Schema)]
    #[spline(name = "other.User")]
    struct Other;
    let mut registry = Registry::new();
    registry.register::<User>(Direction::Serialize).unwrap();
    assert_eq!(
        Typescript::new()
            .export(&registry.clone().finish().unwrap(), &[])
            .unwrap()
            .symbols[&id::<User>(Direction::Serialize)],
        "User"
    );
    registry.register::<Other>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    assert!(
        matches!(Typescript::new().export(&graph, &[]), Err(Error::SymbolCollision(name)) if name == "User")
    );
    let artifact = Typescript::new()
        .name(id::<Other>(Direction::Serialize), "OtherUser")
        .export(&graph, &[])
        .unwrap();
    assert!(artifact.source.contains("export type OtherUser"));
    assert!(artifact.source.contains("export type User"));
}

#[test]
fn explicit_directional_names_can_split_equivalent_shapes_or_share_a_custom_name() {
    let mut registry = Registry::new();
    both::<Page<User>>(&mut registry);
    let graph = registry.finish().unwrap();
    let shared = Typescript::new()
        .name(id::<User>(Direction::Deserialize), "Person")
        .name(id::<User>(Direction::Serialize), "Person")
        .export(&graph, &[])
        .unwrap();
    assert_eq!(shared.source.matches("export type Person =").count(), 1);
    assert!(shared.source.contains("\"items\": Person[]"));
    let split = Typescript::new()
        .name(id::<User>(Direction::Deserialize), "PersonInput")
        .name(id::<User>(Direction::Serialize), "PersonOutput")
        .export(&graph, &[])
        .unwrap();
    assert!(split.source.contains("export type PersonInput"));
    assert!(split.source.contains("export type PersonOutput"));
}
