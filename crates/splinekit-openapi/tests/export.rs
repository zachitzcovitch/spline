mod support;

use serde_json::json;
use splinekit::http::{
    contract::{ContractBuilder, HttpBinding, HttpMethod},
    DeclaredErrors, StatusCode,
};
use splinekit::{Context, DefinitionId, Direction, Schema, Shape, TypeInfo};
use splinekit_openapi::{Error, OpenApi};
use support::*;

#[derive(Schema)]
struct Args {}

fn binding<O: Schema>() -> splinekit::http::contract::ContractDescription {
    let mut neutral = splinekit::contract::ContractBuilder::new("test", "").unwrap();
    neutral
        .operation::<Args, O, ()>(
            "value",
            "value",
            splinekit::contract::OperationKind::Query,
            "",
        )
        .unwrap();
    let mut builder = ContractBuilder::new(neutral.finish().unwrap());
    builder
        .bind(
            "test.value",
            HttpBinding {
                method: HttpMethod::Get,
                path: "/value".into(),
                body: None,
                success_status: 200,
            },
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn independent_schema_validation_preserves_serde_directions_recursion_tuples_and_patch_states() {
    let doc = OpenApi::new("Sample", "1")
        .export_contract(&sample::http::describe_with(&DeclaredErrors).unwrap())
        .unwrap();
    let input: Profile = serde_json::from_value(json!({"id":7})).unwrap();
    let output = serde_json::to_value(input).unwrap();
    let pair = Pair((
        "label".into(),
        Node {
            value: "root".into(),
            next: Some(Box::new(Node {
                value: "child".into(),
                next: None,
            })),
        },
    ));
    independent(
        &doc,
        json!([
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":7},"valid":true},
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":4294967295u64},"valid":true},
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":4294967296u64},"valid":false},
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":-1},"valid":false},
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":7,"extra":true},"valid":false},
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":7,"counters":{"__proto__":65535}},"valid":true},
            {"pointer":"#/components/schemas/ProfileInput","value":{"id":7,"counters":{"x":65536}},"valid":false},
            {"pointer":"#/components/schemas/ProfileOutput","value":output,"valid":true},
            {"pointer":"#/components/schemas/ProfileOutput","value":{"id":7},"valid":false},
            {"pointer":"#/components/schemas/Pair","value":serde_json::to_value(pair).unwrap(),"valid":true},
            {"pointer":"#/components/schemas/Pair","value":["label"],"valid":false},
            {"pointer":"#/components/schemas/Pair","value":["label",{"value":"root","next":null},"extra"],"valid":false},
            {"pointer":"#/components/schemas/Node","value":{"value":"root","next":{"value":"child","next":null}},"valid":true},
            {"pointer":"#/components/schemas/Delta","value":serde_json::to_value(Delta::default()).unwrap(),"valid":true},
            {"pointer":"#/components/schemas/Delta","value":{"note":null},"valid":true},
            {"pointer":"#/components/schemas/Delta","value":{"note":"new"},"valid":true},
            {"pointer":"#/components/schemas/Delta","value":{"note":false},"valid":false},
            {"pointer":"#/paths/~1profiles~1{id}/get/parameters/1/schema","value":"first","valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/parameters/1/schema","value":"unknown","valid":false},
            {"pointer":"#/paths/~1profiles~1{id}/get/parameters/1/schema","value":null,"valid":false},
            {"pointer":"#/paths/~1nullable/post/requestBody/content/application~1json/schema","value":null,"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/400/content/application~1json/schema","value":{"kind":"application","value":{"kind":"invalid_value","details":{"message":"bad"}}},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/400/content/application~1json/schema","value":{"kind":"application","value":{"kind":"rejected","details":"bad"}},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/400/content/application~1json/schema","value":{"kind":"framework","code":"invalid_request","message":"bad"},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/400/content/application~1json/schema","value":{"kind":"application","value":{"kind":"missing"}},"valid":false},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/404/content/application~1json/schema","value":{"kind":"application","value":{"kind":"missing"}},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/404/content/application~1json/schema","value":{"kind":"framework","code":"hidden","message":"not visible"},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/500/content/application~1json/schema","value":{"kind":"application","value":{"kind":"internal","details":{"trace":"x"}}},"valid":true}
        ]),
    );
    let components = doc.as_json()["components"]["schemas"].as_object().unwrap();
    assert!(components.contains_key("PageOfNode"));
    assert!(components.keys().all(|name| !name.contains("Args")));
    assert!(components["ProfileInput"]["properties"]["labels"]
        .get("default")
        .is_none());
}

#[test]
fn transport_mapping_and_deployment_options_are_explicit_and_inspectable() {
    let mut doc = OpenApi::new("Sample API", "2026-10")
        .server("/api")
        .bearer_auth("access")
        .export_contract(&sample::http::describe_with(&DeclaredErrors).unwrap())
        .unwrap();
    let value = doc.as_json();
    assert_eq!(value["openapi"], "3.1.2");
    assert_eq!(value["servers"][0]["url"], "/api");
    assert_eq!(value["security"], json!([{"access":[]}]));
    assert_eq!(
        value["components"]["securitySchemes"]["access"],
        json!({"type":"http","scheme":"bearer"})
    );
    let get = &value["paths"]["/profiles/{id}"]["get"];
    assert_eq!(get["operationId"], "sample.get");
    assert_eq!(get["parameters"][0]["required"], true);
    assert_eq!(get["parameters"][1]["required"], false);
    assert!(get["parameters"][1]["description"]
        .as_str()
        .unwrap()
        .contains("Select the profile kind."));
    assert_eq!(
        value["paths"]["/nullable"]["post"]["requestBody"]["required"],
        true
    );
    assert!(
        value["paths"]["/profiles/{id}"]["delete"]["responses"]["204"]
            .get("content")
            .is_none()
    );
    assert_eq!(
        value["paths"]["/unit"]["get"]["responses"]["200"]["content"]["application/json"]["schema"],
        json!({"type":"null"})
    );
    assert!(get["responses"]["400"].get("headers").is_none());
    assert!(get.get("x-splinekit-error-policy").is_none());
    assert!(value["components"]["schemas"].get("ServiceError").is_some());
    assert!(value["components"]["responses"]
        .get("ServiceErrorResponse")
        .is_some());
    assert!(!doc
        .to_pretty_json()
        .unwrap()
        .to_ascii_lowercase()
        .contains("splinekit"));
    doc.as_json_mut()["info"]["description"] =
        "Splinekit is this application's authored name".into();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&doc.to_json().unwrap()).unwrap(),
        *doc.as_json()
    );
    assert!(doc.to_pretty_json().unwrap().ends_with('\n'));
    independent(&doc, json!([]));
}

#[test]
fn opaque_policies_document_full_errors_without_inventing_status_mappings() {
    let policy = |_: &Failure| StatusCode::IM_A_TEAPOT;
    let binding = sample::http::describe_with(&policy).unwrap();
    let doc = OpenApi::new("Opaque", "1")
        .export_contract(&binding)
        .unwrap();
    let responses = &doc.as_json()["paths"]["/profiles/{id}"]["get"]["responses"];
    assert!(responses.get("404").is_none());
    assert!(responses.get("418").is_none());
    independent(
        &doc,
        json!([
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/4XX/content/application~1json/schema","value":{"kind":"application","value":{"kind":"missing"}},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/500/content/application~1json/schema","value":{"kind":"application","value":{"kind":"missing"}},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/400/content/application~1json/schema","value":{"kind":"framework","code":"custom","message":"bad"},"valid":true},
            {"pointer":"#/paths/~1profiles~1{id}/get/responses/4XX/content/application~1json/schema","value":{"kind":"application","value":{"kind":"new_variant"}},"valid":false}
        ]),
    );
}

#[test]
fn overlapping_unions_and_empty_tuples_remain_valid_json_schema() {
    struct Overlap;
    impl Schema for Overlap {
        fn info() -> TypeInfo {
            TypeInfo::named("test.Overlap")
        }
        fn describe(_: &mut Context<'_>) -> splinekit::Result<Shape> {
            Ok(Shape::Union(vec![
                Shape::Null,
                Shape::Null,
                Shape::Tuple(vec![]),
            ]))
        }
    }
    let doc = OpenApi::new("Overlap", "1")
        .export_contract(&binding::<Overlap>())
        .unwrap();
    independent(
        &doc,
        json!([
            {"pointer":"#/components/schemas/Overlap","value":null,"valid":true},
            {"pointer":"#/components/schemas/Overlap","value":[],"valid":true},
            {"pointer":"#/components/schemas/Overlap","value":[null],"valid":false},
            {"pointer":"#/components/schemas/Overlap","value":false,"valid":false}
        ]),
    );
}

#[test]
fn component_collisions_directional_overrides_and_unguarded_recursion_are_checked() {
    mod first {
        #[derive(serde::Serialize, serde::Deserialize, splinekit::Schema)]
        #[splinekit(name = "first.Record")]
        pub struct Record {
            pub value: String,
        }
    }
    mod second {
        #[derive(serde::Serialize, serde::Deserialize, splinekit::Schema)]
        #[splinekit(name = "second.Record")]
        pub struct Record {
            pub value: String,
        }
    }
    #[derive(Schema)]
    #[allow(dead_code)]
    struct Both {
        first: first::Record,
        second: second::Record,
    }
    let export = OpenApi::new("Collisions", "1");
    assert!(matches!(
        export.export_contract(&binding::<Both>()),
        Err(Error::ComponentCollision(_))
    ));
    let id = DefinitionId {
        name: "first.Record".into(),
        direction: Direction::Serialize,
    };
    let doc = export
        .clone()
        .name(id.clone(), "FirstRecord")
        .export_contract(&binding::<Both>())
        .unwrap();
    assert!(doc.as_json()["components"]["schemas"]
        .get("FirstRecord")
        .is_some());
    assert!(matches!(
        export
            .name(id, "bad/name")
            .export_contract(&binding::<Both>()),
        Err(Error::InvalidComponentName(_))
    ));
    let profile_input = DefinitionId {
        name: Profile::info().name,
        direction: Direction::Deserialize,
    };
    let profile_output = DefinitionId {
        name: Profile::info().name,
        direction: Direction::Serialize,
    };
    assert!(matches!(
        OpenApi::new("Directions", "1")
            .name(profile_input, "Unified")
            .name(profile_output, "Unified")
            .export_contract(&sample::http::describe().unwrap()),
        Err(Error::ComponentCollision(_))
    ));
    struct Cycle;
    impl Schema for Cycle {
        fn info() -> TypeInfo {
            TypeInfo::named("test.Cycle")
        }
        fn describe(cx: &mut Context<'_>) -> splinekit::Result<Shape> {
            cx.schema::<Self>()
        }
    }
    assert!(matches!(
        OpenApi::new("Cycle", "1").export_contract(&binding::<Cycle>()),
        Err(Error::UnguardedRecursion(_))
    ));
}

#[test]
fn export_is_deterministic_across_resource_registration_order_and_keeps_logical_ids() {
    #[allow(dead_code)]
    pub mod other {
        #[splinekit::contract]
        pub trait Other {
            #[query]
            #[http(get = "/other")]
            async fn get(&self) -> Result<(), ()>;
        }
    }
    fn application(reverse: bool) -> splinekit::http::contract::ApplicationDescription {
        let mut builder = splinekit::contract::ApplicationBuilder::new("An application");
        if reverse {
            other::register(&mut builder, "other").unwrap();
        }
        sample::register(&mut builder, "people").unwrap();
        if !reverse {
            other::register(&mut builder, "other").unwrap();
        }
        let mut builder =
            splinekit::http::contract::ApplicationBuilder::new(builder.finish().unwrap());
        let mut bindings = sample::http::__bindings();
        bindings.extend(other::http::__bindings());
        if reverse {
            bindings.reverse();
        }
        for (id, binding) in bindings {
            builder.bind(&id, binding).unwrap();
        }
        builder.finish().unwrap()
    }
    let export = OpenApi::new("Application", "1");
    let first = export.export_application(&application(false)).unwrap();
    assert_eq!(
        first.to_pretty_json().unwrap(),
        export
            .export_application(&application(true))
            .unwrap()
            .to_pretty_json()
            .unwrap()
    );
    assert_eq!(
        first.as_json()["paths"]["/profiles/{id}"]["get"]["tags"],
        json!(["people"])
    );
    assert_eq!(
        first.as_json()["paths"]["/profiles/{id}"]["get"]["operationId"],
        "sample.get"
    );
    assert!(first.as_json().get("security").is_none());
}

#[test]
fn malformed_deployment_configuration_is_rejected_before_export() {
    let binding = binding::<()>();
    for server in [
        "",
        "ftp://example.com",
        "//example.com",
        "https://name:secret@example.com",
        "/api?x=1",
        "/api#part",
        "/{prefix}",
        "https://example.com/white space",
    ] {
        assert!(
            OpenApi::new("Example", "1")
                .server(server)
                .export_contract(&binding)
                .is_err(),
            "{server}"
        );
    }
    assert!(OpenApi::new("", "1").export_contract(&binding).is_err());
    assert!(OpenApi::new("Example", " ")
        .export_contract(&binding)
        .is_err());
    assert!(OpenApi::new("Example", "1")
        .bearer_auth("bad/name")
        .export_contract(&binding)
        .is_err());
    OpenApi::new("Example", "1")
        .server("https://example.com/api")
        .export_contract(&binding)
        .unwrap();
}

#[test]
fn formatted_strings_floats_and_safe_integers_keep_their_constraints() {
    #[derive(serde::Serialize, splinekit::Schema)]
    #[splinekit(name = "test.Formats")]
    #[allow(dead_code)]
    struct Formats {
        #[splinekit(schema = "splinekit::wire::Uuid")]
        id: String,
        #[splinekit(schema = "splinekit::wire::DateTime")]
        at: String,
        #[splinekit(schema = "splinekit::wire::Decimal")]
        amount: String,
        #[splinekit(schema = "splinekit::wire::IntegerString<i64>")]
        big: String,
        #[splinekit(schema = "splinekit::wire::SafeInteger<u64>")]
        safe: u64,
        ratio: f32,
    }
    let doc = OpenApi::new("Formats", "1")
        .export_contract(&binding::<Formats>())
        .unwrap();
    let properties = &doc.as_json()["components"]["schemas"]["Formats"]["properties"];
    assert_eq!(properties["id"], json!({"type":"string","format":"uuid"}));
    assert_eq!(
        properties["at"],
        json!({"type":"string","format":"date-time"})
    );
    assert_eq!(
        properties["ratio"],
        json!({"type":"number","format":"float"})
    );
    let valid = json!({
        "id": "0191f1b2-7c4e-7a3b-9f1e-2a3b4c5d6e7f",
        "at": "2026-11-11T05:06:53Z",
        "amount": "-12.50",
        "big": "-9223372036854775808",
        "safe": 9_007_199_254_740_991_u64,
        "ratio": 0.5,
    });
    let with = |name: &str, value: serde_json::Value| {
        let mut changed = valid.clone();
        changed[name] = value;
        json!({"pointer":"#/components/schemas/Formats","value":changed,"valid":false})
    };
    independent(
        &doc,
        json!([
            {"pointer":"#/components/schemas/Formats","value":valid,"valid":true},
            with("amount", json!("1e5")),
            with("amount", json!("01.5")),
            with("big", json!("1.5")),
            with("big", json!(7)),
            with("safe", json!(9_007_199_254_740_992_u64)),
            with("safe", json!(-1)),
            with("ratio", json!("0.5")),
            with("id", json!(7)),
        ]),
    );
}

#[test]
fn open_enums_list_their_known_values_and_accept_any_string() {
    #[derive(serde::Serialize, splinekit::Schema)]
    #[splinekit(name = "test.Status")]
    #[serde(rename_all = "snake_case")]
    #[allow(dead_code)]
    enum Status {
        Active,
        PastDue,
    }
    #[derive(serde::Serialize, splinekit::Schema)]
    #[splinekit(name = "test.Plan")]
    #[allow(dead_code)]
    struct Plan {
        status: splinekit::Open<Status>,
    }
    let doc = OpenApi::new("Open", "1")
        .export_contract(&binding::<Plan>())
        .unwrap();
    assert_eq!(
        doc.as_json()["components"]["schemas"]["Plan"]["properties"]["status"],
        json!({"anyOf":[{"$ref":"#/components/schemas/Status"},{"type":"string"}]})
    );
    independent(
        &doc,
        json!([
            {"pointer":"#/components/schemas/Plan","value":{"status":"active"},"valid":true},
            {"pointer":"#/components/schemas/Plan","value":{"status":"paused"},"valid":true},
            {"pointer":"#/components/schemas/Plan","value":{"status":7},"valid":false},
            {"pointer":"#/components/schemas/Status","value":"paused","valid":false}
        ]),
    );
}
