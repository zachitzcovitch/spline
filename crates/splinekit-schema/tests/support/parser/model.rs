//! Test inspection metadata; no parser or Serde imports.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Serialize,
    Deserialize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    pub rust_name: String,
    pub wire_name: String,
    pub direction: Direction,
    pub generics: String,
    pub where_clause: Option<String>,
    pub explicit_bounds: Option<Vec<String>>,
    pub docs: Vec<String>,
    pub deserialize_default: DefaultRule,
    pub deny_unknown_fields: bool,
    pub transparent: bool,
    pub tagging: Tagging,
    pub conversion: Option<Conversion>,
    pub data: Data,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Data {
    Struct { style: Style, fields: Vec<Field> },
    Enum(Vec<Variant>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Named,
    Tuple,
    Newtype,
    Unit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tagging {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
    Untagged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub rust_name: String,
    pub wire_name: String,
    /// Additional accepted names, excluding the canonical name; input only.
    pub aliases: Vec<String>,
    pub skipped: bool,
    pub untagged: bool,
    pub other: bool,
    pub codec: Option<Codec>,
    pub style: Style,
    pub fields: Vec<Field>,
    pub docs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub member: String,
    pub rust_type: String,
    pub wire_name: String,
    pub aliases: Vec<String>,
    pub skipped: bool,
    /// Deserialization information, even in a serialization inspection.
    /// Unspecified does NOT imply that the field is required (e.g. Option).
    pub deserialize_default: DefaultRule,
    /// Serialization predicate, present only in the serialization view.
    pub omit_if: Option<String>,
    pub codec: Option<Codec>,
    pub flattened: bool,
    pub transparent_member: bool,
    pub borrowed_lifetimes: Vec<String>,
    pub explicit_bounds: Option<Vec<String>>,
    pub docs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultRule {
    Unspecified,
    Trait,
    Function(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Codec {
    Function(String),
    /// Serde synthesizes these helpers for #[serde(borrow)] on Cow fields.
    BorrowedCowString,
    BorrowedCowBytes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conversion {
    From(String),
    TryFrom(String),
    Into(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub origin: DiagnosticOrigin,
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticOrigin {
    Syntax,
    Serde,
    Adapter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// One-based line; column is zero-based.
    pub line: usize,
    pub column: usize,
}
