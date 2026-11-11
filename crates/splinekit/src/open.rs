use serde::de::{DeserializeOwned, IntoDeserializer};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Context, Name, Schema, Shape, TypeInfo};

/// A forward-compatible string enum: one of `E`'s known values, or a value a
/// newer server added that this client doesn't know yet.
///
/// `E` is a unit-only enum that Serde writes as strings. Clients keep working
/// when the server gains a variant, and still see exactly what it sent:
///
/// ```
/// use splinekit::Open;
///
/// #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize, splinekit::Schema)]
/// #[serde(rename_all = "snake_case")]
/// enum Status { Active, PastDue }
///
/// let known: Open<Status> = serde_json::from_str("\"past_due\"").unwrap();
/// assert_eq!(known, Open::Known(Status::PastDue));
/// let newer: Open<Status> = serde_json::from_str("\"paused\"").unwrap();
/// assert_eq!(newer, Open::Unknown("paused".into()));
/// ```
///
/// TypeScript sees `Status | (string & {})`; OpenAPI lists the known values
/// alongside any string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Open<E> {
    Known(E),
    /// The exact string the server sent.
    Unknown(String),
}

impl<E> Open<E> {
    /// The known value, if this is one.
    pub fn known(&self) -> Option<&E> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown(_) => None,
        }
    }

    /// The known value, if this is one.
    pub fn into_known(self) -> Option<E> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown(_) => None,
        }
    }
}

impl<E> From<E> for Open<E> {
    fn from(value: E) -> Self {
        Self::Known(value)
    }
}

impl<E: Serialize> Serialize for Open<E> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Known(value) => value.serialize(serializer),
            Self::Unknown(value) => serializer.serialize_str(value),
        }
    }
}

impl<'de, E: DeserializeOwned> Deserialize<'de> for Open<E> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let known: Result<E, serde::de::value::Error> =
            E::deserialize(text.as_str().into_deserializer());
        Ok(known.map_or(Self::Unknown(text), Self::Known))
    }
}

impl<E: Schema> Schema for Open<E> {
    fn info() -> TypeInfo {
        TypeInfo::inline(Name::applied("core.Open", vec![E::info().name]))
    }

    fn describe(cx: &mut Context<'_>) -> crate::Result<Shape> {
        Ok(Shape::Open(Box::new(cx.schema::<E>()?)))
    }
}
