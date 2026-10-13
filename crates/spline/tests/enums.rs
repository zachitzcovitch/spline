mod support;

use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use spline::*;
use support::*;

#[test]
fn adjacent_enums_model_unit_and_optional_newtype_content_correctly() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[serde(
        tag = "kind",
        content = "details",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    enum Event {
        Ready,
        Text(Option<String>),
        Changed { field: String },
    }
    let (input, input_graph) = graph::<Event>(Direction::Deserialize);
    let (output, output_graph) = graph::<Event>(Direction::Serialize);
    let Shape::Union(variants) = body(&input, &input_graph) else {
        panic!("expected union")
    };
    assert_eq!(field(&variants[0], "details").presence, Presence::Optional);
    assert_eq!(field(&variants[1], "details").presence, Presence::Optional);
    assert_eq!(field(&variants[2], "details").presence, Presence::Required);
    for value in [
        json!({ "kind": "ready" }),
        json!({ "kind": "ready", "details": null }),
        json!({ "kind": "text" }),
        json!({ "kind": "text", "details": "hello" }),
        json!({ "kind": "changed", "details": { "field": "title" } }),
    ] {
        assert!(matches(&input_graph, &input, &value));
        let event: Event = from_value(value).unwrap();
        assert!(matches(&output_graph, &output, &to_value(event).unwrap()));
    }
    for value in [
        json!({ "kind": "ready", "details": 7 }),
        json!({ "kind": "changed" }),
        json!({ "kind": "ready", "unknown": true }),
    ] {
        assert!(!matches(&input_graph, &input, &value));
        assert!(from_value::<Event>(value).is_err());
    }
    assert!(!matches(
        &output_graph,
        &output,
        &json!({ "kind": "ready", "details": null })
    ));
}

#[test]
fn external_enums_support_unit_newtype_and_named_variants() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[serde(rename_all = "snake_case", rename_all_fields = "camelCase")]
    enum Event<T> {
        Ready,
        Item(T),
        Named { field_name: String },
    }
    let (root, graph) = graph::<Event<String>>(Direction::Serialize);
    for value in [
        Event::<String>::Ready,
        Event::Item("hello".into()),
        Event::Named {
            field_name: "title".into(),
        },
    ] {
        let encoded = to_value(value).unwrap();
        assert!(matches(&graph, &root, &encoded));
        assert!(from_value::<Event<String>>(encoded).is_ok());
    }
    assert!(matches(
        &graph,
        &root,
        &json!({ "named": { "fieldName": "title" } })
    ));
}

#[test]
fn internal_enums_honor_field_rules_and_directional_variant_skips() {
    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Event {
        Ready,
        Named {
            #[serde(default)]
            labels: Vec<String>,
        },
        #[serde(skip_deserializing)]
        OutputOnly,
    }
    let (input, input_graph) = graph::<Event>(Direction::Deserialize);
    let (output, output_graph) = graph::<Event>(Direction::Serialize);
    let value = json!({ "kind": "named" });
    assert!(matches(&input_graph, &input, &value));
    let encoded = to_value(from_value::<Event>(value).unwrap()).unwrap();
    assert!(matches(&output_graph, &output, &encoded));
    assert!(!matches(
        &input_graph,
        &input,
        &json!({ "kind": "output_only" })
    ));
    assert!(matches(
        &output_graph,
        &output,
        &to_value(Event::OutputOnly).unwrap()
    ));
}

#[test]
fn wholly_skipped_variants_do_not_require_payload_schema_bounds() {
    struct NoSchema;
    #[derive(Schema)]
    #[allow(dead_code)]
    enum Event {
        Ready,
        #[serde(skip)]
        Hidden(NoSchema),
    }
    let (root, graph) = graph::<Event>(Direction::Serialize);
    assert_eq!(
        body(&root, &graph),
        &Shape::Union(vec![Shape::Literal("Ready".into())])
    );
}

#[test]
fn whole_type_overrides_are_an_explicit_escape_hatch() {
    struct WireValue;
    impl Schema for WireValue {
        fn info() -> TypeInfo {
            TypeInfo::inline("custom.WireValue")
        }
        fn describe(_: &mut Context<'_>) -> Result<Shape> {
            Ok(Shape::Union(vec![Shape::String, Shape::Boolean]))
        }
    }
    #[derive(Serialize, Deserialize, Schema)]
    #[serde(untagged)]
    #[spline(schema = "WireValue")]
    enum Opaque {
        Text(String),
        Flag(bool),
    }
    let (root, graph) = graph::<Opaque>(Direction::Serialize);
    for value in [Opaque::Text("hello".into()), Opaque::Flag(true)] {
        assert!(matches(&graph, &root, &to_value(value).unwrap()));
    }
}
