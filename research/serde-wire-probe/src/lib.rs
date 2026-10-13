//! Behavioral evidence for the schema research, using Serde directly.

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::{from_str, from_value, json, to_string, to_value};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct User {
        display_name: String,
        nickname: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bio: Option<String>,
    }

    #[test]
    fn missing_nullable_defaulted_and_skipped_fields_are_distinct() {
        let user: User = from_value(json!({ "displayName": "Zach" })).unwrap();
        assert_eq!(user.nickname, None);
        assert!(user.tags.is_empty());
        assert_eq!(user.bio, None);

        let serialized = to_value(&user).unwrap();
        assert_eq!(
            serialized,
            json!({ "displayName": "Zach", "nickname": null, "tags": [] })
        );
        assert_eq!(from_value::<User>(serialized).unwrap(), user);

        assert!(from_value::<User>(json!({ "displayName": "Zach", "tags": null })).is_err());
        assert!(from_value::<User>(json!({ "displayName": "Zach", "nickname": 3 })).is_err());
    }

    #[test]
    fn an_is_none_predicate_omits_none_and_preserves_some() {
        let user: User = from_value(json!({
            "displayName": "Zach",
            "bio": "Hello"
        }))
        .unwrap();
        assert_eq!(to_value(user).unwrap()["bio"], "Hello");
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind", content = "details", rename_all = "snake_case")]
    enum ApiError {
        NotFound,
        Validation { field: String },
    }

    #[test]
    fn adjacent_enum_tags_have_variant_specific_payloads() {
        assert_eq!(
            to_value(ApiError::NotFound).unwrap(),
            json!({ "kind": "not_found" })
        );
        let error = ApiError::Validation {
            field: "title".into(),
        };
        let value = to_value(&error).unwrap();
        assert_eq!(
            value,
            json!({ "kind": "validation", "details": { "field": "title" } })
        );
        assert_eq!(from_value::<ApiError>(value).unwrap(), error);
    }

    #[derive(Debug, PartialEq)]
    struct UserId(u64);

    impl Serialize for UserId {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&self.0.to_string())
        }
    }

    impl<'de> Deserialize<'de> for UserId {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let value = String::deserialize(deserializer)?;
            value.parse().map(UserId).map_err(serde::de::Error::custom)
        }
    }

    #[test]
    fn custom_newtype_representation_is_not_its_rust_field_type() {
        let id = UserId(u64::MAX);
        assert_eq!(to_string(&id).unwrap(), "\"18446744073709551615\"");
        assert_eq!(from_value::<UserId>(to_value(&id).unwrap()).unwrap(), id);
        assert!(from_value::<UserId>(json!(42)).is_err());
        assert!(from_value::<UserId>(json!("invalid")).is_err());
        assert!(from_value::<UserId>(json!("18446744073709551616")).is_err());
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct DirectionalName {
        #[serde(rename(serialize = "out", deserialize = "in"))]
        name: String,
    }

    #[test]
    fn sharing_a_rust_type_does_not_guarantee_wire_round_trips() {
        let value = to_value(DirectionalName {
            name: "Zach".into(),
        })
        .unwrap();
        assert_eq!(value, json!({ "out": "Zach" }));
        assert!(from_value::<DirectionalName>(value).is_err());
        assert_eq!(
            from_value::<DirectionalName>(json!({ "in": "Zach" }))
                .unwrap()
                .name,
            "Zach"
        );
    }

    fn custom_optional<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<String>, D::Error> {
        Option::<String>::deserialize(deserializer)
    }

    #[derive(Debug, Deserialize)]
    struct RequiredOption {
        #[serde(deserialize_with = "custom_optional")]
        nickname: Option<String>,
    }

    #[test]
    fn deserialize_with_changes_missing_option_behavior() {
        assert!(from_value::<RequiredOption>(json!({})).is_err());
        assert_eq!(
            from_value::<RequiredOption>(json!({ "nickname": null }))
                .unwrap()
                .nickname,
            None
        );
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Strict {
        name: String,
    }

    #[test]
    fn unknown_field_policy_is_separate_from_known_field_types() {
        assert!(from_value::<User>(json!({ "displayName": "Zach", "extra": true })).is_ok());
        assert_eq!(
            from_value::<Strict>(json!({ "name": "Zach" }))
                .unwrap()
                .name,
            "Zach"
        );
        assert!(from_value::<Strict>(json!({ "name": "Zach", "extra": true })).is_err());
    }

    #[test]
    fn serde_json_can_accept_sequence_form_for_a_named_struct() {
        assert_eq!(from_str::<Strict>("[\"Zach\"]").unwrap().name, "Zach");
    }

    #[test]
    fn non_finite_floats_serialize_to_null_but_cannot_round_trip_as_f64() {
        assert_eq!(to_string(&f64::NAN).unwrap(), "null");
        assert_eq!(to_string(&f64::INFINITY).unwrap(), "null");
        assert!(from_str::<f64>("null").is_err());
    }

    #[test]
    fn raw_u64_serializes_as_a_json_number() {
        assert_eq!(to_string(&u64::MAX).unwrap(), "18446744073709551615");
    }
}
