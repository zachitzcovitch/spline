mod support;

use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use spline::*;
use support::*;

type MaybeText = Option<String>;

/// A user visible to API clients.
#[derive(Debug, Serialize, Deserialize, Schema)]
#[spline(name = "api.User")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct User {
    /// Display name.
    display_name: String,
    nickname: MaybeText,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "::core::option::Option::is_none")]
    bio: MaybeText,
}

#[test]
fn derives_directional_fields_names_defaults_and_documentation() {
    let (input, input_graph) = graph::<User>(Direction::Deserialize);
    let (output, output_graph) = graph::<User>(Direction::Serialize);
    let input_body = body(&input, &input_graph);
    let output_body = body(&output, &output_graph);
    assert_eq!(User::info().name, Name::new("api.User"));
    assert_eq!(User::info().documentation, "A user visible to API clients.");
    assert_eq!(
        field(output_body, "displayName").documentation,
        ["Display name."]
    );
    assert_eq!(field(input_body, "nickname").presence, Presence::Optional);
    assert_eq!(field(output_body, "nickname").presence, Presence::Required);
    assert_eq!(field(input_body, "tags").presence, Presence::Optional);
    assert_eq!(field(output_body, "tags").presence, Presence::Required);
    assert_eq!(field(output_body, "bio").presence, Presence::Optional);
    assert_eq!(field(output_body, "bio").shape, Shape::String);

    let value = json!({ "displayName": "Zach" });
    assert!(matches(&input_graph, &input, &value));
    let user: User = from_value(value).unwrap();
    assert!(matches(&output_graph, &output, &to_value(user).unwrap()));
    for invalid in [
        json!({}),
        json!({ "displayName": "Zach", "tags": null }),
        json!({ "displayName": "Zach", "extra": true }),
    ] {
        assert!(!matches(&input_graph, &input, &invalid));
        assert!(from_value::<User>(invalid).is_err());
    }
}

#[test]
fn directional_renames_and_raw_identifiers_are_preserved() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Renamed {
        #[serde(rename(serialize = "out", deserialize = "in"))]
        name: String,
        r#type: bool,
    }
    let (input, input_graph) = graph::<Renamed>(Direction::Deserialize);
    let (output, output_graph) = graph::<Renamed>(Direction::Serialize);
    let value = json!({ "in": "name", "type": true });
    assert!(matches(&input_graph, &input, &value));
    let encoded = to_value(from_value::<Renamed>(value).unwrap()).unwrap();
    assert_eq!(encoded, json!({ "out": "name", "type": true }));
    assert!(matches(&output_graph, &output, &encoded));
    assert!(from_value::<Renamed>(encoded).is_err());
}

#[derive(Debug, Serialize, Deserialize, Schema)]
#[spline(name = "api.Page")]
struct Page<T> {
    items: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize, Schema)]
struct Node<T> {
    value: T,
    children: Vec<Node<T>>,
}

#[test]
fn generic_and_recursive_impls_do_not_introduce_cyclic_where_bounds() {
    let mut registry = Registry::new();
    let strings = registry
        .register::<Page<String>>(Direction::Serialize)
        .unwrap();
    let numbers = registry
        .register::<Page<u32>>(Direction::Serialize)
        .unwrap();
    let tree = registry
        .register::<Node<String>>(Direction::Serialize)
        .unwrap();
    let graph = registry.finish().unwrap();
    assert_ne!(strings, numbers);
    assert_eq!(
        Page::<String>::info().name.to_string(),
        "api.Page<core.String>"
    );
    let value = to_value(Page {
        items: vec!["hello".to_owned()],
    })
    .unwrap();
    assert!(matches(&graph, &strings, &value));
    assert!(!matches(&graph, &numbers, &value));
    let value: Node<String> = Node {
        value: "root".into(),
        children: vec![Node {
            value: "leaf".into(),
            children: vec![],
        }],
    };
    assert!(matches(&graph, &tree, &to_value(value).unwrap()));
}

#[derive(Debug, Serialize, Deserialize, Schema)]
struct Left {
    right: Option<Box<Right>>,
}
#[derive(Debug, Serialize, Deserialize, Schema)]
struct Right {
    left: Option<Box<Left>>,
}

#[test]
fn mutually_recursive_derives_share_reserved_definitions() {
    let (root, graph) = graph::<Left>(Direction::Serialize);
    assert_eq!(graph.definitions().len(), 2);
    let value = Left {
        right: Some(Box::new(Right { left: None })),
    };
    assert!(matches(&graph, &root, &to_value(value).unwrap()));
}

type Children = Vec<TransparentTree>;

#[derive(Debug, Serialize, Deserialize, Schema)]
#[serde(transparent)]
struct TransparentTree(Children);

#[test]
fn transparent_recursive_metadata_does_not_recurse_through_type_names() {
    assert_eq!(TransparentTree::missing_field(), MissingField::Rejected);
    let (root, graph) = graph::<TransparentTree>(Direction::Serialize);
    assert_eq!(graph.definitions().len(), 1);
    let value = TransparentTree(vec![TransparentTree(vec![])]);
    let encoded = to_value(value).unwrap();
    assert_eq!(encoded, json!([[]]));
    assert!(matches(&graph, &root, &encoded));
    assert!(from_value::<TransparentTree>(encoded).is_ok());
}

#[test]
fn nullable_newtypes_and_transparent_wrappers_keep_distinct_missing_rules() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Plain(MaybeText);
    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[serde(transparent)]
    struct Transparent(MaybeText);
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Wrappers {
        plain: Plain,
        transparent: Transparent,
    }

    assert_eq!(Plain::missing_field(), MissingField::Rejected);
    assert_eq!(Transparent::missing_field(), MissingField::Accepted);
    let (root, graph) = graph::<Wrappers>(Direction::Deserialize);
    assert!(!matches(&graph, &root, &json!({})));
    assert!(from_value::<Wrappers>(json!({})).is_err());
    let value = json!({ "plain": null });
    assert!(matches(&graph, &root, &value));
    let decoded: Wrappers = from_value(value).unwrap();
    assert_eq!(decoded.plain.0, None);
    assert_eq!(decoded.transparent.0, None);
}

#[test]
fn transparent_fields_are_selected_independently_for_each_direction() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[serde(transparent)]
    struct Split {
        #[serde(skip_deserializing)]
        emitted: String,
        #[serde(skip_serializing)]
        accepted: u32,
    }
    let (input, input_graph) = graph::<Split>(Direction::Deserialize);
    let (output, output_graph) = graph::<Split>(Direction::Serialize);
    assert!(matches(&input_graph, &input, &json!(7)));
    assert!(matches(&output_graph, &output, &json!("seven")));
    assert_eq!(from_value::<Split>(json!(7)).unwrap().accepted, 7);
    assert_eq!(
        to_value(Split {
            emitted: "seven".into(),
            accepted: 7
        })
        .unwrap(),
        json!("seven")
    );
}

#[derive(Debug, Default)]
struct NoSchema;

#[derive(Debug, Serialize, Deserialize, Schema)]
struct Skipped<T> {
    visible: String,
    #[serde(skip)]
    hidden: T,
}

#[test]
fn skipped_fields_and_generic_parameters_need_no_schema_implementation() {
    let (root, graph) = graph::<Skipped<NoSchema>>(Direction::Serialize);
    let value = Skipped {
        visible: "hello".into(),
        hidden: NoSchema,
    };
    assert!(matches(&graph, &root, &to_value(value).unwrap()));
    let decoded: Skipped<NoSchema> = from_value(json!({ "visible": "hello" })).unwrap();
    let _: NoSchema = decoded.hidden;
    assert!(Skipped::<NoSchema>::info().name.arguments().is_empty());

    // Hidden parameters have no schema name. The strict registry reports
    // collisions if distinct hidden-only instantiations are exported together.
    let mut registry = Registry::new();
    registry
        .register::<Skipped<NoSchema>>(Direction::Serialize)
        .unwrap();
    assert!(matches!(
        registry.register::<Skipped<u32>>(Direction::Serialize),
        Err(Error::NameCollision { .. })
    ));
}

#[test]
fn selected_directions_prune_inactive_fields_and_validation() {
    #[derive(Serialize, Schema)]
    #[spline(only = "serialize")]
    struct ReadOnly {
        visible: String,
        #[serde(skip_serializing)]
        hidden: NoSchema,
    }
    #[derive(Serialize, Schema)]
    #[spline(only = "serialize")]
    #[serde(transparent)]
    struct Defaulted(#[serde(default)] String);

    let (root, graph) = graph::<ReadOnly>(Direction::Serialize);
    let value = ReadOnly {
        visible: "hello".into(),
        hidden: NoSchema,
    };
    let _ = &value.hidden;
    assert!(matches(&graph, &root, &to_value(value).unwrap()));
    let (root, graph) = support::graph::<Defaulted>(Direction::Serialize);
    assert!(matches(
        &graph,
        &root,
        &to_value(Defaulted("x".into())).unwrap()
    ));
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register::<Defaulted>(Direction::Deserialize),
        Err(Error::Unsupported(_))
    ));
}

mod decimal {
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

#[test]
fn field_codec_overrides_use_actual_wire_types() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Encoded {
        #[serde(with = "decimal")]
        #[spline(schema = "String")]
        count: u64,
    }
    let (input, input_graph) = graph::<Encoded>(Direction::Deserialize);
    let (output, output_graph) = graph::<Encoded>(Direction::Serialize);
    let value = to_value(Encoded { count: u64::MAX }).unwrap();
    assert_eq!(value, json!({ "count": "18446744073709551615" }));
    assert!(matches(&output_graph, &output, &value));
    assert!(matches(&input_graph, &input, &value));
    assert_eq!(from_value::<Encoded>(value).unwrap().count, u64::MAX);
}

#[test]
fn codec_overrides_remove_native_generic_schema_bounds() {
    mod opaque {
        use serde::{Deserializer, Serializer};
        pub fn serialize<T, S: Serializer>(_: &T, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str("opaque")
        }
        pub fn deserialize<'de, T: Default, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<T, D::Error> {
            let _: String = serde::Deserialize::deserialize(deserializer)?;
            Ok(T::default())
        }
    }
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Opaque<T: Default> {
        #[serde(with = "opaque")]
        #[spline(schema = "String")]
        value: T,
    }
    let (root, graph) = graph::<Opaque<NoSchema>>(Direction::Serialize);
    let value = to_value(Opaque { value: NoSchema }).unwrap();
    assert!(matches(&graph, &root, &value));
    let _: Opaque<NoSchema> = from_value(value).unwrap();
}

#[test]
fn directional_codec_overrides_describe_different_encodings() {
    fn narrow_input<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<u64, D::Error> {
        u32::deserialize(d).map(u64::from)
    }
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Encoded {
        #[serde(
            serialize_with = "decimal::serialize",
            deserialize_with = "narrow_input"
        )]
        #[spline(serialize_as = "String", deserialize_as = "u32")]
        count: u64,
    }
    let (input, input_graph) = graph::<Encoded>(Direction::Deserialize);
    let (output, output_graph) = graph::<Encoded>(Direction::Serialize);
    let value = json!({ "count": 7 });
    assert!(matches(&input_graph, &input, &value));
    let encoded = to_value(from_value::<Encoded>(value).unwrap()).unwrap();
    assert_eq!(encoded, json!({ "count": "7" }));
    assert!(matches(&output_graph, &output, &encoded));
    let out_of_range = json!({ "count": u64::from(u32::MAX) + 1 });
    assert!(!matches(&input_graph, &input, &out_of_range));
    assert!(from_value::<Encoded>(out_of_range).is_err());
}

#[test]
fn custom_deserializer_overrides_do_not_erase_missing_field_rules() {
    fn optional<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<MaybeText, D::Error> {
        MaybeText::deserialize(d)
    }
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Custom {
        #[serde(deserialize_with = "optional")]
        #[spline(deserialize_as = "MaybeText")]
        required: MaybeText,
        #[serde(default, deserialize_with = "optional")]
        #[spline(deserialize_as = "MaybeText")]
        defaulted: MaybeText,
    }
    let (root, graph) = graph::<Custom>(Direction::Deserialize);
    assert!(!matches(&graph, &root, &json!({})));
    assert!(from_value::<Custom>(json!({})).is_err());
    let value = json!({ "required": null });
    assert!(matches(&graph, &root, &value));
    let decoded: Custom = from_value(value).unwrap();
    assert_eq!((decoded.required, decoded.defaulted), (None, None));
}

#[test]
fn a_present_value_override_does_not_invent_missing_field_acceptance() {
    #[derive(Debug, Serialize, Deserialize)]
    struct Plain(Option<String>);
    #[derive(Debug, Serialize, Deserialize, Schema)]
    struct Holder {
        #[spline(schema = "MaybeText")]
        value: Plain,
    }
    let (root, graph) = graph::<Holder>(Direction::Deserialize);
    assert!(!matches(&graph, &root, &json!({})));
    assert!(from_value::<Holder>(json!({})).is_err());
    let value = json!({ "value": null });
    assert!(matches(&graph, &root, &value));
    assert_eq!(from_value::<Holder>(value).unwrap().value.0, None);
}

#[test]
fn opaque_type_overrides_and_explicit_missing_policy_are_supported() {
    #[derive(Debug, Schema)]
    #[spline(name = "api.CustomId", schema = "String")]
    struct Id(u64);
    impl Serialize for Id {
        fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
            decimal::serialize(&self.0, s)
        }
    }
    let (root, graph) = graph::<Id>(Direction::Serialize);
    assert!(matches(&graph, &root, &to_value(Id(7)).unwrap()));

    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[serde(transparent)]
    #[spline(schema = "MaybeText", missing = "accepted")]
    struct Optional(MaybeText);
    assert_eq!(Optional::missing_field(), MissingField::Accepted);
    #[derive(Debug, Deserialize, Schema)]
    struct Wrapper {
        optional: Optional,
    }
    let (root, graph) = support::graph::<Wrapper>(Direction::Deserialize);
    assert!(matches(&graph, &root, &json!({})));
    assert_eq!(from_value::<Wrapper>(json!({})).unwrap().optional.0, None);
}

#[test]
fn nested_options_preserve_null_after_none_omission() {
    #[derive(Serialize, Deserialize, Schema)]
    struct Nested {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        value: Option<MaybeText>,
    }
    let (root, graph) = graph::<Nested>(Direction::Serialize);
    let value = to_value(Nested { value: Some(None) }).unwrap();
    assert_eq!(value, json!({ "value": null }));
    assert!(matches(&graph, &root, &value));
    assert!(matches(
        &graph,
        &root,
        &to_value(Nested { value: None }).unwrap()
    ));
}

#[test]
fn short_predicate_paths_are_conservative_even_when_shadowed() {
    struct Option;
    impl Option {
        fn is_none(_: &std::option::Option<String>) -> bool {
            false
        }
    }
    #[derive(Serialize, Schema)]
    #[spline(only = "serialize")]
    struct Shadowed {
        #[serde(skip_serializing_if = "Option::is_none")]
        value: std::option::Option<String>,
    }
    let (root, graph) = graph::<Shadowed>(Direction::Serialize);
    let value = to_value(Shadowed { value: None }).unwrap();
    assert_eq!(value, json!({ "value": null }));
    assert!(matches(&graph, &root, &value));
}

#[test]
fn logical_names_and_runtime_paths_can_be_overridden() {
    use spline as renamed_runtime;
    struct String;
    struct Vec;
    let _ = (String, Vec);
    #[derive(Serialize, Schema)]
    #[spline(crate = "renamed_runtime", name = "custom.Record")]
    struct Record {
        value: std::string::String,
    }
    let (root, graph) = graph::<Record>(Direction::Serialize);
    assert_eq!(Record::info().name, Name::new("custom.Record"));
    assert!(matches(
        &graph,
        &root,
        &to_value(Record {
            value: "hello".into()
        })
        .unwrap()
    ));
}

#[test]
fn empty_named_and_unit_structs_have_distinct_shapes() {
    #[derive(Serialize, Deserialize, Schema)]
    struct Empty {}
    #[derive(Serialize, Deserialize, Schema)]
    struct Unit;
    let (root, graph) = graph::<Empty>(Direction::Serialize);
    assert!(matches(&graph, &root, &to_value(Empty {}).unwrap()));
    let (root, graph) = support::graph::<Unit>(Direction::Serialize);
    assert!(matches(&graph, &root, &to_value(Unit).unwrap()));
}

#[test]
fn numeric_profile_errors_still_happen_at_registration() {
    #[derive(Schema)]
    #[allow(dead_code)]
    struct Raw {
        value: u64,
    }
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register::<Raw>(Direction::Serialize),
        Err(Error::Unsupported(_))
    ));
    assert!(registry.finish().unwrap().definitions().is_empty());
}
