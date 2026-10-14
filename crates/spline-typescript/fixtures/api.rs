//! Shared by the generator and Rust tests, so the checked-in artifacts cannot drift.

use serde::{Deserialize, Serialize};
use spline::*;
use spline_typescript::{Artifact, Root, Typescript};

mod decimal {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Decimal-string user identifier.
#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.UserId")]
pub struct UserId(
    #[serde(with = "decimal")]
    #[spline(schema = "String")]
    pub u64,
);

/// A user record shared with API clients.
#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.User")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct User {
    pub id: UserId,
    /// Public display name.
    pub display_name: String,
    pub nickname: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "::core::option::Option::is_none")]
    pub bio: Option<String>,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Post")]
#[serde(deny_unknown_fields)]
pub struct Post {
    pub title: String,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Page")]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
    pub items: Vec<T>,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.ApiError")]
#[serde(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ApiError {
    NotFound,
    Validation { field: String },
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Node")]
#[serde(deny_unknown_fields)]
pub struct Node<T> {
    pub value: T,
    pub children: Vec<Node<T>>,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Tree")]
#[serde(transparent)]
pub struct Tree(pub Vec<Tree>);

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Left")]
pub struct Left {
    pub right: Option<Box<Right>>,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Right")]
pub struct Right {
    pub left: Option<Box<Left>>,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.OpenRecord")]
pub struct OpenRecord {
    pub label: String,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Empty")]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Unit")]
pub struct Unit;

#[doc = "Escapes */ without ending documentation.\nexport type Injected = any;"]
#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.Unusual")]
#[serde(deny_unknown_fields)]
pub struct Unusual {
    #[doc = "A field with quotes, a newline, a backslash, and a line separator."]
    #[serde(rename = "quote\"line\n\\end\u{2028}")]
    pub awkward: String,
    #[serde(rename = "default")]
    pub keyword: bool,
    #[serde(rename = "__proto__")]
    pub prototype: String,
    pub kind: UnusualLiteral,
}

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.UnusualLiteral")]
pub enum UnusualLiteral {
    #[serde(rename = "literal\"\\\n\u{2029}")]
    Value,
}

type Register = fn(&mut Registry) -> Result<()>;

fn both<T: Schema>(registry: &mut Registry) -> Result<()> {
    registry.register::<T>(Direction::Deserialize)?;
    registry.register::<T>(Direction::Serialize)?;
    Ok(())
}

pub fn build(reverse: bool) -> Result<(Graph, Vec<Root>)> {
    let mut registrations: Vec<Register> = vec![
        both::<User>,
        both::<Page<User>>,
        both::<Page<Post>>,
        both::<ApiError>,
        both::<Node<String>>,
        both::<Tree>,
        both::<Left>,
        both::<OpenRecord>,
        both::<Empty>,
        both::<Unit>,
        both::<Unusual>,
    ];
    if reverse {
        registrations.reverse();
    }
    let mut registry = Registry::new();
    for register in registrations {
        register(&mut registry)?;
    }
    let mut roots = vec![
        Root::new(
            "UserListInput",
            Direction::Deserialize,
            registry.register::<Vec<User>>(Direction::Deserialize)?,
        ),
        Root::new(
            "OptionalIdsOutput",
            Direction::Serialize,
            registry.register::<Vec<Option<UserId>>>(Direction::Serialize)?,
        ),
        Root::new(
            "LabelsOutput",
            Direction::Serialize,
            registry.register::<std::collections::BTreeMap<String, Vec<Option<u32>>>>(
                Direction::Serialize,
            )?,
        ),
        Root::new(
            "PairOutput",
            Direction::Serialize,
            Shape::Tuple(vec![Shape::String, Shape::Boolean]),
        ),
        // Public symbols can shadow lib utility names: rendering uses [] and
        // index signatures instead of relying on Array<T> or Record<K, V>.
        Root::new("Array", Direction::Serialize, Shape::String),
        Root::new("Record", Direction::Serialize, Shape::Boolean),
    ];
    if reverse {
        roots.reverse();
    }
    Ok((registry.finish()?, roots))
}

pub fn artifact(reverse: bool) -> std::result::Result<Artifact, Box<dyn std::error::Error>> {
    let (graph, roots) = build(reverse)?;
    Ok(Typescript::new().export(&graph, &roots)?)
}

/// Real Serde output checked by TypeScript's satisfies operator. This fixture
/// writer is separate from the exporter; no sample values are needed for schemas.
pub fn samples() -> std::result::Result<String, serde_json::Error> {
    let user = User {
        id: UserId(u64::MAX),
        display_name: "Zach".into(),
        nickname: None,
        tags: vec![],
        bio: None,
    };
    let mut output = String::from(
        "// Generated from real Serde values. Do not edit.\n\
         import type { UserOutput, ApiErrorOutput, NodeOfString, Tree } from \"./generated\";\n\n",
    );
    let values = [
        ("user", "UserOutput", serde_json::to_value(user)?),
        (
            "notFound",
            "ApiErrorOutput",
            serde_json::to_value(ApiError::NotFound)?,
        ),
        (
            "validation",
            "ApiErrorOutput",
            serde_json::to_value(ApiError::Validation {
                field: "title".into(),
            })?,
        ),
        (
            "node",
            "NodeOfString",
            serde_json::to_value(Node {
                value: "root",
                children: vec![Node {
                    value: "leaf",
                    children: vec![],
                }],
            })?,
        ),
        (
            "tree",
            "Tree",
            serde_json::to_value(Tree(vec![Tree(vec![])]))?,
        ),
    ];
    for (name, ty, value) in values {
        output.push_str(&format!(
            "export const {name} = {} satisfies {ty};\n\n",
            serde_json::to_string_pretty(&value)?
        ));
    }
    output.pop();
    Ok(output)
}
