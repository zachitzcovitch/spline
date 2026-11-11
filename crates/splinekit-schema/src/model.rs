use std::fmt;

/// A logical, namespace-qualified name with structured generic arguments.
/// Exporters can assign target-language identifiers independently.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    path: String,
    arguments: Vec<Name>,
}

impl Name {
    pub fn new(path: impl Into<String>) -> Self {
        Self::applied(path, Vec::new())
    }

    pub fn applied(path: impl Into<String>, arguments: Vec<Name>) -> Self {
        Self {
            path: path.into(),
            arguments,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn arguments(&self) -> &[Name] {
        &self.arguments
    }

    pub(crate) fn validate(&self) -> crate::Result<()> {
        for segment in self.path.split('.') {
            let mut chars = segment.chars();
            let valid_start = chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if !valid_start || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(Error::InvalidName(self.path.clone()));
            }
        }
        for argument in &self.arguments {
            argument.validate()?;
        }
        Ok(())
    }
}

impl From<&str> for Name {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)?;
        if !self.arguments.is_empty() {
            f.write_str("<")?;
            for (index, argument) in self.arguments.iter().enumerate() {
                if index > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{argument}")?;
            }
            f.write_str(">")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    Deserialize,
    Serialize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefinitionId {
    pub name: Name,
    pub direction: Direction,
}

impl fmt::Display for DefinitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.name, self.direction)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub shape: Shape,
    pub documentation: String,
    /// Source variant names in union-alternative order, when known.
    pub variants: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Null,
    Boolean,
    /// Any JSON string.
    String,
    /// A JSON string with a known lexical format. Exporters describe and, where
    /// they validate, check the format; Rust clients rely on Serde decoding.
    Formatted(StringFormat),
    /// A JSON integer of `bits` 8, 16, 32 or [`SAFE_INTEGER_BITS`].
    Integer {
        signed: bool,
        bits: u8,
    },
    /// A finite JSON number held in a 32- or 64-bit float. Non-finite values
    /// have no JSON form.
    Number {
        bits: u8,
    },
    Literal(String),
    /// One of the inner shape's known string values, or any other string: a
    /// forward-compatible enum. The inner shape must consist of string
    /// literals (directly, in a union, or through references).
    Open(Box<Shape>),
    Array(Box<Shape>),
    Tuple(Vec<Shape>),
    Map(Box<Shape>),
    Object(Object),
    Union(Vec<Shape>),
    Ref(DefinitionId),
}

/// Lexical formats of [`Shape::Formatted`] strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StringFormat {
    /// An RFC 9562 UUID in hyphenated form.
    Uuid,
    /// An RFC 3339 date-time with an offset or `Z`.
    DateTime,
    /// An RFC 3339 full date (`YYYY-MM-DD`).
    Date,
    /// An absolute URI.
    Uri,
    /// An exact base-10 decimal: optional `-`, digits, optional fraction.
    Decimal,
    /// A base-10 integer of the given width written as a string, for widths
    /// JavaScript numbers can't hold exactly (64 or 128 bits).
    Integer { signed: bool, bits: u8 },
}

/// The [`Shape::Integer`] width for JavaScript's safe-integer range:
/// ±(2^53 − 1), or 0 to 2^53 − 1 when unsigned. Every value in it survives
/// `JSON.parse` exactly.
pub const SAFE_INTEGER_BITS: u8 = 53;

/// Inclusive bounds of a [`Shape::Integer`], or `None` for an unsupported width.
pub fn integer_bounds(signed: bool, bits: u8) -> Option<(i64, i64)> {
    match bits {
        8 | 16 | 32 if signed => {
            let limit = 1_i64 << (bits - 1);
            Some((-limit, limit - 1))
        }
        8 | 16 | 32 => Some((0, (1_i64 << bits) - 1)),
        SAFE_INTEGER_BITS => {
            let limit = (1_i64 << 53) - 1;
            Some((if signed { -limit } else { 0 }, limit))
        }
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub fields: Vec<Field>,
    pub unknown_fields: UnknownFields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The name on the wire.
    pub name: String,
    pub shape: Shape,
    pub presence: Presence,
    pub documentation: Vec<String>,
    /// The authored Rust field name, when known (the derive records it).
    /// Exporters derive each language's idiomatic name from it, falling back
    /// to the wire name.
    pub source_name: Option<String>,
}

impl Field {
    pub fn required(name: impl Into<String>, shape: Shape) -> Self {
        Self {
            name: name.into(),
            shape,
            presence: Presence::Required,
            documentation: Vec::new(),
            source_name: None,
        }
    }

    /// The authored name exporters start from: the Rust name if recorded,
    /// otherwise the wire name.
    pub fn source(&self) -> &str {
        self.source_name.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Required,
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingField {
    Rejected,
    Accepted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownFields {
    Ignore,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidName(String),
    NameCollision {
        id: DefinitionId,
        first: &'static str,
        second: &'static str,
    },
    InlineRecursion(Name),
    MissingCodec(String),
    InvalidField(String),
    InvalidShape(String),
    UnresolvedReference(DefinitionId),
    DirectionMismatch(DefinitionId),
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "invalid logical schema name: {name}"),
            Self::NameCollision { id, first, second } => {
                write!(f, "schema name collision for {id}: {first} and {second}")
            }
            Self::InlineRecursion(name) => write!(f, "recursive schema must be named: {name}"),
            Self::MissingCodec(field) => write!(f, "field {field} needs a codec description"),
            Self::InvalidField(message) | Self::InvalidShape(message) => f.write_str(message),
            Self::UnresolvedReference(id) => write!(f, "unresolved schema reference: {id}"),
            Self::DirectionMismatch(id) => write!(f, "reference crosses schema directions: {id}"),
            Self::Unsupported(message) => write!(f, "unsupported JSON profile feature: {message}"),
        }
    }
}

impl std::error::Error for Error {}
