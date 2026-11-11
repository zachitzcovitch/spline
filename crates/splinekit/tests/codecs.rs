use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, to_value};
use splinekit::*;

mod support;

#[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
#[splinekit(name = "test.Wide")]
struct Wide {
    #[serde(with = "splinekit::codec::decimal_string")]
    credits: u64,
    #[serde(with = "splinekit::codec::decimal_string")]
    balance: i128,
    #[serde(with = "splinekit::codec::decimal_string::option")]
    extra: Option<u64>,
    #[serde(with = "splinekit::codec::safe_number")]
    count: i64,
    #[serde(with = "splinekit::codec::safe_number::option")]
    limit: Option<u64>,
}

fn sample() -> Wide {
    Wide {
        credits: u64::MAX,
        balance: i128::MIN,
        extra: None,
        count: -9_007_199_254_740_991,
        limit: Some(9_007_199_254_740_991),
    }
}

#[test]
fn derive_describes_builtin_codecs_without_a_schema_override() {
    for direction in [Direction::Serialize, Direction::Deserialize] {
        let (root, graph) = support::graph::<Wide>(direction);
        let body = support::body(&root, &graph);
        let digits = |signed, bits| Shape::Formatted(StringFormat::Integer { signed, bits });
        let safe = |signed| Shape::Integer {
            signed,
            bits: SAFE_INTEGER_BITS,
        };
        assert_eq!(support::field(body, "credits").shape, digits(false, 64));
        assert_eq!(support::field(body, "balance").shape, digits(true, 128));
        assert_eq!(
            support::field(body, "extra").shape,
            Shape::Union(vec![digits(false, 64), Shape::Null])
        );
        assert_eq!(support::field(body, "count").shape, safe(true));
        assert_eq!(
            support::field(body, "limit").shape,
            Shape::Union(vec![safe(false), Shape::Null])
        );
        let value = to_value(sample()).unwrap();
        assert!(support::matches(&graph, &root, &value));
    }
}

#[test]
fn decimal_strings_round_trip_every_digit_and_accept_only_canonical_input() {
    let wire = to_value(sample()).unwrap();
    assert_eq!(wire["credits"], json!("18446744073709551615"));
    assert_eq!(
        wire["balance"],
        json!("-170141183460469231731687303715884105728")
    );
    assert_eq!(wire["extra"], json!(null));
    assert_eq!(from_value::<Wide>(wire.clone()).unwrap(), sample());

    let with = |name: &str, value: serde_json::Value| {
        let mut changed = wire.clone();
        changed[name] = value;
        from_value::<Wide>(changed)
    };
    assert_eq!(with("extra", json!("7")).unwrap().extra, Some(7));
    for bad in [
        "007",
        "+7",
        "-1",
        " 7",
        "7 ",
        "1e3",
        "",
        "18446744073709551616",
    ] {
        assert!(with("credits", json!(bad)).is_err(), "credits {bad:?}");
    }
    for bad in ["-0", "--1", "-01"] {
        assert!(with("balance", json!(bad)).is_err(), "balance {bad:?}");
    }
    assert!(
        with("credits", json!(7)).is_err(),
        "a JSON number is not digits"
    );
}

#[test]
fn safe_numbers_refuse_values_javascript_would_round() {
    let limit = (1_i64 << 53) - 1;
    let wire = to_value(sample()).unwrap();
    assert_eq!(wire["count"], json!(-limit));
    assert_eq!(wire["limit"], json!(limit));

    let mut beyond = sample();
    beyond.count = limit + 1;
    assert!(to_value(beyond).is_err(), "serializing must refuse 2^53");
    let mut beyond = sample();
    beyond.limit = Some(u64::MAX);
    assert!(to_value(beyond).is_err());

    let with = |name: &str, value: serde_json::Value| {
        let mut changed = wire.clone();
        changed[name] = value;
        from_value::<Wide>(changed)
    };
    assert!(with("count", json!(limit + 1)).is_err());
    assert!(
        with("limit", json!(-1)).is_err(),
        "unsigned refuses negatives"
    );
    assert!(
        with("count", json!("7")).is_err(),
        "digits are not a number"
    );
    assert_eq!(with("limit", json!(null)).unwrap().limit, None);
}

#[test]
fn bare_wide_integers_point_to_the_codecs() {
    #[derive(Schema)]
    #[allow(dead_code)]
    struct Raw {
        value: u64,
    }
    let mut registry = Registry::new();
    let Err(Error::Unsupported(message)) = registry.register::<Raw>(Direction::Serialize) else {
        panic!("a bare u64 must not register");
    };
    assert!(
        message.contains("splinekit::codec::decimal_string"),
        "{message}"
    );
}
