use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use spline::{Direction, MissingField, Patch, Presence, Registry, Schema, Shape};

type NullableUpdate = Patch<Option<String>>;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
struct Changes {
    #[serde(default, skip_serializing_if = "spline::Patch::is_missing")]
    name: Patch<String>,
    #[serde(default, skip_serializing_if = "spline::Patch::is_missing")]
    bio: NullableUpdate,
}

#[test]
fn missing_null_and_value_round_trip_as_three_distinct_states() {
    for (wire, expected) in [
        (json!({}), Changes::default()),
        (
            json!({ "bio": null }),
            Changes {
                bio: Patch::Value(None),
                ..Changes::default()
            },
        ),
        (
            json!({ "bio": "hello" }),
            Changes {
                bio: Patch::Value(Some("hello".into())),
                ..Changes::default()
            },
        ),
        (
            json!({ "name": "new" }),
            Changes {
                name: Patch::Value("new".into()),
                ..Changes::default()
            },
        ),
    ] {
        let changes: Changes = from_value(wire.clone()).unwrap();
        assert_eq!(changes, expected);
        assert_eq!(to_value(changes).unwrap(), wire);
    }
}

#[test]
fn nonnullable_updates_reject_null_and_unknown_fields_remain_rejected() {
    for invalid in [
        json!({ "name": null }),
        json!({ "bio": 7 }),
        json!({ "typo": true }),
    ] {
        assert!(from_value::<Changes>(invalid).is_err());
    }
}

#[test]
fn missing_is_not_silently_serialized_as_null() {
    assert!(to_value(Patch::<String>::Missing)
        .unwrap_err()
        .to_string()
        .contains("skip_serializing_if"));
    #[derive(Serialize)]
    struct ForgotSkip {
        bio: Patch<Option<String>>,
    }
    assert!(to_value(ForgotSkip {
        bio: Patch::Missing
    })
    .is_err());
    assert_eq!(to_value(Patch::Value(None::<String>)).unwrap(), json!(null));
}

#[test]
fn missing_requires_a_field_default_even_for_nullable_payloads() {
    #[derive(Debug, Deserialize)]
    struct Required {
        bio: Patch<Option<String>>,
    }
    assert!(from_value::<Required>(json!({})).is_err());
    assert_eq!(
        from_value::<Required>(json!({ "bio": null })).unwrap().bio,
        Patch::Value(None)
    );
    assert_eq!(NullableUpdate::missing_field(), MissingField::Rejected);
    struct NoDefault;
    assert!(Patch::<NoDefault>::default().is_missing());
}

#[test]
fn schema_preserves_field_presence_independently_from_payload_nullability() {
    let mut registry = Registry::new();
    let input = registry
        .register::<Changes>(Direction::Deserialize)
        .unwrap();
    let output = registry.register::<Changes>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    for shape in [input, output] {
        let Shape::Ref(id) = shape else {
            panic!("expected a named DTO")
        };
        let Shape::Object(object) = &graph.definitions()[&id].shape else {
            panic!("expected object")
        };
        assert!(object
            .fields
            .iter()
            .all(|field| field.presence == Presence::Optional));
        assert_eq!(object.fields[0].shape, Shape::String);
        assert_eq!(
            object.fields[1].shape,
            Shape::Union(vec![Shape::String, Shape::Null])
        );
    }
}
