use std::fmt;
use std::marker::PhantomData;

use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use spline_schema::{Context, MissingField, Name, Schema, Shape, TypeInfo};

/// A field-wise update: absent means unchanged, present means replace with T.
///
/// Pair fields with `#[serde(default, skip_serializing_if = "spline::Patch::is_missing")]`.
/// Use `Patch<Option<T>>` when an explicit JSON null should clear a nullable field.
/// The application defines how to apply updates; this is not an automatic merge algorithm.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Patch<T> {
    #[default]
    Missing,
    Value(T),
}

impl<T> Patch<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<T> From<T> for Patch<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Missing => Err(serde::ser::Error::custom(
                "Patch::Missing has no JSON value; use skip_serializing_if = \"spline::Patch::is_missing\" on its field",
            )),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Present<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Present<T> {
            type Value = Patch<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a present patch field")
            }
            fn visit_newtype_struct<D: Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                T::deserialize(deserializer).map(Patch::Value)
            }
        }
        // A newtype visitor makes a missing field an error unless the enclosing
        // field supplies a default. Delegating straight to Option::deserialize
        // would collapse an absent Patch<Option<T>> into Value(None).
        deserializer.deserialize_newtype_struct("Patch", Present(PhantomData))
    }
}

impl<T: Schema> Schema for Patch<T> {
    fn info() -> TypeInfo {
        TypeInfo::inline(Name::applied("spline.Patch", vec![T::info().name]))
    }
    fn describe(cx: &mut Context<'_>) -> spline_schema::Result<Shape> {
        cx.schema::<T>()
    }
    fn missing_field() -> MissingField {
        MissingField::Rejected
    }
}
