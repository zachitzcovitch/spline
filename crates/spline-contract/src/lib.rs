//! Transport-neutral operation metadata. Construction registers the server's
//! input deserialization shapes and its output/error serialization shapes.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use spline_schema::{Direction, Graph, Presence, Registry, Schema, Shape};

pub mod http;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Query,
    Mutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    pub id: String,
    pub name: String,
    pub rust_name: String,
    pub kind: OperationKind,
    pub documentation: String,
    pub input: Shape,
    /// Authored argument order, independent of operation sorting and JSON key order.
    pub arguments: Vec<Argument>,
    pub output: Shape,
    pub error: Shape,
    pub http: Option<http::HttpPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub shape: Shape,
    pub optional: bool,
}

/// Sealed through a builder, so identifiers, operations, and schemas agree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractDescription {
    namespace: String,
    documentation: String,
    operations: BTreeMap<String, Operation>,
    schemas: Graph,
}

impl ContractDescription {
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    pub fn documentation(&self) -> &str {
        &self.documentation
    }
    pub fn operations(&self) -> &BTreeMap<String, Operation> {
        &self.operations
    }
    pub fn schemas(&self) -> &Graph {
        &self.schemas
    }
}

pub struct ContractBuilder {
    namespace: String,
    documentation: String,
    operations: BTreeMap<String, Operation>,
    schemas: Registry,
}

impl ContractBuilder {
    pub fn new(namespace: impl Into<String>, documentation: impl Into<String>) -> Result<Self> {
        let namespace = namespace.into();
        if !namespace.split('.').all(identifier) {
            return Err(Error::InvalidIdentifier(namespace));
        }
        Ok(Self {
            namespace,
            documentation: documentation.into(),
            operations: BTreeMap::new(),
            schemas: Registry::new(),
        })
    }

    pub fn operation<Input: Schema, Output: Schema, ApplicationError: Schema>(
        &mut self,
        name: &str,
        rust_name: &str,
        kind: OperationKind,
        documentation: &str,
    ) -> Result<()> {
        if !identifier(name) || !identifier(rust_name) {
            return Err(Error::InvalidIdentifier(format!("{name} / {rust_name}")));
        }
        if self.operations.contains_key(name) {
            return Err(Error::DuplicateOperation(format!(
                "{}.{name}",
                self.namespace
            )));
        }
        // Commit the entire operation together; output failure must not leave
        // successfully registered input definitions behind.
        let mut candidate = self.schemas.clone();
        let input = candidate.register::<Input>(Direction::Deserialize)?;
        let output = candidate.register::<Output>(Direction::Serialize)?;
        let error = candidate.register::<ApplicationError>(Direction::Serialize)?;
        let graph = candidate.clone().finish()?;
        let arguments = match http::resolve(&input, &graph)? {
            Shape::Object(object) => object
                .fields
                .iter()
                .map(|field| Argument {
                    name: field.name.clone(),
                    shape: field.shape.clone(),
                    optional: field.presence == Presence::Optional,
                })
                .collect(),
            Shape::Null => Vec::new(),
            _ => vec![Argument {
                name: "input".into(),
                shape: input.clone(),
                optional: false,
            }],
        };
        self.operations.insert(
            name.into(),
            Operation {
                id: format!("{}.{name}", self.namespace),
                name: name.into(),
                rust_name: rust_name.into(),
                kind,
                documentation: documentation.into(),
                input,
                arguments,
                output,
                error,
                http: None,
            },
        );
        self.schemas = candidate;
        Ok(())
    }

    pub fn http(&mut self, operation: &str, binding: http::HttpBinding) -> Result<()> {
        let target = self
            .operations
            .get(operation)
            .ok_or_else(|| Error::Http(format!("unknown operation {operation}")))?;
        if target.http.is_some() {
            return Err(Error::Http(format!("duplicate binding for {operation}")));
        }
        let graph = self.schemas.clone().finish()?;
        let plan = http::plan(target, binding, &graph)?;
        for existing in self
            .operations
            .values()
            .filter_map(|operation| operation.http.as_ref())
        {
            if existing.route_key == plan.route_key
                && (existing.binding.path != plan.binding.path
                    || existing.binding.method == plan.binding.method)
            {
                return Err(Error::Http(format!(
                    "conflicting HTTP route {}",
                    plan.binding.path
                )));
            }
        }
        self.operations.get_mut(operation).unwrap().http = Some(plan);
        Ok(())
    }

    pub fn finish(self) -> Result<ContractDescription> {
        if self.operations.is_empty() {
            return Err(Error::EmptyContract);
        }
        Ok(ContractDescription {
            namespace: self.namespace,
            documentation: self.documentation,
            operations: self.operations,
            schemas: self.schemas.finish()?,
        })
    }
}

fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentifier(String),
    DuplicateOperation(String),
    EmptyContract,
    Http(String),
    Schema(spline_schema::Error),
}

impl From<spline_schema::Error> for Error {
    fn from(error: spline_schema::Error) -> Self {
        Self::Schema(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(id) => write!(f, "invalid contract identifier: {id}"),
            Self::DuplicateOperation(id) => write!(f, "duplicate operation: {id}"),
            Self::EmptyContract => f.write_str("a contract must contain at least one operation"),
            Self::Http(message) => write!(f, "invalid HTTP binding: {message}"),
            Self::Schema(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Schema(error) => Some(error),
            _ => None,
        }
    }
}
