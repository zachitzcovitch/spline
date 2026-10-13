mod support;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use spline_schema::*;
use support::*;

#[test]
fn parser_facts_and_typed_schemas_produce_distinct_user_views() {
    let mut registry = Registry::new();
    let input = registry.register::<User>(Direction::Deserialize).unwrap();
    let output = registry.register::<User>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    let input_body = definition(&graph, "demo.User", Direction::Deserialize);
    let output_body = definition(&graph, "demo.User", Direction::Serialize);
    assert_ne!(input, output);
    assert_eq!(field(input_body, "nickname").presence, Presence::Optional);
    assert_eq!(field(output_body, "nickname").presence, Presence::Required);
    assert_eq!(field(input_body, "tags").presence, Presence::Optional);
    assert_eq!(field(output_body, "tags").presence, Presence::Required);
    assert_eq!(field(output_body, "bio").presence, Presence::Optional);
    assert_eq!(field(output_body, "bio").shape, Shape::String);
    assert_eq!(
        field(output_body, "displayName").documentation,
        ["Name shown to other users."]
    );

    let value = json!({ "id": "18446744073709551615", "displayName": "Zach", "extra": 3 });
    assert!(matches(&graph, &input, &value));
    let user: User = from_value(value).unwrap();
    assert_eq!(user.id, UserId(u64::MAX));
    let encoded = to_value(&user).unwrap();
    assert!(matches(&graph, &output, &encoded));
    assert_eq!(encoded["nickname"], json!(null));
    assert!(encoded.get("bio").is_none());
    assert!(encoded.get("extra").is_none());

    let mut missing_output = encoded.clone();
    missing_output.as_object_mut().unwrap().remove("nickname");
    assert!(!matches(&graph, &output, &missing_output));
    let bad = json!({ "id": "7", "displayName": "Zach", "tags": null });
    assert!(!matches(&graph, &input, &bad));
    assert!(from_value::<User>(bad).is_err());
}

#[test]
fn none_omission_keeps_null_when_the_some_value_is_itself_optional() {
    #[derive(Debug, Serialize, Deserialize)]
    struct Nested {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Option<String>>,
    }
    impl Schema for Nested {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Nested")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            let rules = FieldRules {
                defaulted: true,
                omission: Omission::IfNone,
                ..FieldRules::default()
            };
            Ok(object(vec![cx
                .option_field::<Option<Option<String>>>("value", rules)?
                .unwrap()]))
        }
    }
    let mut registry = Registry::new();
    let output = registry.register::<Nested>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    for value in [None, Some(None), Some(Some("hello".into()))] {
        let encoded = to_value(Nested { value }).unwrap();
        assert!(matches(&graph, &output, &encoded));
    }
    let encoded = to_value(Nested { value: Some(None) }).unwrap();
    assert_eq!(encoded, json!({ "value": null }));
    // Ordinary nested Option loses this distinction on deserialization. Wire
    // shape fidelity does not imply preservation of every Rust value state.
    assert_eq!(from_value::<Nested>(encoded).unwrap().value, None);
    let body = definition(&graph, "demo.Nested", Direction::Serialize);
    assert_eq!(field(body, "value").presence, Presence::Optional);
    assert_eq!(
        field(body, "value").shape,
        Shape::Union(vec![Shape::String, Shape::Null])
    );
}

#[test]
fn nullable_newtypes_do_not_automatically_accept_missing_fields() {
    #[derive(Debug, Deserialize)]
    struct Plain(Option<String>);
    #[derive(Debug, Deserialize)]
    #[serde(transparent)]
    struct Transparent(Option<String>);
    #[derive(Debug, Deserialize)]
    struct Wrappers {
        plain: Plain,
        transparent: Transparent,
        boxed: Box<Option<String>>,
    }
    impl Schema for Plain {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Plain")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            cx.schema::<Option<String>>()
        }
    }
    impl Schema for Transparent {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Transparent").accepts_missing()
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            cx.schema::<Option<String>>()
        }
    }
    impl Schema for Wrappers {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Wrappers")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            Ok(object(vec![
                cx.field::<Plain>("plain", FieldRules::default())?.unwrap(),
                cx.field::<Transparent>("transparent", FieldRules::default())?
                    .unwrap(),
                cx.field::<Box<Option<String>>>("boxed", FieldRules::default())?
                    .unwrap(),
            ]))
        }
    }
    let mut registry = Registry::new();
    let input = registry
        .register::<Wrappers>(Direction::Deserialize)
        .unwrap();
    let graph = registry.finish().unwrap();
    assert!(!matches(&graph, &input, &json!({})));
    assert!(from_value::<Wrappers>(json!({})).is_err());
    let value = json!({ "plain": null });
    assert!(matches(&graph, &input, &value));
    let decoded: Wrappers = from_value(value).unwrap();
    assert_eq!(decoded.plain.0, None);
    assert_eq!(decoded.transparent.0, None);
    assert_eq!(*decoded.boxed, None);
}

#[test]
fn field_level_custom_deserializers_override_option_missing_behavior() {
    fn optional<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Option<String>, D::Error> {
        Option::<String>::deserialize(deserializer)
    }
    #[derive(Debug, Deserialize)]
    struct Custom {
        #[serde(deserialize_with = "optional")]
        required: Option<String>,
        #[serde(default, deserialize_with = "optional")]
        defaulted: Option<String>,
    }
    impl Schema for Custom {
        fn info() -> TypeInfo {
            TypeInfo::named("demo.Custom")
        }
        fn describe(cx: &mut Context<'_>) -> Result<Shape> {
            let rules = FieldRules {
                custom_codec: true,
                ..FieldRules::default()
            };
            Ok(object(vec![
                cx.field_with::<Option<String>>("required", rules)?.unwrap(),
                cx.field_with::<Option<String>>(
                    "defaulted",
                    FieldRules {
                        defaulted: true,
                        ..rules
                    },
                )?
                .unwrap(),
            ]))
        }
    }
    let mut registry = Registry::new();
    let input = registry.register::<Custom>(Direction::Deserialize).unwrap();
    let graph = registry.finish().unwrap();
    assert!(!matches(&graph, &input, &json!({})));
    assert!(from_value::<Custom>(json!({})).is_err());
    let value = json!({ "required": null });
    assert!(matches(&graph, &input, &value));
    let decoded: Custom = from_value(value).unwrap();
    assert_eq!(decoded.required, None);
    assert_eq!(decoded.defaulted, None);
}

#[test]
fn custom_codec_descriptions_match_real_encoding_and_are_required() {
    let mut registry = Registry::new();
    let error = registry
        .register::<MissingCodec>(Direction::Serialize)
        .unwrap_err();
    assert_eq!(error, Error::MissingCodec("count".into()));
    assert!(registry.clone().finish().unwrap().definitions().is_empty());
    // The failed name reservation must not conflict with the corrected description.
    let input = registry
        .register::<Encoded>(Direction::Deserialize)
        .unwrap();
    let output = registry.register::<Encoded>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    let value = to_value(Encoded { count: u64::MAX }).unwrap();
    assert!(matches(&graph, &output, &value));
    assert!(matches(&graph, &input, &value));
    assert_eq!(from_value::<Encoded>(value).unwrap().count, u64::MAX);
    assert!(!matches(&graph, &input, &json!({ "count": 7 })));
}

#[test]
fn custom_rust_predicates_are_not_proved_by_a_structural_schema() {
    let mut registry = Registry::new();
    let input = registry.register::<UserId>(Direction::Deserialize).unwrap();
    let graph = registry.finish().unwrap();
    assert!(matches(&graph, &input, &json!("not-an-id")));
    assert!(from_value::<UserId>(json!("not-an-id")).is_err());
}

#[test]
fn tagged_errors_are_normalized_into_wire_alternatives() {
    let mut registry = Registry::new();
    let output = registry.register::<ApiError>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    for error in [
        ApiError::NotFound,
        ApiError::Validation {
            field: "title".into(),
        },
    ] {
        assert!(matches(&graph, &output, &to_value(error).unwrap()));
    }
    assert!(!matches(&graph, &output, &json!({ "kind": "validation" })));
    assert!(!matches(&graph, &output, &json!({ "kind": "unknown" })));
    assert!(!matches(
        &graph,
        &output,
        &json!({ "kind": "not_found", "details": null })
    ));
}

#[test]
fn recursive_definitions_terminate_and_describe_finite_values() {
    let mut registry = Registry::new();
    let output = registry.register::<Node>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    assert_eq!(graph.definitions().len(), 1);
    let node = Node {
        label: "root".into(),
        children: vec![Node {
            label: "leaf".into(),
            children: vec![],
        }],
    };
    let value = to_value(&node).unwrap();
    assert!(matches(&graph, &output, &value));
    assert_eq!(from_value::<Node>(value).unwrap(), node);
}

#[test]
fn small_integer_ranges_maps_and_unit_follow_the_initial_json_profile() {
    let mut registry = Registry::new();
    let map = registry
        .register::<BTreeMap<String, u32>>(Direction::Deserialize)
        .unwrap();
    let signed = registry.register::<i8>(Direction::Deserialize).unwrap();
    let unit = registry.register::<()>(Direction::Serialize).unwrap();
    assert!(matches!(
        registry.register::<u64>(Direction::Serialize),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        registry.register::<f64>(Direction::Serialize),
        Err(Error::Unsupported(_))
    ));
    let graph = registry.finish().unwrap();
    assert!(matches(&graph, &map, &json!({ "n": u32::MAX })));
    assert!(!matches(
        &graph,
        &map,
        &json!({ "n": u64::from(u32::MAX) + 1 })
    ));
    assert!(matches(&graph, &signed, &json!(-128)));
    assert!(!matches(&graph, &signed, &json!(-129)));
    assert!(!matches(&graph, &signed, &json!(1.5)));
    assert!(matches(&graph, &unit, &to_value(()).unwrap()));
}
