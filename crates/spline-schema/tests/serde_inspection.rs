#[path = "support/parser/mod.rs"]
mod parser;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use parser::{
    inspect, Codec, Container, Conversion, Data, DefaultRule, Diagnostic, DiagnosticOrigin,
    Direction::{Deserialize as Input, Serialize as Output},
    Field, Style, Tagging,
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, from_value, json, to_value};

// The parser and actual Serde derives see the same declaration, avoiding a
// separately maintained string fixture that could drift from the tested type.
macro_rules! fixture {
    ($source:ident, $($item:tt)*) => {
        const $source: &str = stringify!($($item)*);
        $($item)*
    };
}

fn fields(container: &Container) -> &[Field] {
    match &container.data {
        Data::Struct { fields, .. } => fields,
        Data::Enum(_) => panic!("expected a struct"),
    }
}

#[test]
fn directional_names_aliases_and_raw_identifiers_match_serde() {
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
        struct User {
            display_name: String,
            #[serde(rename(serialize = "id", deserialize = "user_id"), alias = "legacy_id")]
            id: u32,
            r#type: String,
        }
    );
    let input = inspect(SOURCE, Input).unwrap();
    let output = inspect(SOURCE, Output).unwrap();
    assert_eq!(fields(&input)[0].wire_name, "display_name");
    assert_eq!(fields(&output)[0].wire_name, "displayName");
    assert_eq!(fields(&input)[1].aliases, ["legacy_id"]);
    assert!(fields(&output)[1].aliases.is_empty());
    assert_eq!(fields(&output)[2].wire_name, "type");

    let user: User =
        from_value(json!({ "display_name": "Zach", "legacy_id": 7, "type": "author" })).unwrap();
    let emitted = to_value(&user).unwrap();
    assert_eq!(
        emitted,
        json!({ "displayName": "Zach", "id": 7, "type": "author" })
    );
    for field in fields(&output) {
        assert!(emitted.get(&field.wire_name).is_some());
    }
    assert!(from_value::<User>(emitted).is_err());
    assert!(from_value::<User>(
        json!({ "display_name": "Zach", "user_id": 7, "legacy_id": 8, "type": "author" })
    )
    .is_err());
}

#[test]
fn defaults_and_omission_are_exposed_without_guessing_option_presence() {
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        struct User {
            nickname: Option<String>,
            #[serde(default)]
            tags: Vec<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            bio: Option<String>,
            #[serde(skip)]
            internal: bool,
        }
    );
    let input = inspect(SOURCE, Input).unwrap();
    let output = inspect(SOURCE, Output).unwrap();
    assert_eq!(
        fields(&input)[0].deserialize_default,
        DefaultRule::Unspecified
    );
    assert_eq!(fields(&input)[1].deserialize_default, DefaultRule::Trait);
    assert_eq!(
        fields(&output)[2].omit_if.as_deref(),
        Some("Option :: is_none")
    );
    assert_eq!(fields(&input)[2].omit_if, None);
    assert!(fields(&input)[3].skipped);
    assert!(fields(&output)[3].skipped);
    let user: User = from_value(json!({})).unwrap();
    assert_eq!(user.nickname, None);
    assert!(!user.internal);
    assert_eq!(
        to_value(user).unwrap(),
        json!({ "nickname": null, "tags": [] })
    );
}

#[test]
fn container_default_and_default_functions_remain_metadata() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn default_limit() -> u32 {
        CALLS.fetch_add(1, Ordering::SeqCst);
        25
    }
    fixture!(
        SOURCE,
        #[derive(Debug, Default, Serialize, Deserialize)]
        #[serde(default)]
        struct Options {
            page: u32,
            #[serde(default = "default_limit")]
            limit: u32,
        }
    );
    let description = inspect(SOURCE, Input).unwrap();
    assert_eq!(description.deserialize_default, DefaultRule::Trait);
    assert_eq!(
        fields(&description)[0].deserialize_default,
        DefaultRule::Unspecified
    );
    assert_eq!(
        fields(&description)[1].deserialize_default,
        DefaultRule::Function("default_limit".into())
    );
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    let options: Options = from_value(json!({})).unwrap();
    assert_eq!((options.page, options.limit), (0, 25));
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn adjacent_enum_names_and_variant_field_precedence_match_serde() {
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(
            tag = "kind",
            content = "details",
            rename_all = "snake_case",
            rename_all_fields = "camelCase"
        )]
        enum Error {
            #[serde(alias = "missing")]
            NotFound,
            InvalidField {
                field_name: String,
            },
            #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
            Override {
                field_name: String,
            },
        }
    );
    let output = inspect(SOURCE, Output).unwrap();
    assert_eq!(
        output.tagging,
        Tagging::Adjacent {
            tag: "kind".into(),
            content: "details".into()
        }
    );
    let Data::Enum(variants) = output.data else {
        panic!("expected enum")
    };
    assert_eq!(variants[0].wire_name, "not_found");
    assert_eq!(variants[0].style, Style::Unit);
    assert_eq!(variants[1].fields[0].wire_name, "fieldName");
    assert_eq!(variants[2].fields[0].wire_name, "FIELD_NAME");
    assert_eq!(
        to_value(Error::NotFound).unwrap(),
        json!({ "kind": "not_found" })
    );
    assert_eq!(
        to_value(Error::InvalidField {
            field_name: "title".into()
        })
        .unwrap(),
        json!({ "kind": "invalid_field", "details": { "fieldName": "title" } })
    );
    assert_eq!(
        to_value(Error::Override {
            field_name: "title".into()
        })
        .unwrap(),
        json!({ "kind": "override", "details": { "FIELD_NAME": "title" } })
    );
    assert!(matches!(
        from_value::<Error>(json!({ "kind": "missing" })).unwrap(),
        Error::NotFound
    ));
    let Data::Enum(input) = inspect(SOURCE, Input).unwrap().data else {
        panic!("expected enum")
    };
    assert_eq!(input[0].aliases, ["missing"]);
}

#[test]
fn all_four_enum_tagging_modes_are_reported() {
    fixture!(
        EXTERNAL,
        #[derive(Serialize)]
        enum External {
            Value(u32),
        }
    );
    fixture!(
        INTERNAL,
        #[derive(Serialize)]
        #[serde(tag = "kind")]
        enum Internal {
            Value { value: u32 },
        }
    );
    fixture!(
        UNTAGGED,
        #[derive(Serialize)]
        #[serde(untagged)]
        enum Untagged {
            Value(u32),
        }
    );
    assert_eq!(
        inspect(EXTERNAL, Output).unwrap().tagging,
        Tagging::External
    );
    assert_eq!(
        inspect(INTERNAL, Output).unwrap().tagging,
        Tagging::Internal { tag: "kind".into() }
    );
    assert_eq!(
        inspect(UNTAGGED, Output).unwrap().tagging,
        Tagging::Untagged
    );
    assert_eq!(to_value(External::Value(7)).unwrap(), json!({ "Value": 7 }));
    assert_eq!(
        to_value(Internal::Value { value: 7 }).unwrap(),
        json!({ "kind": "Value", "value": 7 })
    );
    assert_eq!(to_value(Untagged::Value(7)).unwrap(), json!(7));
}

#[test]
fn transparent_members_are_selected_per_direction() {
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(transparent)]
        struct Split {
            #[serde(skip_deserializing)]
            emitted: String,
            #[serde(skip_serializing)]
            accepted: u32,
        }
    );
    let input = inspect(SOURCE, Input).unwrap();
    let output = inspect(SOURCE, Output).unwrap();
    assert!(fields(&input)[1].transparent_member);
    assert!(!fields(&input)[0].transparent_member);
    assert!(fields(&output)[0].transparent_member);
    assert!(!fields(&output)[1].transparent_member);
    assert_eq!(fields(&input)[0].deserialize_default, DefaultRule::Trait);
    assert_eq!(
        to_value(Split {
            emitted: "seven".into(),
            accepted: 7
        })
        .unwrap(),
        json!("seven")
    );
    let value: Split = from_value(json!(7)).unwrap();
    assert_eq!(value.accepted, 7);
    assert_eq!(value.emitted, "");
}

#[test]
fn serialize_only_transparent_types_need_serialize_validation() {
    fixture!(
        SOURCE,
        #[derive(Serialize)]
        #[serde(transparent)]
        struct Defaulted(#[serde(default)] String);
    );
    assert!(inspect(SOURCE, Output).is_ok());
    let errors = inspect(SOURCE, Input).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.message.contains("neither skipped nor has a default")));
    assert_eq!(to_value(Defaulted("hello".into())).unwrap(), json!("hello"));
}

#[test]
fn with_exposes_codec_paths_without_inferring_the_wire_type() {
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
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        struct Id {
            #[serde(with = "decimal")]
            value: u64,
        }
    );
    let input = inspect(SOURCE, Input).unwrap();
    let output = inspect(SOURCE, Output).unwrap();
    assert_eq!(
        fields(&input)[0].codec,
        Some(Codec::Function("decimal :: deserialize".into()))
    );
    assert_eq!(
        fields(&output)[0].codec,
        Some(Codec::Function("decimal :: serialize".into()))
    );
    assert_eq!(fields(&output)[0].rust_type, "u64");
    assert_eq!(
        to_value(Id { value: u64::MAX }).unwrap(),
        json!({ "value": "18446744073709551615" })
    );
    assert_eq!(from_value::<Id>(json!({ "value": "7" })).unwrap().value, 7);
    assert!(from_value::<Id>(json!({ "value": 7 })).is_err());
}

#[test]
fn container_conversions_are_directional() {
    fixture!(
        SOURCE,
        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        struct Id(u32);
    );
    impl TryFrom<String> for Id {
        type Error = std::num::ParseIntError;
        fn try_from(value: String) -> Result<Self, Self::Error> {
            value.parse().map(Self)
        }
    }
    impl From<Id> for String {
        fn from(value: Id) -> Self {
            value.0.to_string()
        }
    }
    assert_eq!(
        inspect(SOURCE, Input).unwrap().conversion,
        Some(Conversion::TryFrom("String".into()))
    );
    assert_eq!(
        inspect(SOURCE, Output).unwrap().conversion,
        Some(Conversion::Into("String".into()))
    );
    assert_eq!(from_value::<Id>(json!("42")).unwrap().0, 42);
    assert_eq!(to_value(Id(42)).unwrap(), json!("42"));
    assert!(from_value::<Id>(json!("nope")).is_err());
}

#[test]
fn borrowed_cow_synthetic_helpers_stay_inside_the_adapter() {
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        struct Borrowed<'a> {
            #[serde(borrow)]
            value: Cow<'a, str>,
        }
    );
    let input = inspect(SOURCE, Input).unwrap();
    assert_eq!(fields(&input)[0].codec, Some(Codec::BorrowedCowString));
    assert_eq!(fields(&input)[0].borrowed_lifetimes, ["'a"]);
    assert_eq!(fields(&inspect(SOURCE, Output).unwrap())[0].codec, None);
    let value: Borrowed<'_> = from_str(r#"{"value":"hello"}"#).unwrap();
    assert!(matches!(value.value, Cow::Borrowed("hello")));
    assert_eq!(to_value(value).unwrap(), json!({ "value": "hello" }));
}

#[test]
fn flatten_is_detectable_but_still_needs_spline_lowering() {
    fixture!(
        SOURCE,
        #[derive(Debug, Serialize, Deserialize)]
        struct Bag {
            name: String,
            #[serde(flatten)]
            extra: BTreeMap<String, u32>,
        }
    );
    assert!(fields(&inspect(SOURCE, Input).unwrap())[1].flattened);
    let bag: Bag = from_value(json!({ "name": "bag", "count": 3 })).unwrap();
    assert_eq!(bag.extra["count"], 3);
    assert_eq!(to_value(bag).unwrap(), json!({ "name": "bag", "count": 3 }));
}

#[test]
fn unknown_field_policy_matches_deserialization() {
    fixture!(
        SOURCE,
        #[derive(Debug, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Strict {
            name: String,
        }
    );
    assert!(inspect(SOURCE, Input).unwrap().deny_unknown_fields);
    assert_eq!(
        from_value::<Strict>(json!({ "name": "Zach" }))
            .unwrap()
            .name,
        "Zach"
    );
    assert!(from_value::<Strict>(json!({ "name": "Zach", "extra": 3 })).is_err());
}

#[test]
fn generic_syntax_bounds_and_docs_are_retained_without_type_resolution() {
    let source = r#"
        /// A page of records.
        #[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
        struct Page<T> where T: Clone {
            /// The records.
            items: Vec<T>,
            next: Option<Box<Page<T>>>,
        }
    "#;
    let input = inspect(source, Input).unwrap();
    let output = inspect(source, Output).unwrap();
    assert_eq!(input.generics, "< T >");
    assert_eq!(input.where_clause.as_deref(), Some("where T : Clone"));
    assert_eq!(input.docs, ["A page of records."]);
    assert_eq!(fields(&input)[0].docs, ["The records."]);
    assert_eq!(fields(&input)[0].rust_type, "Vec < T >");
    assert_eq!(fields(&input)[1].rust_type, "Option < Box < Page < T > > >");
    assert_eq!(input.explicit_bounds.unwrap(), ["T : Deserialize < 'de >"]);
    assert_eq!(output.explicit_bounds.unwrap(), ["T : Serialize"]);
}

#[test]
fn diagnostics_are_aggregated_and_retain_source_locations() {
    let source =
        "struct Bad {\n    #[serde(unknown_a)]\n    a: u8,\n    #[serde(unknown_b)]\n    b: u8,\n}";
    let errors = inspect(source, Input).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(errors[0].message.contains("unknown_a"));
    assert!(errors[1].message.contains("unknown_b"));
    assert_eq!(errors[0].origin, DiagnosticOrigin::Serde);
    assert_eq!(errors[0].start.line, 2);
    assert_eq!(errors[1].start.line, 4);
    assert!(errors[0].end.column > errors[0].start.column);
}

#[test]
fn syntax_errors_and_union_rejection_return_diagnostics_without_panicking() {
    let syntax = inspect("struct Broken {", Input).unwrap_err();
    assert_eq!(syntax[0].origin, DiagnosticOrigin::Syntax);
    let union = inspect("union Unsupported { value: u32 }", Input).unwrap_err();
    assert!(union[0]
        .message
        .contains("does not support derive for unions"));
}

#[test]
fn invalid_tagging_is_rejected_by_the_shared_serde_checks() {
    let source = include_str!("ui/invalid_tag.rs");
    // The fixture's declaration is shared with a real compiler-failure test.
    let declaration = source.split("fn main()").next().unwrap();
    let declaration = declaration.replace("use serde::Deserialize;", "");
    let errors = inspect(&declaration, Input).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.message.contains("conflict with each other")));
}

#[test]
fn renaming_rules_and_non_serde_attributes_are_handled_at_distinct_boundaries() {
    let errors = inspect(
        "#[serde(rename_all = \"camelcase\")] struct Bad { field: u8 }",
        Output,
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| e.message.contains("rename rule")));
    let own_attribute = "#[spline(schema(unknown_setting))] struct Plain { value: u32 }";
    assert!(inspect(own_attribute, Output).is_ok());
}

#[test]
fn parsing_success_does_not_imply_serde_derive_or_rust_type_check_success() {
    for source in [
        include_str!("ui/reserved_lifetime.rs"),
        include_str!("ui/missing_codec.rs"),
    ] {
        let declaration = source.split("fn main()").next().unwrap();
        let declaration = declaration.replace("use serde::Deserialize;", "");
        assert!(inspect(&declaration, Input).is_ok());
    }
}

#[test]
fn public_metadata_is_owned_and_independent_of_parser_lifetimes() {
    fn assert_owned<T: Send + Sync + 'static>() {}
    assert_owned::<Container>();
    assert_owned::<Diagnostic>();
    let result = {
        let temporary_source = String::from("struct User { name: String }");
        inspect(&temporary_source, Input).unwrap()
    };
    assert_eq!(result.rust_name, "User");
}
