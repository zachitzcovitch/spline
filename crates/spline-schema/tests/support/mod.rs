#![allow(dead_code)] // Different integration-test binaries use different fixtures.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use spline_schema::*;
pub mod parser;

macro_rules! fixture {
    ($source:ident, $($item:tt)*) => {
        const $source: &str = stringify!($($item)*);
        $($item)*
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserId(pub u64);

impl Serialize for UserId {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for UserId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl Schema for UserId {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.UserId").documented("An ID encoded as a decimal string.")
    }

    fn describe(_: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::String)
    }
}

pub type OptionalText = Option<String>;

fixture!(
    USER_SOURCE,
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct User {
        pub id: UserId,
        /// Name shown to other users.
        pub display_name: String,
        pub nickname: OptionalText,
        #[serde(default)]
        pub tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bio: OptionalText,
    }
);

impl Schema for User {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.User").documented("A user record.")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        let metadata = inspect(USER_SOURCE, cx.direction())?;
        let mut fields = Vec::new();
        for field in parsed_fields(&metadata)? {
            let rules = field_rules(&metadata, field)?;
            // This table stands in for generated, statically typed calls. It
            // never interprets field.rust_type or resolves types by string.
            let described = match field.member.as_str() {
                "id" => cx.field::<UserId>(&field.wire_name, rules)?,
                "display_name" => cx.field::<String>(&field.wire_name, rules)?,
                "nickname" => cx.field::<OptionalText>(&field.wire_name, rules)?,
                "tags" => cx.field::<Vec<String>>(&field.wire_name, rules)?,
                "bio" => cx.option_field::<OptionalText>(&field.wire_name, rules)?,
                member => return Err(Error::Unsupported(format!("unmapped field {member}"))),
            };
            if let Some(mut described) = described {
                described.documentation = field.docs.clone();
                fields.push(described);
            }
        }
        Ok(Shape::Object(Object {
            fields,
            unknown_fields: unknown_fields(&metadata, cx.direction()),
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Post {
    pub title: String,
}

impl Schema for Post {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.Post")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(object(vec![Field::required(
            "title",
            cx.schema::<String>()?,
        )]))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
}

impl<T: Schema> Schema for Page<T> {
    fn info() -> TypeInfo {
        TypeInfo::named(Name::applied("demo.Page", vec![T::info().name]))
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(object(vec![Field::required(
            "items",
            cx.schema::<Vec<T>>()?,
        )]))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details", rename_all = "snake_case")]
pub enum ApiError {
    NotFound,
    Validation { field: String },
}

impl Schema for ApiError {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.ApiError")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::Union(vec![
            object(vec![Field::required(
                "kind",
                Shape::Literal("not_found".into()),
            )]),
            object(vec![
                Field::required("kind", Shape::Literal("validation".into())),
                Field::required(
                    "details",
                    object(vec![Field::required("field", cx.schema::<String>()?)]),
                ),
            ]),
        ]))
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub label: String,
    pub children: Vec<Node>,
}

impl Schema for Node {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.Node")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(object(vec![
            Field::required("label", cx.schema::<String>()?),
            Field::required("children", cx.schema::<Vec<Node>>()?),
        ]))
    }
}

pub struct Left;
pub struct Right;

impl Schema for Left {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.Left")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(object(vec![Field::required(
            "right",
            cx.schema::<Option<Box<Right>>>()?,
        )]))
    }
}

impl Schema for Right {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.Right")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(object(vec![Field::required(
            "left",
            cx.schema::<Option<Box<Left>>>()?,
        )]))
    }
}

pub mod decimal {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

fixture!(
    ENCODED_SOURCE,
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct Encoded {
        #[serde(with = "decimal")]
        pub count: u64,
    }
);

impl Schema for Encoded {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.Encoded")
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        let metadata = inspect(ENCODED_SOURCE, cx.direction())?;
        let field = &parsed_fields(&metadata)?[0];
        let rules = field_rules(&metadata, field)?;
        let field = cx.field_with::<String>(&field.wire_name, rules)?.unwrap();
        Ok(object(vec![field]))
    }
}

/// Same proposed logical name as Encoded, but missing its explicit codec description.
pub struct MissingCodec;

impl Schema for MissingCodec {
    fn info() -> TypeInfo {
        Encoded::info()
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        let metadata = inspect(ENCODED_SOURCE, cx.direction())?;
        let field = &parsed_fields(&metadata)?[0];
        let rules = field_rules(&metadata, field)?;
        let field = cx.field::<u64>(&field.wire_name, rules)?.unwrap();
        Ok(object(vec![field]))
    }
}

pub fn object(fields: Vec<Field>) -> Shape {
    Shape::Object(Object {
        fields,
        unknown_fields: UnknownFields::Reject,
    })
}

pub fn inspect(source: &str, direction: Direction) -> Result<parser::Container> {
    let direction = match direction {
        Direction::Serialize => parser::Direction::Serialize,
        Direction::Deserialize => parser::Direction::Deserialize,
    };
    parser::inspect(source, direction)
        .map_err(|errors| Error::InvalidShape(format!("attribute errors: {errors:?}")))
}

pub fn parsed_fields(container: &parser::Container) -> Result<&[parser::Field]> {
    match &container.data {
        parser::Data::Struct {
            style: parser::Style::Named,
            fields,
        } => Ok(fields),
        _ => Err(Error::Unsupported(
            "expected named struct in bridge fixture".into(),
        )),
    }
}

pub fn field_rules(container: &parser::Container, field: &parser::Field) -> Result<FieldRules> {
    // This is deliberately a support check for the fixtures, not an exhaustive
    // Serde lowering pass. Even successfully parsed forms can be unsupported.
    if field.flattened || !field.aliases.is_empty() {
        return Err(Error::Unsupported(
            "flatten or aliases in bridge fixture".into(),
        ));
    }
    let omission = match field.omit_if.as_deref() {
        None => Omission::Never,
        Some("Option :: is_none") => Omission::IfNone,
        Some(_) => Omission::Conditional,
    };
    Ok(FieldRules {
        skipped: field.skipped,
        defaulted: field.deserialize_default != parser::DefaultRule::Unspecified
            || container.deserialize_default != parser::DefaultRule::Unspecified,
        omission,
        custom_codec: field.codec.is_some(),
    })
}

fn unknown_fields(container: &parser::Container, direction: Direction) -> UnknownFields {
    if direction == Direction::Serialize || container.deny_unknown_fields {
        UnknownFields::Reject
    } else {
        UnknownFields::Ignore
    }
}

pub fn definition(graph: &Graph, name: impl Into<Name>, direction: Direction) -> &Shape {
    &graph
        .get(name, direction)
        .expect("missing definition")
        .shape
}

pub fn field<'a>(shape: &'a Shape, name: &str) -> &'a Field {
    let Shape::Object(object) = shape else {
        panic!("expected object")
    };
    object
        .fields
        .iter()
        .find(|field| field.name == name)
        .expect("missing field")
}

/// A test oracle for the prototype's structural shapes, not a production JSON
/// validator. It intentionally does not enforce arbitrary custom Rust predicates.
pub fn matches(graph: &Graph, shape: &Shape, value: &Value) -> bool {
    fn visit(graph: &Graph, shape: &Shape, value: &Value, budget: usize) -> bool {
        if budget == 0 {
            return false;
        }
        let child = |shape: &Shape, value: &Value| visit(graph, shape, value, budget - 1);
        match shape {
            Shape::Null => value.is_null(),
            Shape::Boolean => value.is_boolean(),
            Shape::String => value.is_string(),
            Shape::Literal(expected) => value.as_str() == Some(expected),
            Shape::Integer { signed: true, bits } => value.as_i64().is_some_and(|n| {
                let limit = 1_i64 << (bits - 1);
                (-limit..limit).contains(&n)
            }),
            Shape::Integer {
                signed: false,
                bits,
            } => value.as_u64().is_some_and(|n| n < (1_u64 << bits)),
            Shape::Array(element) => value
                .as_array()
                .is_some_and(|values| values.iter().all(|v| child(element, v))),
            Shape::Map(element) => value
                .as_object()
                .is_some_and(|values| values.values().all(|v| child(element, v))),
            Shape::Tuple(elements) => value.as_array().is_some_and(|values| {
                elements.len() == values.len()
                    && elements.iter().zip(values).all(|(s, v)| child(s, v))
            }),
            Shape::Union(variants) => variants.iter().any(|shape| child(shape, value)),
            Shape::Ref(id) => graph
                .definitions()
                .get(id)
                .is_some_and(|definition| child(&definition.shape, value)),
            Shape::Object(object) => value.as_object().is_some_and(|values| {
                object
                    .fields
                    .iter()
                    .all(|field| match values.get(&field.name) {
                        Some(value) => child(&field.shape, value),
                        None => field.presence == Presence::Optional,
                    })
                    && (object.unknown_fields == UnknownFields::Ignore
                        || values
                            .keys()
                            .all(|name| object.fields.iter().any(|f| &f.name == name)))
            }),
        }
    }
    visit(graph, shape, value, 256)
}
