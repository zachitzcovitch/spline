use proc_macro2::Span;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Deserialize,
    Serialize,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deserialize => "deserialize",
            Self::Serialize => "serialize",
        }
    }
}

pub struct Container {
    pub data: Data,
    pub transparent: bool,
    pub defaulted: bool,
    pub deny_unknown_fields: bool,
    pub tagging: Tagging,
}

pub enum Data {
    Struct(Style, Vec<Field>),
    Enum(Vec<Variant>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Named,
    Tuple,
    Newtype,
    Unit,
}

pub enum Tagging {
    External,
    Internal(syn::LitStr),
    Adjacent {
        tag: syn::LitStr,
        content: syn::LitStr,
    },
    Untagged,
}

pub struct Field {
    pub ty: syn::Type,
    pub attrs: Vec<syn::Attribute>,
    pub name: syn::LitStr,
    pub skipped: bool,
    pub defaulted: bool,
    pub omission: Option<syn::ExprPath>,
    pub custom_codec: bool,
    pub transparent_member: bool,
}

pub struct Variant {
    pub name: syn::LitStr,
    pub attrs: Vec<syn::Attribute>,
    pub style: Style,
    pub fields: Vec<Field>,
    pub skipped: bool,
    pub span: Span,
}
