//! Type-owned schema descriptions and a directional JSON registry.
//!
//! The normal dependency graph is std-only. Parser integration and comparisons
//! against actual Serde behavior live in the integration tests.

#![forbid(unsafe_code)]

mod builtins;
mod compare;
mod field;
mod model;
mod registry;

pub use field::{FieldRules, Omission};
pub use model::*;
pub use registry::{Context, Graph, Registry};

pub type Result<T> = std::result::Result<T, Error>;

/// Type-owned facts used by the registry and field builders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeInfo {
    pub name: Name,
    pub inline: bool,
    pub missing: MissingField,
    pub documentation: String,
}

impl TypeInfo {
    pub fn named(name: impl Into<Name>) -> Self {
        Self {
            name: name.into(),
            inline: false,
            missing: MissingField::Rejected,
            documentation: String::new(),
        }
    }

    pub fn inline(name: impl Into<Name>) -> Self {
        Self {
            inline: true,
            ..Self::named(name)
        }
    }

    pub fn accepts_missing(mut self) -> Self {
        self.missing = MissingField::Accepted;
        self
    }

    pub fn documented(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = documentation.into();
        self
    }
}

/// Descriptions must be deterministic for a fixed type and JSON profile.
///
/// `'static` permits process-local TypeId registration for the owned-DTO slice
/// of this experiment. Metadata is a declaration, not proof of arbitrary codec
/// behavior. Named descriptions must request children through Context::schema.
pub trait Schema: 'static {
    fn info() -> TypeInfo;
    fn describe(cx: &mut Context<'_>) -> Result<Shape>;

    /// Source variant names corresponding to the alternatives of a named union.
    /// These are Rust identities, independent of directional Serde wire names.
    /// Overrides and handwritten schemas need not claim a variant association.
    fn variants(_direction: Direction) -> Option<&'static [&'static str]> {
        None
    }

    /// Query missing-field behavior without constructing generic names.
    /// Transparent recursive derives override this; the default preserves
    /// existing handwritten descriptions that declare the fact in info().
    fn missing_field() -> MissingField {
        Self::info().missing
    }
}

/// Identifies the value serialized by a supported Option when it is Some.
/// This is separate from nullability: Some(None) can still serialize as null.
pub trait OptionSchema: Schema {
    type Value: Schema;
}
