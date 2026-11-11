//! Serde codecs for integers wider than JavaScript numbers hold exactly
//! (`u64`, `i64`, `u128`, `i128`, `usize`, `isize`).
//!
//! Use them with `#[serde(with = "...")]`; `#[derive(splinekit::Schema)]`
//! recognizes these paths and describes the field without a schema override:
//!
//! ```
//! #[derive(serde::Serialize, serde::Deserialize, splinekit::Schema)]
//! pub struct Balance {
//!     /// "18446744073709551615": every digit survives in every language.
//!     #[serde(with = "splinekit::codec::decimal_string")]
//!     pub credits: u64,
//!     /// A JSON number, refused outside ±(2^53 − 1).
//!     #[serde(with = "splinekit::codec::safe_number::option")]
//!     pub limit: Option<i64>,
//! }
//! ```
//!
//! Each codec has an `option` module for `Option<T>` fields.

macro_rules! field_schemas {
    ($marker:ident) => {
        /// Field types this codec supports, and the wire description the
        /// derive uses for them.
        pub trait Field {
            type Schema: crate::Schema;
        }
        field_schemas!(@impl $marker; u64, i64, u128, i128, usize, isize);
    };
    (@impl $marker:ident; $($ty:ty),*) => {$(
        impl Field for $ty {
            type Schema = crate::wire::$marker<$ty>;
        }
        impl Field for Option<$ty> {
            type Schema = Option<crate::wire::$marker<$ty>>;
        }
    )*};
}

/// Base-10 digits in a JSON string: `"18446744073709551615"`.
///
/// Input must be canonical (no sign on unsigned types, no `+`, no leading
/// zeros, no `-0`), so every client accepts exactly the same strings.
pub mod decimal_string {
    use std::fmt::Display;
    use std::str::FromStr;

    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::wire::WideInteger;

    field_schemas!(IntegerString);

    pub fn serialize<T: Display, S: Serializer>(
        value: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: WideInteger + FromStr,
        D: Deserializer<'de>,
    {
        parse(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }

    pub(super) fn parse<T: WideInteger + FromStr>(text: &str) -> Result<T, String> {
        let digits = match text.strip_prefix('-') {
            Some(rest) if T::SIGNED && rest != "0" => rest,
            _ => text,
        };
        let canonical = !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && (digits == "0" || !digits.starts_with('0'));
        if !canonical {
            return Err(format!("expected canonical base-10 digits, got {text:?}"));
        }
        text.parse()
            .map_err(|_| format!("{text} is outside the integer's range"))
    }

    /// The same encoding for `Option<T>`: `null` when absent.
    pub mod option {
        use std::fmt::Display;
        use std::str::FromStr;

        use serde::de::Error as _;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        use crate::wire::WideInteger;

        struct Digits<'a, T>(&'a T);

        impl<T: Display> Serialize for Digits<'_, T> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self.0)
            }
        }

        pub fn serialize<T: Display, S: Serializer>(
            value: &Option<T>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match value {
                Some(value) => serializer.serialize_some(&Digits(value)),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
        where
            T: WideInteger + FromStr,
            D: Deserializer<'de>,
        {
            Option::<String>::deserialize(deserializer)?
                .map(|text| super::parse(&text).map_err(D::Error::custom))
                .transpose()
        }
    }
}

/// A JSON number limited to JavaScript's safe-integer range, ±(2^53 − 1).
///
/// Serializing a value outside the range fails instead of sending a number
/// JavaScript would round; deserializing one fails too.
pub mod safe_number {
    use serde::de::Error as _;
    use serde::ser::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    field_schemas!(SafeInteger);

    const LIMIT: i64 = (1 << 53) - 1;

    pub(super) fn checked<T: Copy + TryInto<i64>>(value: T) -> Option<i64> {
        value
            .try_into()
            .ok()
            .filter(|value| (-LIMIT..=LIMIT).contains(value))
    }

    pub(super) fn narrowed<T: TryFrom<i64>>(value: i64) -> Result<T, String> {
        if !(-LIMIT..=LIMIT).contains(&value) {
            return Err(format!(
                "{value} is outside JavaScript's safe-integer range"
            ));
        }
        T::try_from(value).map_err(|_| format!("{value} is outside the integer's range"))
    }

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Copy + TryInto<i64>,
        S: Serializer,
    {
        let value = checked(*value)
            .ok_or_else(|| S::Error::custom("integer outside JavaScript's safe-integer range"))?;
        serializer.serialize_i64(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: TryFrom<i64>,
        D: Deserializer<'de>,
    {
        narrowed(i64::deserialize(deserializer)?).map_err(D::Error::custom)
    }

    /// The same encoding for `Option<T>`: `null` when absent.
    pub mod option {
        use serde::de::Error as _;
        use serde::ser::Error as _;
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<T, S>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
        where
            T: Copy + TryInto<i64>,
            S: Serializer,
        {
            match value {
                Some(value) => {
                    let value = super::checked(*value).ok_or_else(|| {
                        S::Error::custom("integer outside JavaScript's safe-integer range")
                    })?;
                    serializer.serialize_some(&value)
                }
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
        where
            T: TryFrom<i64>,
            D: Deserializer<'de>,
        {
            Option::<i64>::deserialize(deserializer)?
                .map(|value| super::narrowed(value).map_err(D::Error::custom))
                .transpose()
        }
    }
}
