//! Schemas for common ecosystem types, each behind the crate's feature of the
//! same name. Every description matches the type's own Serde form; the
//! `ecosystem` integration test checks that against real serialization.

#[cfg(any(
    feature = "uuid",
    feature = "chrono",
    feature = "url",
    feature = "rust_decimal"
))]
use crate::{Context, Result, Schema, Shape, StringFormat, TypeInfo};

#[cfg(any(
    feature = "uuid",
    feature = "chrono",
    feature = "url",
    feature = "rust_decimal"
))]
macro_rules! formatted {
    ($ty:ty, $name:literal, $format:expr) => {
        impl Schema for $ty {
            fn info() -> TypeInfo {
                TypeInfo::inline($name)
            }

            fn describe(_: &mut Context<'_>) -> Result<Shape> {
                Ok(Shape::Formatted($format))
            }
        }
    };
}

// Serde: a hyphenated lowercase string; input also accepts other UUID spellings.
#[cfg(feature = "uuid")]
formatted!(uuid::Uuid, "core.Uuid", StringFormat::Uuid);

// Serde: RFC 3339 with `Z` (UTC) or the offset (fixed offsets).
#[cfg(feature = "chrono")]
formatted!(
    chrono::DateTime<chrono::Utc>,
    "core.DateTimeUtc",
    StringFormat::DateTime
);
#[cfg(feature = "chrono")]
formatted!(
    chrono::DateTime<chrono::FixedOffset>,
    "core.DateTimeFixedOffset",
    StringFormat::DateTime
);
// Serde: `YYYY-MM-DD`.
#[cfg(feature = "chrono")]
formatted!(chrono::NaiveDate, "core.NaiveDate", StringFormat::Date);

// Serde: the absolute URL as a string.
#[cfg(feature = "url")]
formatted!(url::Url, "core.Url", StringFormat::Uri);

// Serde (default `serde` feature): an exact decimal string; input also accepts
// JSON numbers, which this description doesn't advertise.
#[cfg(feature = "rust_decimal")]
formatted!(
    rust_decimal::Decimal,
    "core.RustDecimal",
    StringFormat::Decimal
);
