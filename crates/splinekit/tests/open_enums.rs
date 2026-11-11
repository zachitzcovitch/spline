use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use splinekit::*;

mod support;

mod v1 {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, splinekit::Schema)]
    #[splinekit(name = "test.Status")]
    #[serde(rename_all = "snake_case")]
    pub enum Status {
        Active,
        PastDue,
    }
}

mod v2 {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, splinekit::Schema)]
    #[splinekit(name = "test.Status")]
    #[serde(rename_all = "snake_case")]
    pub enum Status {
        Active,
        PastDue,
        Paused,
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "test.Plan")]
struct Plan {
    status: Open<v1::Status>,
}

#[test]
fn open_enums_describe_their_known_values_or_any_string() {
    for direction in [Direction::Serialize, Direction::Deserialize] {
        let (root, graph) = support::graph::<Plan>(direction);
        let status = &support::field(support::body(&root, &graph), "status").shape;
        let Shape::Open(known) = status else {
            panic!("expected an open enum, got {status:?}");
        };
        assert!(matches!(known.as_ref(), Shape::Ref(id) if id.name == Name::new("test.Status")));
        for value in [json!("active"), json!("paused"), json!("")] {
            assert!(support::matches(&graph, status, &value), "{value}");
        }
        assert!(!support::matches(&graph, status, &json!(7)));
        assert!(!support::matches(&graph, status, &json!(null)));
    }
}

#[test]
fn an_older_client_keeps_a_newer_servers_value_exactly() {
    let sent = to_value(Open::Known(v2::Status::Paused)).unwrap();
    assert_eq!(sent, json!("paused"));
    let received: Open<v1::Status> = from_value(sent).unwrap();
    assert_eq!(received, Open::Unknown("paused".into()));
    assert_eq!(received.known(), None);
    // Sending it back reproduces exactly what the server said.
    assert_eq!(to_value(&received).unwrap(), json!("paused"));

    let known: Open<v1::Status> = from_value(json!("past_due")).unwrap();
    assert_eq!(known.into_known(), Some(v1::Status::PastDue));
    assert!(from_value::<Open<v1::Status>>(json!(7)).is_err());
}

#[test]
fn only_string_enums_can_be_open() {
    #[derive(Serialize, Deserialize, Schema)]
    #[splinekit(name = "test.Tagged")]
    #[allow(dead_code)]
    enum Tagged {
        Value { amount: u32 },
    }
    let mut registry = Registry::new();
    assert!(matches!(
        registry.register::<Open<String>>(Direction::Serialize),
        Err(Error::InvalidShape(message)) if message.contains("open enum")
    ));
    assert!(matches!(
        registry.register::<Open<Tagged>>(Direction::Serialize),
        Err(Error::InvalidShape(_))
    ));
    assert!(registry.finish().unwrap().definitions().is_empty());
}
