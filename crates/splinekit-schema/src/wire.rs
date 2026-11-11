//! Wire descriptions for `#[splinekit(schema = "...")]` overrides.
//!
//! A type whose Serde form is a formatted string or a JavaScript-safe integer
//! names one of these instead of plain `String` or an integer, so exporters
//! can describe and check the format:
//!
//! ```ignore
//! #[derive(serde::Serialize, serde::Deserialize, splinekit::Schema)]
//! #[serde(try_from = "String", into = "String")]
//! #[splinekit(schema = "splinekit::wire::Decimal")]
//! pub struct Usd(i64);
//! ```
//!
//! Like every override, these are declarations: the type's own Serde code must
//! produce and accept the described form.

use std::marker::PhantomData;

use crate::{Context, Name, Result, Schema, Shape, StringFormat, TypeInfo, SAFE_INTEGER_BITS};

macro_rules! formatted {
    ($ty:ident, $name:literal, $format:expr, $doc:literal) => {
        #[doc = $doc]
        pub enum $ty {}

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

formatted!(
    Uuid,
    "core.Uuid",
    StringFormat::Uuid,
    "A hyphenated UUID string."
);
formatted!(
    DateTime,
    "core.DateTime",
    StringFormat::DateTime,
    "An RFC 3339 date-time string with an offset or `Z`."
);
formatted!(
    Date,
    "core.Date",
    StringFormat::Date,
    "An RFC 3339 full-date string (`YYYY-MM-DD`)."
);
formatted!(
    Uri,
    "core.Uri",
    StringFormat::Uri,
    "An absolute URI string."
);
formatted!(
    Decimal,
    "core.Decimal",
    StringFormat::Decimal,
    "An exact base-10 decimal string, such as `-12.50`."
);

/// Integer types wider than JavaScript numbers can hold exactly.
pub trait WideInteger: sealed::Sealed + 'static {
    const SIGNED: bool;
    const BITS: u8;
    const NAME: &'static str;
}

mod sealed {
    pub trait Sealed {}
}

macro_rules! wide {
    ($($ty:ty => $signed:literal, $bits:literal, $name:literal;)*) => {$(
        impl sealed::Sealed for $ty {}
        impl WideInteger for $ty {
            const SIGNED: bool = $signed;
            const BITS: u8 = $bits;
            const NAME: &'static str = $name;
        }
    )*};
}

wide! {
    u64 => false, 64, "U64";
    i64 => true, 64, "I64";
    u128 => false, 128, "U128";
    i128 => true, 128, "I128";
    usize => false, 64, "Usize";
    isize => true, 64, "Isize";
}

/// `T` written as a base-10 string, preserving every digit across languages.
pub struct IntegerString<T: WideInteger>(PhantomData<fn() -> T>);

impl<T: WideInteger> Schema for IntegerString<T> {
    fn info() -> TypeInfo {
        TypeInfo::inline(Name::new(format!("core.{}String", T::NAME)))
    }

    fn describe(_: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::Formatted(StringFormat::Integer {
            signed: T::SIGNED,
            bits: T::BITS,
        }))
    }
}

/// `T` written as a JSON number limited to JavaScript's safe-integer range
/// (±(2^53 − 1)); the type's codec must refuse values outside it.
pub struct SafeInteger<T: WideInteger>(PhantomData<fn() -> T>);

impl<T: WideInteger> Schema for SafeInteger<T> {
    fn info() -> TypeInfo {
        TypeInfo::inline(Name::new(format!("core.Safe{}", T::NAME)))
    }

    fn describe(_: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::Integer {
            signed: T::SIGNED,
            bits: SAFE_INTEGER_BITS,
        })
    }
}
