//! Ecosystem integrations: each type's real Serde output satisfies its
//! described form and decodes back. Run with
//! `cargo test -p splinekit-schema --all-features --test ecosystem`.
#![cfg(all(
    feature = "uuid",
    feature = "chrono",
    feature = "url",
    feature = "rust_decimal"
))]

mod support;

use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{from_value, json, to_value, Value};
use splinekit_schema::*;
use support::*;

/// Registers `T` in both directions, checks its form, then round-trips every value.
fn check<T>(format: StringFormat, values: &[T])
where
    T: Schema + Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let mut registry = Registry::new();
    let output = registry.register::<T>(Direction::Serialize).unwrap();
    let input = registry.register::<T>(Direction::Deserialize).unwrap();
    let graph = registry.finish().unwrap();
    assert_eq!(output, Shape::Formatted(format));
    assert_eq!(input, Shape::Formatted(format));
    for value in values {
        let wire = to_value(value).unwrap();
        assert!(matches(&graph, &output, &wire), "{wire} is not {format:?}");
        assert_eq!(&from_value::<T>(wire).unwrap(), value);
    }
}

fn rejected<T: Schema>(format: StringFormat, invalid: &[Value]) {
    let mut registry = Registry::new();
    let shape = registry.register::<T>(Direction::Serialize).unwrap();
    let graph = registry.finish().unwrap();
    assert_eq!(shape, Shape::Formatted(format));
    for value in invalid {
        assert!(
            !matches(&graph, &shape, value),
            "{value} accepted as {format:?}"
        );
    }
}

#[test]
fn uuid_serializes_as_a_hyphenated_string() {
    check(
        StringFormat::Uuid,
        &[
            uuid::Uuid::nil(),
            uuid::Uuid::max(),
            uuid::Uuid::from_str("0191f1b2-7c4e-7a3b-9f1e-2a3b4c5d6e7f").unwrap(),
        ],
    );
    assert_eq!(
        to_value(uuid::Uuid::max()).unwrap(),
        json!("ffffffff-ffff-ffff-ffff-ffffffffffff")
    );
    rejected::<uuid::Uuid>(StringFormat::Uuid, &[json!("0191f1b27c4e7a3b"), json!(7)]);
}

#[test]
fn chrono_date_times_serialize_as_rfc3339_with_z_or_an_offset() {
    use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
    let utc = Utc.with_ymd_and_hms(2026, 11, 11, 5, 6, 53).unwrap();
    check(
        StringFormat::DateTime,
        &[
            utc,
            utc + chrono::Duration::nanoseconds(123_456_789),
            DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        ],
    );
    assert_eq!(to_value(utc).unwrap(), json!("2026-11-11T05:06:53Z"));
    let offset = FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap()
        .with_ymd_and_hms(2026, 11, 11, 10, 36, 53)
        .unwrap();
    check(StringFormat::DateTime, &[offset]);
    assert_eq!(
        to_value(offset).unwrap(),
        json!("2026-11-11T10:36:53+05:30")
    );
    check(
        StringFormat::Date,
        &[NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()],
    );
    rejected::<DateTime<Utc>>(StringFormat::DateTime, &[json!("2026-11-11 05:06:53")]);
    rejected::<NaiveDate>(StringFormat::Date, &[json!("11/11/2026")]);
}

#[test]
fn urls_serialize_as_absolute_strings() {
    check(
        StringFormat::Uri,
        &[
            url::Url::parse("https://example.test/a?b=c#d").unwrap(),
            url::Url::parse("mailto:billing@example.test").unwrap(),
        ],
    );
    rejected::<url::Url>(StringFormat::Uri, &[json!("relative/path")]);
}

#[test]
fn rust_decimal_serializes_as_an_exact_decimal_string() {
    use rust_decimal::Decimal;
    check(
        StringFormat::Decimal,
        &[
            Decimal::new(-1250, 2),
            Decimal::ZERO,
            Decimal::MAX,
            Decimal::from_str("0.000000001").unwrap(),
        ],
    );
    assert_eq!(to_value(Decimal::new(-1250, 2)).unwrap(), json!("-12.50"));
    // Input also accepts numbers; the description advertises only the string.
    assert_eq!(
        from_value::<Decimal>(json!(1.5)).unwrap(),
        Decimal::new(15, 1)
    );
    rejected::<Decimal>(StringFormat::Decimal, &[json!("1e5"), json!(1.5)]);
}
